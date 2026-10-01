//! `data pull`: download a published data file and put it in place.
//!
//! The file is downloaded and unpacked beside the place it will live,
//! checked, and only then renamed over the old one. A download that fails,
//! at any point and for any reason, leaves the old data file as it was.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use flate2::read::GzDecoder;
use serde::Serialize;
use serde_json::Value;

use crate::data::{self, Status};
use crate::env::DATA_FILE;
use crate::error::{
    BAD_RESPONSE, CliError, DATA_INVALID, DOWNLOAD_FAILED, IO, NETWORK, NOT_FOUND, RATE_LIMITED,
};
use crate::local::Local;
use crate::remote;

/// The largest data file accepted once unpacked. The file of 2026.09 is
/// 122 MB.
pub const LARGEST: u64 = 4 * 1024 * 1024 * 1024;
/// The longest release list read.
const LONGEST_LIST: u64 = 4 * 1024 * 1024;
/// A part-downloaded file this old was left by a download that died.
const STALE: Duration = Duration::from_secs(60 * 60);
/// How long the release list may take.
const LIST_TIMEOUT: Duration = Duration::from_secs(30);

const WRITE_HINT: &str =
    "Set WENMAR_OPEN_DATA_DIR to a directory you can write to, or pass --data-dir.";

/// What `data pull` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pulled {
    /// Whether a file was downloaded. `false` when the data file was
    /// already that version.
    pub updated: bool,
    pub data_version: String,
    /// The version that was in place before, when it was another.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// A published data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The data version, such as `2026.09`.
    pub version: String,
    /// Where the gzipped file is.
    pub url: String,
}

/// The numbers of a data version: `2026.09` is `[2026, 9]` and `2026.09.1`
/// is `[2026, 9, 1]`. `None` for anything else.
pub fn version_numbers(version: &str) -> Option<Vec<u32>> {
    let parts: Vec<&str> = version.split('.').collect();
    let well_formed = matches!(parts.len(), 2 | 3)
        && parts.first().is_some_and(|year| year.len() == 4)
        && parts.get(1).is_some_and(|month| month.len() == 2)
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 4 && part.bytes().all(|byte| byte.is_ascii_digit())
        });
    if !well_formed {
        return None;
    }
    parts.iter().map(|part| part.parse().ok()).collect()
}

/// Reads one release of the release list. `None` when it is not a
/// published data release with its file attached.
pub fn release_of(release: &Value) -> Option<Release> {
    let flag = |name: &str| release.get(name).and_then(Value::as_bool).unwrap_or(false);
    if flag("draft") || flag("prerelease") {
        return None;
    }
    let version = release.get("tag_name")?.as_str()?.strip_prefix("data-")?;
    version_numbers(version)?;
    let wanted = format!("wenmar-open-{version}.sqlite3.gz");
    let url = release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|asset| asset.get("name").and_then(Value::as_str) == Some(wanted.as_str()))?
        .get("browser_download_url")?
        .as_str()?;
    remote::is_http(url).then(|| Release {
        version: version.to_owned(),
        url: url.to_owned(),
    })
}

/// The data release with the highest version in a release list.
pub fn newest(releases: &Value) -> Option<Release> {
    releases
        .as_array()?
        .iter()
        .filter_map(release_of)
        .max_by_key(|release| version_numbers(&release.version))
}

fn get_json(url: &str) -> Result<(u16, Value), CliError> {
    let unreachable = |cause: &ureq::Error| {
        CliError::new(
            NETWORK,
            format!("The release list at {url} could not be reached: {cause}."),
        )
        .with_hint("Check the connection and try again.")
    };
    let mut response = remote::agent(Some(LIST_TIMEOUT))
        .get(url)
        .header("accept", "application/vnd.github+json")
        .call()
        .map_err(|error| unreachable(&error))?;
    let status = response.status().as_u16();
    if matches!(status, 403 | 429) {
        return Err(CliError::new(
            RATE_LIMITED,
            "The release server is limiting requests from this address.",
        )
        .with_hint("Try again in an hour."));
    }
    let text = response
        .body_mut()
        .with_config()
        .limit(LONGEST_LIST)
        .read_to_string()
        .map_err(|error| unreachable(&error))?;
    match serde_json::from_str(&text) {
        Ok(value) => Ok((status, value)),
        Err(_) => Err(CliError::new(
            BAD_RESPONSE,
            format!("The answer from {url} was not a release list (HTTP {status})."),
        )
        .with_hint("Check --releases and WENMAR_OPEN_RELEASES.")),
    }
}

