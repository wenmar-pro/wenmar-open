//! Asks for every stored link to a maker's site, one at a time, and
//! reports the ones that no longer answer.
//!
//! This is a report, never a gate. Some makers refuse any request a
//! script makes, so a link that could not be checked is told apart from
//! one that is dead: only an answer of "not found" (404) or "gone" (410)
//! counts as dead. Everything else, a refusal, a limit, a timeout, a host
//! name that does not resolve from here, is "could not be checked", and a
//! person opens it in a browser. Nothing is downloaded: a
//! request asks for the headers only, and where a server will not answer
//! that, for one byte, which is dropped unread.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Deserialize;
use ureq::ResponseExt as _;
use wenmar_vehicles::text::normalize;

use crate::shop::hand::load_manuals;
use crate::sources::USER_AGENT;

/// How long one request may take.
pub const TIMEOUT: Duration = Duration::from_secs(20);
/// How many model years an address pattern is tried on for each make.
pub const PATTERN_SAMPLES: usize = 3;

/// One address to ask for, and where it is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLink {
    /// `data/catalog/makes.yaml, Honda, manual_url`.
    pub place: String,
    pub url: String,
}

/// What a server said when asked for an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// It answered with this status, at this address after any redirect.
    Status { code: u16, landed: String },
    /// No answer: a timeout, a refused connection, a failed handshake, a
    /// host name that did not resolve.
    Failed(String),
}

/// What the report says of one link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It answered.
    Ok,
    /// It answered, at another address. A person should look.
    Moved(String),
    /// The server says it is not there.
    Dead(String),
    /// The server would not say. Not a finding against the link.
    Unchecked(String),
}

/// Reads one probe. `url` is the address asked for.
pub fn classify(url: &str, probe: &Probe) -> Outcome {
    match probe {
        Probe::Status { code, landed } if (200..300).contains(code) => {
            if landed.trim_end_matches('/') == url.trim_end_matches('/') {
                Outcome::Ok
            } else {
                Outcome::Moved(landed.clone())
            }
        }
        Probe::Status { code, .. } if matches!(code, 404 | 410) => {
            Outcome::Dead(format!("the server answered {code}"))
        }
        Probe::Status { code, .. } => Outcome::Unchecked(format!(
            "the server answered {code}; some makers refuse a request from a script"
        )),
        Probe::Failed(why) => Outcome::Unchecked(format!("no answer: {why}")),
    }
}

/// A make of `makes.yaml`, as far as its addresses go. Fields this does
/// not name are let through: the server's build is what checks the file.
#[derive(Deserialize)]
struct Make {
    name: String,
    recall_lookup_url: Option<String>,
    manual_url: Option<String>,
    maintenance_url: Option<String>,
    service_info_url: Option<String>,
    parts_url: Option<String>,
    manual_url_template: Option<String>,
    maintenance_url_template: Option<String>,
}

#[derive(Deserialize)]
struct MakesFile {
    makes: Vec<Make>,
}

