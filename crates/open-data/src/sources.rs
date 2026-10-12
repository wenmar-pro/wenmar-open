//! The files each outside source publishes, and where a fetch puts them.
//!
//! vPIC is not here: its release has to be found on a page first, and
//! `fetch.rs` does that.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// NHTSA's and Transport Canada's file servers refuse the default agent of
/// an HTTP library, so requests name a browser.
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// One file of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchFile {
    pub url: &'static str,
    /// The name it is saved under.
    pub name: &'static str,
}

/// A source: an id and the files it publishes at fixed addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchSource {
    pub id: &'static str,
    pub files: &'static [FetchFile],
}

/// Every source `open-data fetch` knows besides vPIC. A new source is one
/// more entry here.
pub const SOURCES: &[FetchSource] = &[
    FetchSource {
        id: "nhtsa-recalls",
        files: &[
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/FLAT_RCL_PRE_2010.zip",
                name: "FLAT_RCL_PRE_2010.zip",
            },
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/FLAT_RCL_POST_2010.zip",
                name: "FLAT_RCL_POST_2010.zip",
            },
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/RCL.txt",
                name: "RCL.txt",
            },
        ],
    },
    FetchSource {
        id: "tc-recalls",
        files: &[FetchFile {
            url: "https://opendatatc.tc.canada.ca/vrdb_full_monthly.csv",
            name: "vrdb_full_monthly.csv",
        }],
    },
    FetchSource {
        id: "nhtsa-investigations",
        files: &[
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/inv/FLAT_INV.zip",
                name: "FLAT_INV.zip",
            },
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/inv/INV.txt",
                name: "INV.txt",
            },
        ],
    },
    FetchSource {
        id: "nhtsa-recall-reports",
        files: &[
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/FLAT_RCL_Qrtly_Rpts.zip",
                name: "FLAT_RCL_Qrtly_Rpts.zip",
            },
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/FLAT_RCL_Annual_Rpts.zip",
                name: "FLAT_RCL_Annual_Rpts.zip",
            },
            // NHTSA spells the quarterly layout's name "Qtrly" and the
            // archive's "Qrtly".
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/RCL_Qtrly_Rpts.txt",
                name: "RCL_Qtrly_Rpts.txt",
            },
            FetchFile {
                url: "https://static.nhtsa.gov/odi/ffdd/rcl/RCL_Annual_Rpts.txt",
                name: "RCL_Annual_Rpts.txt",
            },
        ],
    },
    FetchSource {
        id: "nhtsa-ratings",
        files: &[
            FetchFile {
                url: "https://static.nhtsa.gov/nhtsa/downloads/Safercar/Safercar_data.csv",
                name: "Safercar_data.csv",
            },
            FetchFile {
                url: "https://static.nhtsa.gov/nhtsa/downloads/Safercar/Safercar_data_READ_ME_file.txt",
                name: "Safercar_data_READ_ME_file.txt",
            },
        ],
    },
    FetchSource {
        id: "epa-vehicles",
        files: &[FetchFile {
            url: "https://www.fueleconomy.gov/feg/epadata/vehicles.csv.zip",
            name: "vehicles.csv.zip",
        }],
    },
    FetchSource {
        id: "epa-emissions",
        files: &[FetchFile {
            url: "https://www.fueleconomy.gov/feg/epadata/emissions.csv.zip",
            name: "emissions.csv.zip",
        }],
    },
];

pub fn find(id: &str) -> Option<&'static FetchSource> {
    SOURCES.iter().find(|source| source.id == id)
}

pub fn ids() -> Vec<&'static str> {
    SOURCES.iter().map(|source| source.id).collect()
}

/// Where a source's files go: `<build_dir>/sources/<id>`.
pub fn directory(build_dir: &Path, id: &str) -> PathBuf {
    build_dir.join("sources").join(id)
}

/// The date of an HTTP `Last-Modified` header, as `2026-10-11`.
pub fn http_date(header: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = header.split_whitespace();
    let _weekday = parts.next()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let month_name = parts.next()?;
    let month = MONTHS.iter().position(|name| *name == month_name)? + 1;
    let year: u32 = parts.next()?.parse().ok()?;
    ((1..=31).contains(&day) && (1970..=9999).contains(&year))
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

/// One downloaded file, as `source.json` records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub name: String,
    pub url: String,
    /// The `Last-Modified` header as the server sent it.
    pub last_modified: Option<String>,
    pub bytes: u64,
}