/// Finds the release to download: the one asked for, or the newest.
pub fn find(releases_url: &str, version: Option<&str>) -> Result<Release, CliError> {
    if !remote::is_http(releases_url) {
        return Err(CliError::new(
            crate::error::USAGE,
            "the release address must start with http:// or https://",
        )
        .with_hint("Check --releases and WENMAR_OPEN_RELEASES."));
    }
    match version {
        Some(version) => {
            if version_numbers(version).is_none() {
                return Err(CliError::validation(
                    "version",
                    "version must look like 2026.09 or 2026.09.1",
                ));
            }
            let missing =
                || CliError::new(NOT_FOUND, format!("There is no data release {version}."));
            let (status, release) = get_json(&format!("{releases_url}/tags/data-{version}"))?;
            if status == 404 {
                return Err(missing());
            }
            release_of(&release).ok_or_else(missing)
        }
        None => {
            let (_, releases) = get_json(&format!("{releases_url}?per_page=100"))?;
            if !releases.is_array() {
                return Err(CliError::new(
                    BAD_RESPONSE,
                    format!("The answer from {releases_url} was not a release list."),
                )
                .with_hint("Check --releases and WENMAR_OPEN_RELEASES."));
            }
            newest(&releases)
                .ok_or_else(|| CliError::new(NOT_FOUND, "No data release has been published yet."))
        }
    }
}

/// A part-downloaded file, removed when this is dropped unless it was
/// renamed into place first.
struct Part(PathBuf);

impl Drop for Part {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Removes part-downloaded files that a dead download left in `directory`.
/// One that was written to in the last hour may belong to a download that
/// is still running, and is left alone.
fn remove_stale_parts(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let prefix = format!("{DATA_FILE}.");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(&prefix) && name.ends_with(".part")) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > STALE);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn not_whole(cause: impl std::fmt::Display) -> CliError {
    CliError::new(
        DOWNLOAD_FAILED,
        format!("The download did not arrive whole: {cause}."),
    )
    .with_hint("Nothing was changed. Run `wenmar-open data pull` again.")
}

fn not_written(path: &Path, cause: &std::io::Error) -> CliError {
    CliError::new(
        IO,
        format!("{} could not be written: {cause}.", path.display()),
    )
    .with_hint(WRITE_HINT)
}

/// Downloads `url` and unpacks it into `part`. Returns the unpacked size.
fn download(url: &str, part: &Path) -> Result<u64, CliError> {
    // No limit on the whole transfer: the file is large and a connection
    // may be slow. A connection that stops answering still ends it.
    let response = remote::agent(None)
        .get(url)
        .call()
        .map_err(|error| not_whole(&error))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(not_whole(format!("the server answered HTTP {status}")));
    }
    let packed = response
        .into_body()
        .into_with_config()
        .limit(LARGEST)
        .reader();
    let mut unpacked = GzDecoder::new(packed);
    let mut file = File::create(part).map_err(|error| not_written(part, &error))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let read = match unpacked.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(not_whole(&error)),
        };
        written = written.saturating_add(read as u64);
        if written > LARGEST {
            return Err(not_whole("it is far larger than a data file"));
        }
        let chunk = buffer.get(..read).unwrap_or_default();
        file.write_all(chunk)
            .map_err(|error| not_written(part, &error))?;
    }
    if written == 0 {
        return Err(not_whole("it is empty"));
    }
    file.sync_all().map_err(|error| not_written(part, &error))?;
    Ok(written)
}

/// Checks that the unpacked file is the data file the release says it is,
/// and that this build can answer from it.
fn verify(part: &Path, release: &Release) -> Result<Status, CliError> {
    let status = data::inspect(part);
    let refuse = |problem: &str| {
        CliError::new(
            DATA_INVALID,
            format!("The downloaded file cannot be used: {problem}."),
        )
    };
    if let Some(problem) = &status.problem {
        let error = refuse(problem);
        return Err(match &status.schema_version {
            Some(_) => error.with_hint(
                "This data release is for another version of wenmar-open. Update wenmar-open, or name an older release, as in `wenmar-open data pull 2026.08`.",
            ),
            None => error,
        });
    }
    if status.data_version.as_deref() != Some(release.version.as_str()) {
        return Err(refuse(&format!(
            "it says it is data version {}, and the release is {}",
            status.data_version.as_deref().unwrap_or("unknown"),
            release.version
        )));
    }
    // Opening it reads the catalog's makes, as every command will.
    Local::open(part).map_err(|error| refuse(error.message.trim_end_matches('.')))?;
    Ok(status)
}

