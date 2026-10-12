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
use crate::env::{DATA_FILE, SHOP_FILE};
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

/// Removes part-downloaded files of `file` that a dead download left in
/// `directory`. One that was written to in the last hour may belong to a
/// download that is still running, and is left alone.
fn remove_stale_parts(directory: &Path, file: &str) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let prefix = format!("{file}.");
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
fn download(url: &str, part: &Path, patience: Patience) -> Result<u64, CliError> {
    // A server that takes the request and does not answer, or stops part
    // way through the body, ends the download when `patience` runs out.
    let response = remote::download_agent(patience.response, patience.body)
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

/// How long a download may wait for the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patience {
    /// For the answer to begin, once the request has been sent.
    pub response: Duration,
    /// For the whole body to arrive, once the answer has begun. It is a
    /// limit on the body as a whole and not on a pause in it, so it has to
    /// leave room for a slow connection: the default is half an hour, which
    /// a 50 MB download meets at 28 KB a second.
    pub body: Duration,
}

impl Default for Patience {
    fn default() -> Patience {
        Patience {
            response: Duration::from_secs(30),
            body: Duration::from_secs(30 * 60),
        }
    }
}

/// Downloads a data release into `directory` and puts it in place.
pub fn pull(
    directory: &Path,
    releases_url: &str,
    version: Option<&str>,
    force: bool,
) -> Result<Pulled, CliError> {
    pull_with(directory, releases_url, version, force, Patience::default())
}

/// [`pull`], waiting for the download only as long as `patience` says.
pub fn pull_with(
    directory: &Path,
    releases_url: &str,
    version: Option<&str>,
    force: bool,
    patience: Patience,
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
    remove_stale_parts(directory, DATA_FILE);

    let part = Part(directory.join(format!("{DATA_FILE}.{}.part", std::process::id())));
    let bytes = download(&release.url, &part.0, patience)?;
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

// ----- the shop file -----

/// The most shop releases whose manifests are read while looking for the
/// one that goes with the data file in place.
const MOST_MANIFESTS: usize = 8;

/// A published shop file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopRelease {
    /// The shop file's version, such as `2026.10.12`.
    pub version: String,
    /// Where the gzipped file is.
    pub url: String,
    /// Where the manifest is: a small JSON file that says which data
    /// version the shop file was matched against.
    pub manifest: String,
}

/// The numbers of a shop version: `2026.10.12` is `[2026, 10, 12]` and
/// `2026.10.12.1`, the first rebuild of that day, is `[2026, 10, 12, 1]`.
/// `None` for anything else.
pub fn shop_version_numbers(version: &str) -> Option<Vec<u32>> {
    let parts: Vec<&str> = version.split('.').collect();
    let well_formed = matches!(parts.len(), 3 | 4)
        && parts.first().is_some_and(|year| year.len() == 4)
        && parts.get(1).is_some_and(|month| month.len() == 2)
        && parts.get(2).is_some_and(|day| day.len() == 2)
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 4 && part.bytes().all(|byte| byte.is_ascii_digit())
        });
    if !well_formed {
        return None;
    }
    parts.iter().map(|part| part.parse().ok()).collect()
}

/// Reads one release of the release list. `None` when it is not a
/// published shop release with its file and its manifest attached.
pub fn shop_release_of(release: &Value) -> Option<ShopRelease> {
    let flag = |name: &str| release.get(name).and_then(Value::as_bool).unwrap_or(false);
    if flag("draft") || flag("prerelease") {
        return None;
    }
    let version = release.get("tag_name")?.as_str()?.strip_prefix("shop-")?;
    shop_version_numbers(version)?;
    let assets = release.get("assets")?.as_array()?;
    let address = |name: String| {
        assets
            .iter()
            .find(|asset| asset.get("name").and_then(Value::as_str) == Some(name.as_str()))
            .and_then(|asset| asset.get("browser_download_url"))
            .and_then(Value::as_str)
            .filter(|url| remote::is_http(url))
            .map(str::to_owned)
    };
    Some(ShopRelease {
        url: address(format!("wenmar-open-shop-{version}.sqlite3.gz"))?,
        manifest: address(format!("wenmar-open-shop-{version}.json"))?,
        version: version.to_owned(),
    })
}