/// `source.json`: where and when a source's files were fetched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub source: String,
    /// UTC, `YYYY-MM-DD HH:MM:SS`.
    pub fetched_at: String,
    /// The newest `Last-Modified` date among the files, or the day fetched.
    pub as_of: String,
    pub files: Vec<FileRecord>,
}

impl SourceRecord {
    /// Reads `source.json` from a source's directory.
    pub fn read(directory: &Path) -> Result<SourceRecord> {
        let path = directory.join("source.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}; run open-data fetch first", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
    }
}

/// What a download reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    pub last_modified: Option<String>,
    pub bytes: u64,
}

/// Downloads `url` to `path`.
pub fn download(url: &str, path: &Path) -> Result<Downloaded> {
    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("requesting {url}"))?;
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let bytes = io::copy(&mut response.body_mut().as_reader(), &mut file)
        .with_context(|| format!("downloading {url}"))?;
    Ok(Downloaded {
        last_modified,
        bytes,
    })
}

/// Fetches every file of `source` through `download` and writes
/// `source.json`. Returns the files' paths in order, then `source.json`.
pub fn fetch_with(
    source: &FetchSource,
    build_dir: &Path,
    fetched_at: &str,
    download: &mut dyn FnMut(&str, &Path) -> Result<Downloaded>,
) -> Result<Vec<PathBuf>> {
    let files: Vec<(String, String)> = source
        .files
        .iter()
        .map(|file| (file.url.to_owned(), file.name.to_owned()))
        .collect();
    fetch_files(source.id, &files, build_dir, fetched_at, download)
}

/// Fetches `files`, each an address and the name it is saved under, into
/// the directory of the source `id`, and writes `source.json`.
pub fn fetch_files(
    id: &str,
    files: &[(String, String)],
    build_dir: &Path,
    fetched_at: &str,
    download: &mut dyn FnMut(&str, &Path) -> Result<Downloaded>,
) -> Result<Vec<PathBuf>> {
    let directory = directory(build_dir, id);
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    let mut paths = Vec::with_capacity(files.len() + 1);
    let mut records = Vec::with_capacity(files.len());
    for (url, name) in files {
        let path = directory.join(name);
        // A download that stops half way must not leave a file that looks
        // whole, so it lands under another name first.
        let part = directory.join(format!("{name}.part"));
        let downloaded = download(url, &part)?;
        std::fs::rename(&part, &path).with_context(|| format!("writing {}", path.display()))?;
        records.push(FileRecord {
            name: name.clone(),
            url: url.clone(),
            last_modified: downloaded.last_modified,
            bytes: downloaded.bytes,
        });
        paths.push(path);
    }
    let as_of = records
        .iter()
        .filter_map(|file| file.last_modified.as_deref().and_then(http_date))
        .max()
        .unwrap_or_else(|| fetched_at.chars().take(10).collect());
    let record = SourceRecord {
        source: id.to_owned(),
        fetched_at: fetched_at.to_owned(),
        as_of,
        files: records,
    };
    let record_path = directory.join("source.json");
    let mut text = serde_json::to_string_pretty(&record)?;
    text.push('\n');
    std::fs::write(&record_path, text)
        .with_context(|| format!("writing {}", record_path.display()))?;
    paths.push(record_path);
    Ok(paths)
}

/// Fetches every file of `source` from the network.
pub fn fetch(source: &FetchSource, build_dir: &Path, fetched_at: &str) -> Result<Vec<PathBuf>> {
    fetch_with(source, build_dir, fetched_at, &mut download)
}

/// NHTSA's manufacturer communications: service bulletins and the like.
/// Its archives are named for the years they hold, and the newest one's
/// name changes each January, so the names are found by asking for them
/// and are not in [`SOURCES`].
pub const MFR_COMMS: &str = "nhtsa-mfr-comms";

/// Where the archives and the layout file are.
pub const MFR_COMMS_BASE: &str = "https://static.nhtsa.gov/odi/ffdd/tsbs/";

/// NHTSA's description of the fields.
pub const MFR_COMMS_LAYOUT: &str = "TSBS.txt";

/// The first year of the first archive, and how many years an archive
/// holds once it is complete.
pub const MFR_COMMS_FIRST_YEAR: u32 = 1995;
pub const MFR_COMMS_BLOCK: u32 = 5;

