//! Finds and downloads NHTSA's vPIC plain-text dump.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub const DOWNLOADS_URL: &str = "https://vpic.nhtsa.dot.gov/downloads/";

const PREFIX: &str = "vPICList_lite_";
const SUFFIX: &str = ".plain.zip";

/// Whether `text` is a release name: four digits, an underscore, two digits.
fn is_release(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 7
        && bytes[4] == b'_'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..].iter().all(u8::is_ascii_digit)
}

/// The newest release named on NHTSA's downloads page, such as `2026_09`.
pub fn latest_release(page: &str) -> Option<String> {
    page.match_indices(PREFIX)
        .filter_map(|(index, _)| {
            let rest = &page[index + PREFIX.len()..];
            let release = rest.get(..7)?;
            (is_release(release) && rest[7..].starts_with(SUFFIX)).then(|| release.to_owned())
        })
        .max()
}

/// `2026_09` becomes `2026.09`.
pub fn data_version(release: &str) -> Option<String> {
    is_release(release).then(|| release.replace('_', "."))
}

/// Downloads a release into `directory` and extracts its `.sql` file.
pub fn download(release: &str, directory: &Path) -> Result<PathBuf> {
    if !is_release(release) {
        bail!("{release} is not a release name like 2026_09");
    }
    std::fs::create_dir_all(directory)?;
    let name = format!("{PREFIX}{release}");
    let archive_path = directory.join(format!("{name}{SUFFIX}"));
    let url = format!("{DOWNLOADS_URL}{name}{SUFFIX}");

    let mut response = ureq::get(&url)
        .call()
        .with_context(|| format!("requesting {url}"))?;
    let mut archive_file = File::create(&archive_path)?;
    io::copy(&mut response.body_mut().as_reader(), &mut archive_file)
        .with_context(|| format!("downloading {url}"))?;
    drop(archive_file);

    let mut archive = zip::ZipArchive::new(File::open(&archive_path)?)
        .with_context(|| format!("{} is not a zip file", archive_path.display()))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name().ends_with(".sql") {
            let sql_path = directory.join(format!("{name}.sql"));
            io::copy(&mut entry, &mut File::create(&sql_path)?)?;
            return Ok(sql_path);
        }
    }
    bail!("{} contains no .sql file", archive_path.display())
}

/// Fetches the downloads page.
pub fn downloads_page() -> Result<String> {
    ureq::get(DOWNLOADS_URL)
        .call()
        .with_context(|| format!("requesting {DOWNLOADS_URL}"))?
        .body_mut()
        .read_to_string()
        .context("reading the downloads page")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"
        <a href="/downloads/vPICList_lite_2026_08.plain.zip">August</a>
        <a href="/downloads/vPICList_lite_2026_09.bak.zip">SQL Server</a>
        <a href="/downloads/vPICList_lite_2026_09.custom.zip">custom</a>
        <a href="/downloads/vPICList_lite_2026_09.plain.zip">plain</a>
        <a href="/downloads/vPICList_lite_2025_12.plain.zip">December</a>
    "#;

    #[test]
    fn finds_the_newest_plain_text_release() {
        assert_eq!(latest_release(PAGE).as_deref(), Some("2026_09"));
    }

    #[test]
    fn a_page_with_no_releases_gives_none() {
        assert_eq!(latest_release("<html>maintenance</html>"), None);
        assert_eq!(latest_release("vPICList_lite_20XX_09.plain.zip"), None);
    }

    #[test]
    fn a_release_name_becomes_a_data_version() {
        assert_eq!(data_version("2026_09").as_deref(), Some("2026.09"));
        assert_eq!(data_version("nonsense"), None);
        assert_eq!(data_version("2026_9"), None);
    }
}