/// The shop releases of a release list, newest first.
pub fn shop_releases(releases: &Value) -> Vec<ShopRelease> {
    let mut found: Vec<ShopRelease> = releases
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(shop_release_of)
        .collect();
    found.sort_by_key(|release| std::cmp::Reverse(shop_version_numbers(&release.version)));
    found
}

/// The data version a shop release was matched against, from its manifest.
fn matched_against(release: &ShopRelease) -> Result<String, CliError> {
    let (_, manifest) = get_json(&release.manifest)?;
    manifest
        .get("catalog_data_version")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CliError::new(
                BAD_RESPONSE,
                format!(
                    "The manifest of shop data {} does not say which data version it goes with.",
                    release.version
                ),
            )
        })
}

/// Finds the shop release to download: the one asked for, or the newest
/// that was matched against the data file in place, whose version is
/// `core`.
pub fn find_shop(
    releases_url: &str,
    version: Option<&str>,
    core: &str,
) -> Result<ShopRelease, CliError> {
    if !remote::is_http(releases_url) {
        return Err(CliError::new(
            crate::error::USAGE,
            "the release address must start with http:// or https://",
        )
        .with_hint("Check --releases and WENMAR_OPEN_RELEASES."));
    }
    match version {
        Some(version) => {
            if shop_version_numbers(version).is_none() {
                return Err(CliError::validation(
                    "version",
                    "a shop data version looks like 2026.10.12 or 2026.10.12.1",
                ));
            }
            let missing = || {
                CliError::new(
                    NOT_FOUND,
                    format!("There is no shop data release {version}."),
                )
            };
            let (status, release) = get_json(&format!("{releases_url}/tags/shop-{version}"))?;
            if status == 404 {
                return Err(missing());
            }
            let release = shop_release_of(&release).ok_or_else(missing)?;
            let matched = matched_against(&release)?;
            if matched != core {
                return Err(CliError::new(
                    DATA_INVALID,
                    format!(
                        "Shop data {version} goes with data version {matched}, and the data file here is {core}."
                    ),
                )
                .with_hint(format!(
                    "Run `wenmar-open data pull {matched}` first, or leave the version out to get the newest shop data for {core}."
                )));
            }
            Ok(release)
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
            let candidates = shop_releases(&releases);
            if candidates.is_empty() {
                return Err(CliError::new(
                    NOT_FOUND,
                    "No shop data release has been published yet.",
                ));
            }
            for release in candidates.into_iter().take(MOST_MANIFESTS) {
                if matched_against(&release)? == core {
                    return Ok(release);
                }
            }
            Err(CliError::new(
                NOT_FOUND,
                format!("No shop data release goes with data version {core}."),
            )
            .with_hint(
                "Run `wenmar-open data pull` to update the data file, then `wenmar-open data pull --shop` again.",
            ))
        }
    }
}

/// Checks that the unpacked file is the shop file the release says it is,
/// and that it goes with the data file in place.
fn verify_shop(part: &Path, release: &ShopRelease, core: &str) -> Result<(), CliError> {
    let status = data::inspect_shop(part, Some(core));
    let refuse = |problem: &str| {
        CliError::new(
            DATA_INVALID,
            format!("The downloaded file cannot be used: {problem}."),
        )
    };
    if let Some(problem) = &status.problem {
        return Err(refuse(problem));
    }
    if status.data_version.as_deref() != Some(release.version.as_str()) {
        return Err(refuse(&format!(
            "it says it is shop data {}, and the release is {}",
            status.data_version.as_deref().unwrap_or("unknown"),
            release.version
        )));
    }
    Ok(())
}

/// Downloads a shop release into `directory` and puts it in place, beside
/// the data file it was matched against.
pub fn pull_shop(
    directory: &Path,
    releases_url: &str,
    version: Option<&str>,
    force: bool,
) -> Result<Pulled, CliError> {
    pull_shop_with(directory, releases_url, version, force, Patience::default())
}