/// The name of the archive that holds the years `from` to `to`.
pub fn mfr_comms_archive(from: u32, to: u32) -> String {
    format!("TSBS_RECEIVED_{from}-{to}.zip")
}

/// The last years to try for the archive that starts in `from`, in order,
/// when the current year is `year`.
///
/// A block that ended before this year has one name: its five years. The
/// block this year is in is named for the newest year it holds so far, so
/// it is tried under next year's name (NHTSA may rename it early), then
/// this year's, then last year's (NHTSA may rename it late). An older
/// file is left on the server under its old name, so the newest is tried
/// first.
pub fn mfr_comms_candidates(from: u32, year: u32) -> Vec<u32> {
    let last = from + MFR_COMMS_BLOCK - 1;
    if last < year {
        return vec![last];
    }
    [year + 1, year, year.saturating_sub(1)]
        .into_iter()
        .filter(|end| (from..=last).contains(end))
        .collect()
}

/// The archives to download when the current year is `year`, oldest
/// first. `exists` says whether an address answers.
///
/// Fails when a block has no archive under any name tried, naming them.
/// The one exception is a block that starts this year: early in January
/// NHTSA may not have published it yet, and it is left out.
pub fn mfr_comms_archives(
    year: u32,
    exists: &mut dyn FnMut(&str) -> Result<bool>,
) -> Result<Vec<String>> {
    if year < MFR_COMMS_FIRST_YEAR {
        bail!("the year {year} is before the first archive, {MFR_COMMS_FIRST_YEAR}");
    }
    let mut found = Vec::new();
    let mut from = MFR_COMMS_FIRST_YEAR;
    while from <= year {
        let mut tried = Vec::new();
        let mut name = None;
        for to in mfr_comms_candidates(from, year) {
            let candidate = mfr_comms_archive(from, to);
            if exists(&format!("{MFR_COMMS_BASE}{candidate}"))? {
                name = Some(candidate);
                break;
            }
            tried.push(candidate);
        }
        match name {
            Some(name) => found.push(name),
            None if from == year => {}
            None => bail!(
                "no archive for the years from {from} at {MFR_COMMS_BASE}; tried {}",
                tried.join(", ")
            ),
        }
        from += MFR_COMMS_BLOCK;
    }
    Ok(found)
}

/// Whether `url` answers: `true` for success, `false` for 404, and an
/// error for anything else.
pub fn exists(url: &str) -> Result<bool> {
    match ureq::head(url).header("User-Agent", USER_AGENT).call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::StatusCode(404)) => Ok(false),
        Err(error) => Err(error).with_context(|| format!("asking for {url}")),
    }
}

/// Finds the manufacturer communication archives through `exists`, then
/// fetches them and the layout file through `download` and writes
/// `source.json`. Nothing is downloaded unless every archive was found.
pub fn fetch_mfr_comms_with(
    build_dir: &Path,
    fetched_at: &str,
    exists: &mut dyn FnMut(&str) -> Result<bool>,
    download: &mut dyn FnMut(&str, &Path) -> Result<Downloaded>,
) -> Result<Vec<PathBuf>> {
    let year: u32 = fetched_at
        .get(0..4)
        .and_then(|year| year.parse().ok())
        .with_context(|| format!("{fetched_at:?} does not start with a year"))?;
    let mut files: Vec<(String, String)> = mfr_comms_archives(year, exists)?
        .into_iter()
        .map(|name| (format!("{MFR_COMMS_BASE}{name}"), name))
        .collect();
    files.push((
        format!("{MFR_COMMS_BASE}{MFR_COMMS_LAYOUT}"),
        MFR_COMMS_LAYOUT.to_owned(),
    ));
    fetch_files(MFR_COMMS, &files, build_dir, fetched_at, download)
}

/// Fetches the manufacturer communications from the network.
pub fn fetch_mfr_comms(build_dir: &Path, fetched_at: &str) -> Result<Vec<PathBuf>> {
    fetch_mfr_comms_with(build_dir, fetched_at, &mut exists, &mut download)
}