/// Downloads a data release into `directory` and puts it in place.
pub fn pull(
    directory: &Path,
    releases_url: &str,
    version: Option<&str>,
    force: bool,
) -> Result<Pulled, CliError> {
    let path = directory.join(DATA_FILE);
    let release = find(releases_url, version)?;
    let before = data::inspect(&path);
    if !force && before.usable && before.data_version.as_deref() == Some(release.version.as_str()) {
        return Ok(Pulled {
            updated: false,
            data_version: release.version,
            previous: None,
            path: before.path,
            bytes: before.bytes,
        });
    }

    std::fs::create_dir_all(directory).map_err(|error| {
        CliError::new(
            IO,
            format!(
                "The data directory {} could not be created: {error}.",
                directory.display()
            ),
        )
        .with_hint(WRITE_HINT)
    })?;
    remove_stale_parts(directory);

    let part = Part(directory.join(format!("{DATA_FILE}.{}.part", std::process::id())));
    let bytes = download(&release.url, &part.0)?;
    verify(&part.0, &release)?;
    std::fs::rename(&part.0, &path).map_err(|error| {
        CliError::new(
            IO,
            format!("{} could not be replaced: {error}.", path.display()),
        )
        .with_hint(
            "If `wenmar-open mcp` or another copy is running on Windows, stop it and pull again.",
        )
    })?;

    Ok(Pulled {
        updated: true,
        previous: before
            .data_version
            .filter(|previous| *previous != release.version),
        data_version: release.version,
        path: path.display().to_string(),
        bytes: Some(bytes),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn release(tag: &str, asset: &str) -> Value {
        json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": false,
            "assets": [
                { "name": format!("{asset}.sha256"), "browser_download_url": "https://example.test/sum" },
                { "name": asset, "browser_download_url": format!("https://example.test/{asset}") }
            ]
        })
    }

    #[test]
    fn a_data_version_is_a_year_a_month_and_perhaps_a_rebuild() {
        assert_eq!(version_numbers("2026.09"), Some(vec![2026, 9]));
        assert_eq!(version_numbers("2026.09.1"), Some(vec![2026, 9, 1]));
        for text in [
            "",
            "2026",
            "2026.9",
            "26.09",
            "2026.09.",
            "2026.09.1.2",
            "2026.0a",
            "../x",
            "2026.09/..",
            "2026.09.99999",
            " 2026.09",
            "２０２６.０９",
        ] {
            assert_eq!(version_numbers(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_newest_data_release_is_chosen_by_its_numbers() {
        let releases = json!([
            release("v0.3.0", "wenmar-open-x86_64.tar.gz"),
            release("data-2026.09", "wenmar-open-2026.09.sqlite3.gz"),
            release("data-2026.10", "something-else.gz"),
            release("data-2026.09.2", "wenmar-open-2026.09.2.sqlite3.gz"),
            release("data-2026.09.10", "wenmar-open-2026.09.10.sqlite3.gz"),
            release("data-2026.08", "wenmar-open-2026.08.sqlite3.gz"),
            release("data-nonsense", "wenmar-open-nonsense.sqlite3.gz"),
            json!({ "tag_name": "data-2027.01", "draft": true, "assets": [] }),
            json!(null),
            json!("text")
        ]);
        assert_eq!(
            newest(&releases),
            Some(Release {
                version: "2026.09.10".to_owned(),
                url: "https://example.test/wenmar-open-2026.09.10.sqlite3.gz".to_owned()
            })
        );
        assert_eq!(newest(&json!([])), None);
        assert_eq!(newest(&json!({ "message": "Not Found" })), None);
    }

    #[test]
    fn a_release_must_be_published_and_carry_its_file_at_an_http_address() {
        let good = release("data-2026.09", "wenmar-open-2026.09.sqlite3.gz");
        assert!(release_of(&good).is_some());
        let mut draft = good.clone();
        draft["draft"] = json!(true);
        assert_eq!(release_of(&draft), None);
        let mut early = good.clone();
        early["prerelease"] = json!(true);
        assert_eq!(release_of(&early), None);
        let mut elsewhere = good.clone();
        elsewhere["assets"][1]["browser_download_url"] = json!("file:///etc/passwd");
        assert_eq!(release_of(&elsewhere), None);
        let mut bare = good;
        bare["assets"] = json!([]);
        assert_eq!(release_of(&bare), None);
    }
}