/// The newest model years of a make's light models, as (year, id form of
/// the model), newest first: what an address pattern is tried on.
fn samples(catalog: &Connection, make: &str) -> Result<Vec<(u16, String)>> {
    let mut statement = catalog.prepare(
        "SELECT m.year_to, m.slug FROM catalog_model m
         JOIN catalog_make k ON k.id = m.make_id
         WHERE k.norm = ?1 AND m.light = 1
         ORDER BY m.year_to DESC, m.name LIMIT ?2",
    )?;
    let rows = statement.query_map(
        rusqlite::params![normalize(make), PATTERN_SAMPLES as i64],
        |row| Ok((row.get::<_, u16>(0)?, row.get::<_, String>(1)?)),
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Every address stored under `curated`: the makers' links of
/// `catalog/makes.yaml`, the links of `fluids/*.yaml`, and, when a
/// catalog is given, each address pattern filled in for a few model
/// years. An address stored twice is asked for once.
pub fn stored_links(curated: &Path, catalog: Option<&Connection>) -> Result<Vec<StoredLink>> {
    let path = curated.join("catalog/makes.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let file: MakesFile = serde_saphyr::from_str(&text).context("reading makes.yaml")?;
    let mut links = Vec::new();
    for make in &file.makes {
        for (field, value) in [
            ("recall_lookup_url", &make.recall_lookup_url),
            ("manual_url", &make.manual_url),
            ("maintenance_url", &make.maintenance_url),
            ("service_info_url", &make.service_info_url),
            ("parts_url", &make.parts_url),
        ] {
            if let Some(url) = value {
                links.push(StoredLink {
                    place: format!("catalog/makes.yaml, {}, {field}", make.name),
                    url: url.clone(),
                });
            }
        }
        let Some(catalog) = catalog else {
            continue;
        };
        for (field, value) in [
            ("manual_url_template", &make.manual_url_template),
            ("maintenance_url_template", &make.maintenance_url_template),
        ] {
            let Some(template) = value else {
                continue;
            };
            for (year, model) in samples(catalog, &make.name)? {
                links.push(StoredLink {
                    place: format!(
                        "catalog/makes.yaml, {}, {field} for {year} {model}",
                        make.name
                    ),
                    url: template
                        .replace("{year}", &year.to_string())
                        .replace("{model}", &model),
                });
            }
        }
    }
    for file in load_manuals(curated)? {
        for (index, record) in file.records.iter().enumerate() {
            links.push(StoredLink {
                place: format!("fluids/{}.yaml, manual {}", file.name, index + 1),
                url: record.url.clone(),
            });
        }
    }
    let mut seen = HashSet::new();
    links.retain(|link| seen.insert(link.url.clone()));
    Ok(links)
}

/// Asks for one address. Headers only; where the server answers that it
/// does not allow that (405 or 501), one byte is asked for and dropped.
pub fn probe(url: &str) -> Probe {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let read = |result: Result<ureq::http::Response<ureq::Body>, ureq::Error>| match result {
        Ok(response) => Probe::Status {
            code: response.status().as_u16(),
            landed: response.get_uri().to_string(),
        },
        Err(error) => Probe::Failed(error.to_string()),
    };
    let first = read(agent.head(url).header("User-Agent", USER_AGENT).call());
    match first {
        Probe::Status {
            code: 405 | 501, ..
        } => read(
            agent
                .get(url)
                .header("User-Agent", USER_AGENT)
                .header("Range", "bytes=0-0")
                .call(),
        ),
        other => other,
    }
}

/// Asks for every link in turn through `probe`, calling `pause` between
/// two requests. Never two at once, and never without a pause: a burst
/// is what gets a script refused.
pub fn check_all(
    links: &[StoredLink],
    probe: &mut dyn FnMut(&str) -> Probe,
    pause: &mut dyn FnMut(),
) -> Vec<(StoredLink, Outcome)> {
    let mut results = Vec::with_capacity(links.len());
    for (index, link) in links.iter().enumerate() {
        if index > 0 {
            pause();
        }
        let outcome = classify(&link.url, &probe(&link.url));
        results.push((link.clone(), outcome));
    }
    results
}

/// The report, as Markdown. `day` is the day it was made.
pub fn report(results: &[(StoredLink, Outcome)], day: &str) -> String {
    let count = |wanted: fn(&Outcome) -> bool| {
        results
            .iter()
            .filter(|(_, outcome)| wanted(outcome))
            .count()
    };
    let mut text = format!(
        "# Link report, {day}\n\n{} links asked for: {} answered, {} answered at another address, {} dead, {} could not be checked.\n",
        results.len(),
        count(|outcome| matches!(outcome, Outcome::Ok)),
        count(|outcome| matches!(outcome, Outcome::Moved(_))),
        count(|outcome| matches!(outcome, Outcome::Dead(_))),
        count(|outcome| matches!(outcome, Outcome::Unchecked(_))),
    );
    let mut section = |heading: &str, lead: &str, pick: fn(&Outcome) -> Option<&str>| {
        let rows: Vec<(&StoredLink, &str)> = results
            .iter()
            .filter_map(|(link, outcome)| pick(outcome).map(|why| (link, why)))
            .collect();
        if rows.is_empty() {
            return;
        }
        let _ = write!(text, "\n## {heading}\n\n{lead}\n\n");
        for (link, why) in rows {
            let _ = writeln!(text, "- {}: <{}> ({why})", link.place, link.url);
        }
    };
    section(
        "Dead",
        "A person should find the new address, or remove the link.",
        |outcome| match outcome {
            Outcome::Dead(why) => Some(why),
            _ => None,
        },
    );
    section(
        "Answered at another address",
        "The link still leads somewhere. A person should open it and see that it is the same page.",
        |outcome| match outcome {
            Outcome::Moved(landed) => Some(landed),
            _ => None,
        },
    );
    section(
        "Could not be checked",
        "The server would not answer a script. This says nothing against the link: a person opens these in a browser.",
        |outcome| match outcome {
            Outcome::Unchecked(why) => Some(why),
            _ => None,
        },
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(code: u16, landed: &str) -> Probe {
        Probe::Status {
            code,
            landed: landed.to_owned(),
        }
    }

    #[test]
    fn only_not_found_and_gone_are_dead() {
        let url = "https://example.org/a";
        assert_eq!(classify(url, &status(200, url)), Outcome::Ok);
        assert_eq!(
            classify(url, &status(206, "https://example.org/a/")),
            Outcome::Ok
        );
        assert_eq!(
            classify(url, &status(200, "https://example.org/home")),
            Outcome::Moved("https://example.org/home".to_owned())
        );
        for code in [404, 410] {
            assert_eq!(
                classify(url, &status(code, url)),
                Outcome::Dead(format!("the server answered {code}"))
            );
        }
        // A refusal, a limit, a server in trouble and silence are not findings.
        for code in [401, 403, 405, 429, 451, 500, 503] {
            assert!(
                matches!(classify(url, &status(code, url)), Outcome::Unchecked(why) if why.contains(&code.to_string())),
                "{code}"
            );
        }
        assert_eq!(
            classify(url, &Probe::Failed("timeout: global".to_owned())),
            Outcome::Unchecked("no answer: timeout: global".to_owned())
        );
    }

    #[test]
    fn links_are_asked_for_one_at_a_time_with_a_pause_between() {
        let links: Vec<StoredLink> = ["a", "b", "c"]
            .iter()
            .map(|name| StoredLink {
                place: format!("place {name}"),
                url: format!("https://example.org/{name}"),
            })
            .collect();
        let mut asked = Vec::new();
        let mut pauses = 0;
        let results = check_all(
            &links,
            &mut |url| {
                asked.push(url.to_owned());
                match url {
                    "https://example.org/a" => status(200, url),
                    "https://example.org/b" => status(404, url),
                    _ => status(403, url),
                }
            },
            &mut || pauses += 1,
        );
        assert_eq!(
            asked,
            [
                "https://example.org/a",
                "https://example.org/b",
                "https://example.org/c"
            ]
        );
        assert_eq!(pauses, 2);
        let text = report(&results, "2026-10-12");
        assert_eq!(
            text,
            "# Link report, 2026-10-12\n\n3 links asked for: 1 answered, 0 answered at another address, 1 dead, 1 could not be checked.\n\n## Dead\n\nA person should find the new address, or remove the link.\n\n- place b: <https://example.org/b> (the server answered 404)\n\n## Could not be checked\n\nThe server would not answer a script. This says nothing against the link: a person opens these in a browser.\n\n- place c: <https://example.org/c> (the server answered 403; some makers refuse a request from a script)\n"
        );
        // Nothing to say but the count when every link answers.
        let fine = check_all(&links[..1], &mut |url| status(200, url), &mut || {});
        assert_eq!(
            report(&fine, "2026-10-12"),
            "# Link report, 2026-10-12\n\n1 links asked for: 1 answered, 0 answered at another address, 0 dead, 0 could not be checked.\n"
        );
    }

    #[test]
    fn every_stored_address_is_found_once() {
        let directory =
            std::env::temp_dir().join(format!("open-data-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("catalog")).unwrap();
        std::fs::create_dir_all(directory.join("fluids")).unwrap();
        std::fs::write(
            directory.join("catalog/makes.yaml"),
            "makes:\n  - name: Toyota\n    aliases: [yota]\n  - name: Testmake\n    links_checked: \"2026-10-12\"\n    manual_url: https://example.org/manuals\n    parts_url: https://example.org/manuals\n    manual_url_template: https://example.org/owners/{year}/{model}\n    customer_line: \"1-800-555-0100\"\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("fluids/testmake_sampler.yaml"),
            "make: testmake\nmodel: sampler\nmanuals:\n  - year_from: 2001\n    kind: owner_manual\n    url: https://example.org/testmake/sampler/manual.pdf\n    checked: 2026-10-12\n    reviewer: test-reviewer\n",
        )
        .unwrap();
        let found = stored_links(&directory, None).unwrap();
        let lines: Vec<String> = found
            .iter()
            .map(|link| format!("{} -> {}", link.place, link.url))
            .collect();
        // The parts address repeats the manuals address and is asked for
        // once. Without a catalog a pattern is not tried.
        assert_eq!(
            lines,
            [
                "catalog/makes.yaml, Testmake, manual_url -> https://example.org/manuals",
                "fluids/testmake_sampler.yaml, manual 1 -> https://example.org/testmake/sampler/manual.pdf",
            ]
        );
        // With a catalog, the pattern is filled for the make's newest models.
        let catalog = Connection::open_in_memory().unwrap();
        catalog
            .execute_batch(wenmar_vehicles::schema::SCHEMA)
            .unwrap();
        catalog
            .execute_batch(
                "INSERT INTO catalog_make VALUES (7000, 'testmake', 'Testmake', 'testmake', NULL, 4, 1);
                 INSERT INTO catalog_model VALUES
                   (7001, 7000, 'sampler', 'Sampler', 'sampler', 2001, 2003, 4, 1),
                   (7002, 7000, 'other', 'Other', 'other', 2001, 2002, 4, 1),
                   (7003, 7000, 'heavy', 'Heavy', 'heavy', 2001, 2009, 8, 0);",
            )
            .unwrap();
        let found = stored_links(&directory, Some(&catalog)).unwrap();
        let patterns: Vec<&str> = found
            .iter()
            .filter(|link| link.place.contains("manual_url_template"))
            .map(|link| link.url.as_str())
            .collect();
        assert_eq!(
            patterns,
            [
                "https://example.org/owners/2003/sampler",
                "https://example.org/owners/2002/other"
            ]
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