/// Every source id `open-data fetch` takes besides `vpic`.
pub fn all_ids() -> Vec<&'static str> {
    let mut ids = ids();
    ids.push(MFR_COMMS);
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory under the system's temporary one, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let path = std::env::temp_dir()
                .join(format!("open-data-sources-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_sources_are_the_ones_the_roadmap_names() {
        assert_eq!(
            ids(),
            [
                "nhtsa-recalls",
                "tc-recalls",
                "nhtsa-investigations",
                "nhtsa-recall-reports",
                "nhtsa-ratings",
                "epa-vehicles",
                "epa-emissions"
            ]
        );
        let names = |id: &str| -> Vec<&str> {
            find(id)
                .unwrap()
                .files
                .iter()
                .map(|file| file.name)
                .collect()
        };
        assert_eq!(
            names("nhtsa-recalls"),
            ["FLAT_RCL_PRE_2010.zip", "FLAT_RCL_POST_2010.zip", "RCL.txt"]
        );
        assert_eq!(names("tc-recalls"), ["vrdb_full_monthly.csv"]);
        assert_eq!(
            names("nhtsa-ratings"),
            ["Safercar_data.csv", "Safercar_data_READ_ME_file.txt"]
        );
        assert_eq!(names("nhtsa-investigations"), ["FLAT_INV.zip", "INV.txt"]);
        assert_eq!(names("epa-vehicles"), ["vehicles.csv.zip"]);
        assert_eq!(names("epa-emissions"), ["emissions.csv.zip"]);
        assert_eq!(
            names("nhtsa-recall-reports"),
            [
                "FLAT_RCL_Qrtly_Rpts.zip",
                "FLAT_RCL_Annual_Rpts.zip",
                "RCL_Qtrly_Rpts.txt",
                "RCL_Annual_Rpts.txt"
            ]
        );
        assert!(find("vpic").is_none());
        assert!(find("").is_none());
        for source in SOURCES {
            for file in source.files {
                assert!(file.url.starts_with("https://"), "{}", file.url);
                assert!(file.url.ends_with(file.name), "{}", file.url);
            }
        }
    }

    #[test]
    fn a_last_modified_header_becomes_a_date() {
        assert_eq!(
            http_date("Sun, 11 Oct 2026 07:12:44 GMT").as_deref(),
            Some("2026-10-11")
        );
        assert_eq!(
            http_date("Thu, 01 Jan 2026 00:00:00 GMT").as_deref(),
            Some("2026-01-01")
        );
        for bad in [
            "",
            "yesterday",
            "Sun, 11 Okt 2026 07:12:44 GMT",
            "Sun, 41 Oct 2026",
            "11 Oct 2026",
        ] {
            assert_eq!(http_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_fetch_saves_each_file_and_records_where_it_came_from() {
        let scratch = Scratch::new("fetch");
        let source = find("nhtsa-recalls").unwrap();
        let mut asked = Vec::new();
        let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
            asked.push(url.to_owned());
            assert!(
                path.to_string_lossy().ends_with(".part"),
                "{}",
                path.display()
            );
            std::fs::write(path, url.as_bytes())?;
            Ok(Downloaded {
                last_modified: url
                    .ends_with("POST_2010.zip")
                    .then(|| "Sun, 11 Oct 2026 07:12:44 GMT".to_owned()),
                bytes: url.len() as u64,
            })
        };
        let paths = fetch_with(source, &scratch.0, "2026-10-12 14:03:22", &mut fake).unwrap();
        let directory = scratch.0.join("sources").join("nhtsa-recalls");
        assert_eq!(
            paths,
            [
                directory.join("FLAT_RCL_PRE_2010.zip"),
                directory.join("FLAT_RCL_POST_2010.zip"),
                directory.join("RCL.txt"),
                directory.join("source.json"),
            ]
        );
        assert_eq!(asked.len(), 3);
        assert_eq!(
            std::fs::read_to_string(directory.join("RCL.txt")).unwrap(),
            "https://static.nhtsa.gov/odi/ffdd/rcl/RCL.txt"
        );
        // Nothing is left under its temporary name.
        let mut left: Vec<String> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "FLAT_RCL_POST_2010.zip",
                "FLAT_RCL_PRE_2010.zip",
                "RCL.txt",
                "source.json"
            ]
        );
        let record = SourceRecord::read(&directory).unwrap();
        assert_eq!(record.source, "nhtsa-recalls");
        assert_eq!(record.fetched_at, "2026-10-12 14:03:22");
        assert_eq!(record.as_of, "2026-10-11");
        assert_eq!(record.files.len(), 3);
        assert_eq!(record.files[0].name, "FLAT_RCL_PRE_2010.zip");
        assert_eq!(record.files[0].last_modified, None);
        assert_eq!(
            record.files[1].last_modified.as_deref(),
            Some("Sun, 11 Oct 2026 07:12:44 GMT")
        );
        assert_eq!(record.files[1].bytes, record.files[1].url.len() as u64);
    }

    #[test]
    fn with_no_last_modified_the_day_fetched_stands_in() {
        let scratch = Scratch::new("no-header");
        let source = find("tc-recalls").unwrap();
        let mut fake = |_: &str, path: &Path| -> Result<Downloaded> {
            std::fs::write(path, b"x")?;
            Ok(Downloaded {
                last_modified: None,
                bytes: 1,
            })
        };
        fetch_with(source, &scratch.0, "2026-10-12 14:03:22", &mut fake).unwrap();
        let record = SourceRecord::read(&directory(&scratch.0, "tc-recalls")).unwrap();
        assert_eq!(record.as_of, "2026-10-12");
    }

    #[test]
    fn a_failed_download_leaves_no_finished_file_and_no_record() {
        let scratch = Scratch::new("failed");
        let source = find("tc-recalls").unwrap();
        let mut fake = |_: &str, path: &Path| -> Result<Downloaded> {
            std::fs::write(path, b"half")?;
            anyhow::bail!("connection reset")
        };
        let error = fetch_with(source, &scratch.0, "2026-10-12 14:03:22", &mut fake).unwrap_err();
        assert!(format!("{error:#}").contains("connection reset"));
        let directory = directory(&scratch.0, "tc-recalls");
        assert!(!directory.join("vrdb_full_monthly.csv").exists());
        assert!(!directory.join("source.json").exists());
        let error = SourceRecord::read(&directory).unwrap_err();
        assert!(format!("{error:#}").contains("run open-data fetch first"));
    }

    #[test]
    fn the_archive_names_tried_follow_the_calendar() {
        assert_eq!(mfr_comms_archive(2025, 2026), "TSBS_RECEIVED_2025-2026.zip");
        // A block that has ended has one name.
        assert_eq!(mfr_comms_candidates(1995, 2026), [1999]);
        assert_eq!(mfr_comms_candidates(2020, 2026), [2024]);
        assert_eq!(mfr_comms_candidates(2025, 2030), [2029]);
        // The block this year is in: next year's name, this year's, last year's.
        assert_eq!(mfr_comms_candidates(2025, 2026), [2027, 2026, 2025]);
        assert_eq!(mfr_comms_candidates(2025, 2025), [2026, 2025]);
        assert_eq!(mfr_comms_candidates(2025, 2029), [2029, 2028]);
        assert_eq!(mfr_comms_candidates(2030, 2030), [2031, 2030]);
    }

    /// The names on NHTSA's server on 2026-10-12. `2025-2025` was still
    /// there, last changed on 2025-12-31.
    const ON_SERVER: [&str; 8] = [
        "TSBS_RECEIVED_1995-1999.zip",
        "TSBS_RECEIVED_2000-2004.zip",
        "TSBS_RECEIVED_2005-2009.zip",
        "TSBS_RECEIVED_2010-2014.zip",
        "TSBS_RECEIVED_2015-2019.zip",
        "TSBS_RECEIVED_2020-2024.zip",
        "TSBS_RECEIVED_2025-2025.zip",
        "TSBS_RECEIVED_2025-2026.zip",
    ];

    fn on_server(url: &str) -> Result<bool> {
        Ok(ON_SERVER
            .iter()
            .any(|name| url == format!("{MFR_COMMS_BASE}{name}")))
    }

    #[test]
    fn the_archives_are_found_by_asking() {
        let mut asked = Vec::new();
        let mut exists = |url: &str| {
            asked.push(url.to_owned());
            on_server(url)
        };
        let found = mfr_comms_archives(2026, &mut exists).unwrap();
        assert_eq!(
            found,
            [
                "TSBS_RECEIVED_1995-1999.zip",
                "TSBS_RECEIVED_2000-2004.zip",
                "TSBS_RECEIVED_2005-2009.zip",
                "TSBS_RECEIVED_2010-2014.zip",
                "TSBS_RECEIVED_2015-2019.zip",
                "TSBS_RECEIVED_2020-2024.zip",
                "TSBS_RECEIVED_2025-2026.zip",
            ]
        );
        // Six finished blocks, one question each; then 2025-2027 and 2025-2026.
        assert_eq!(asked.len(), 8);
        assert!(asked[6].ends_with("TSBS_RECEIVED_2025-2027.zip"));

        // In January, before NHTSA renames the newest archive.
        let found = mfr_comms_archives(2027, &mut on_server).unwrap();
        assert_eq!(found.last().unwrap(), "TSBS_RECEIVED_2025-2026.zip");
        // The stale 2025-2025 is never taken while a newer name answers.
        assert!(!found.contains(&"TSBS_RECEIVED_2025-2025.zip".to_owned()));

        // A block that starts this year may not be published yet.
        let mut early =
            |url: &str| Ok(on_server(url)? || url.ends_with("TSBS_RECEIVED_2025-2029.zip"));
        let found = mfr_comms_archives(2030, &mut early).unwrap();
        assert_eq!(found.len(), 7);
        assert_eq!(found.last().unwrap(), "TSBS_RECEIVED_2025-2029.zip");
    }

    #[test]
    fn a_missing_archive_stops_the_fetch_before_any_download() {
        let scratch = Scratch::new("mfr-comms-missing");
        let mut exists = |url: &str| Ok(on_server(url)? && !url.contains("2010-2014"));
        let downloads = std::cell::Cell::new(0);
        let mut fake = |_: &str, path: &Path| -> Result<Downloaded> {
            downloads.set(downloads.get() + 1);
            std::fs::write(path, b"x")?;
            Ok(Downloaded {
                last_modified: None,
                bytes: 1,
            })
        };
        let error = fetch_mfr_comms_with(&scratch.0, "2026-10-12 14:03:22", &mut exists, &mut fake)
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "no archive for the years from 2010 at https://static.nhtsa.gov/odi/ffdd/tsbs/; tried TSBS_RECEIVED_2010-2014.zip"
        );
        assert_eq!(downloads.get(), 0);
        assert!(
            !directory(&scratch.0, MFR_COMMS)
                .join("source.json")
                .exists()
        );

        // A server that answers neither yes nor no stops it too.
        let mut broken = |_: &str| -> Result<bool> { anyhow::bail!("status 503") };
        let error = fetch_mfr_comms_with(&scratch.0, "2026-10-12 14:03:22", &mut broken, &mut fake)
            .unwrap_err();
        assert!(format!("{error:#}").contains("status 503"));
        let error = mfr_comms_archives(1990, &mut on_server).unwrap_err();
        assert!(format!("{error:#}").contains("before the first archive"));
    }

    #[test]
    fn a_fetch_of_the_communications_saves_the_archives_and_the_layout() {
        let scratch = Scratch::new("mfr-comms");
        let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
            std::fs::write(path, url.as_bytes())?;
            Ok(Downloaded {
                last_modified: Some("Sun, 11 Oct 2026 10:14:59 GMT".to_owned()),
                bytes: url.len() as u64,
            })
        };
        let paths =
            fetch_mfr_comms_with(&scratch.0, "2026-10-12 14:03:22", &mut on_server, &mut fake)
                .unwrap();
        let directory = directory(&scratch.0, MFR_COMMS);
        assert_eq!(paths.len(), 9);
        assert_eq!(paths[0], directory.join("TSBS_RECEIVED_1995-1999.zip"));
        assert_eq!(paths[6], directory.join("TSBS_RECEIVED_2025-2026.zip"));
        assert_eq!(paths[7], directory.join("TSBS.txt"));
        assert_eq!(paths[8], directory.join("source.json"));
        let record = SourceRecord::read(&directory).unwrap();
        assert_eq!(record.source, "nhtsa-mfr-comms");
        assert_eq!(record.as_of, "2026-10-11");
        assert_eq!(record.files.len(), 8);
        assert_eq!(
            record.files[6].url,
            "https://static.nhtsa.gov/odi/ffdd/tsbs/TSBS_RECEIVED_2025-2026.zip"
        );
        assert_eq!(record.files[7].name, "TSBS.txt");
        assert_eq!(
            all_ids(),
            [
                "nhtsa-recalls",
                "tc-recalls",
                "nhtsa-investigations",
                "nhtsa-recall-reports",
                "nhtsa-ratings",
                "epa-vehicles",
                "epa-emissions",
                "nhtsa-mfr-comms"
            ]
        );
        assert!(find(MFR_COMMS).is_none());
    }
}