/// [`pull_shop`], waiting for the download only as long as `patience` says.
pub fn pull_shop_with(
    directory: &Path,
    releases_url: &str,
    version: Option<&str>,
    force: bool,
    patience: Patience,
) -> Result<Pulled, CliError> {
    // A shop file holds the catalog ids of one data file, so the data file
    // comes first.
    let core = data::inspect(&directory.join(DATA_FILE));
    let Some(core_version) = core.data_version.as_deref().filter(|_| core.usable) else {
        return Err(CliError::new(
            crate::error::NO_DATA,
            format!(
                "The shop data goes with a data file, and there is no usable data file at {}.",
                core.path
            ),
        )
        .with_hint("Run `wenmar-open data pull`, then `wenmar-open data pull --shop`."));
    };
    let path = directory.join(SHOP_FILE);
    let release = find_shop(releases_url, version, core_version)?;
    let before = data::inspect_shop(&path, Some(core_version));
    if !force && before.usable && before.data_version.as_deref() == Some(release.version.as_str()) {
        return Ok(Pulled {
            updated: false,
            data_version: release.version,
            previous: None,
            path: before.path,
            bytes: before.bytes,
        });
    }
    remove_stale_parts(directory, SHOP_FILE);

    let part = Part(directory.join(format!("{SHOP_FILE}.{}.part", std::process::id())));
    let bytes = download(&release.url, &part.0, patience)?;
    verify_shop(&part.0, &release, core_version)?;
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

    fn shop_release(tag: &str, version: &str) -> Value {
        json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": false,
            "assets": [
                { "name": format!("wenmar-open-shop-{version}.sqlite3.gz.sha256"), "browser_download_url": "https://example.test/sum" },
                { "name": format!("wenmar-open-shop-{version}.json"), "browser_download_url": format!("https://example.test/{version}.json") },
                { "name": format!("wenmar-open-shop-{version}.sqlite3.gz"), "browser_download_url": format!("https://example.test/{version}.gz") }
            ]
        })
    }

    #[test]
    fn a_shop_version_is_a_day_and_perhaps_a_rebuild() {
        assert_eq!(shop_version_numbers("2026.10.12"), Some(vec![2026, 10, 12]));
        assert_eq!(
            shop_version_numbers("2026.10.12.1"),
            Some(vec![2026, 10, 12, 1])
        );
        for text in [
            "",
            "2026.10",
            "2026.10.1",
            "2026.1.12",
            "26.10.12",
            "2026.10.12.",
            "2026.10.12.1.2",
            "2026.10.1a",
            "../x",
            "2026.10.12/..",
            " 2026.10.12",
        ] {
            assert_eq!(shop_version_numbers(text), None, "{text:?}");
        }
    }

    #[test]
    fn shop_releases_are_listed_newest_first_and_data_releases_are_not_among_them() {
        let mut no_manifest = shop_release("shop-2026.11.02", "2026.11.02");
        no_manifest["assets"] = json!([no_manifest["assets"][2].clone()]);
        let mut draft = shop_release("shop-2026.12.01", "2026.12.01");
        draft["draft"] = json!(true);
        let releases = json!([
            release("data-2026.09", "wenmar-open-2026.09.sqlite3.gz"),
            shop_release("shop-2026.10.05", "2026.10.05"),
            shop_release("shop-2026.10.12.2", "2026.10.12.2"),
            shop_release("shop-2026.10.12", "2026.10.12"),
            shop_release("shop-2026.10.12.10", "2026.10.12.10"),
            shop_release("shop-nonsense", "nonsense"),
            shop_release("shop-2026.10.19", "2026.10.20"),
            no_manifest,
            draft,
            json!(null)
        ]);
        let versions: Vec<String> = shop_releases(&releases)
            .into_iter()
            .map(|release| release.version)
            .collect();
        assert_eq!(
            versions,
            ["2026.10.12.10", "2026.10.12.2", "2026.10.12", "2026.10.05"]
        );
        let all = shop_releases(&releases);
        assert_eq!(all[0].url, "https://example.test/2026.10.12.10.gz");
        assert_eq!(all[0].manifest, "https://example.test/2026.10.12.10.json");
        // The data file's own list does not take a shop release for one.
        assert_eq!(newest(&releases).unwrap().version, "2026.09");
        assert!(shop_releases(&json!({ "message": "Not Found" })).is_empty());
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
