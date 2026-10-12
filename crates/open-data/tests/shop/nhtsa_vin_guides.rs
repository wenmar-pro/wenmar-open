use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use open_data::shop::build::build_with;
use open_data::shop::nhtsa_vin_guides::{
    CAP, VinGuideCounts, VinGuideRow, fetch_with, parse, shape,
};
use open_data::sources::{Downloaded, SourceRecord};
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::vin_guides::is_document_url;
use wenmar_shopdata::{Dataset, ShopData, licence};

use crate::common::{Scratch, add_wmi_rows, info, nhtsa_vin_guides_dir, rows};

fn read(directory: &Path) -> Result<(Vec<VinGuideRow>, VinGuideCounts)> {
    let mut found = Vec::new();
    let counts = parse(directory, &mut |row| {
        found.push(row);
        Ok(())
    })?;
    Ok((found, counts))
}

#[test]
fn reads_every_answer_the_record_of_the_fetch_lists() {
    let scratch = Scratch::new("vin-guides-parse");
    let (found, counts) = read(&nhtsa_vin_guides_dir(&scratch)).unwrap();
    assert_eq!(
        counts,
        VinGuideCounts {
            files: 4,
            filings: 13
        }
    );
    // In the order of the files, and within a file as the API gave them.
    let names: Vec<&str> = found.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "01-022-N11B-1841",
            "01-22-N11B-1670",
            "01-22-N11B-1724",
            "01-22-N11B-1500",
            "01-022-N11B-10517",
            "ORG380",
            "ORG21569",
            "ORG27020",
            "ORG26817",
            "ORG26778",
            "ORG26610",
            "ORG26611",
            "ORG26392",
        ]
    );
    assert_eq!(
        found[10],
        VinGuideRow {
            name: "ORG26610".to_owned(),
            manufacturer_id: 19109,
            manufacturer: "SUBARU CORPORATION".to_owned(),
            letter_date: "2026-06-01".to_owned(),
            year_from: Some(2027),
            year_to: Some(2027),
            url: "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b"
                .to_owned(),
        }
    );
    // A name with a blank after it, a letter from 1964, a curly apostrophe
    // and a filing that gives only the first model year.
    assert_eq!(found[0].manufacturer, "R.V. CLASSICS, INC");
    assert_eq!(found[3].letter_date, "1964-03-09");
    assert_eq!(found[3].year_from, None);
    assert_eq!(found[4].manufacturer, "BILLY\u{2019}S HOT ROD CYCLES");
    assert_eq!((found[11].year_from, found[11].year_to), (Some(2026), None));
    for row in &found {
        assert!(is_document_url(&row.url), "{}", row.url);
    }
}

#[test]
fn refuses_a_directory_it_cannot_read() {
    let scratch = Scratch::new("vin-guides-refuse");
    let message = |directory: &Path| format!("{:#}", read(directory).unwrap_err());

    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        message(&empty).contains("run open-data fetch first"),
        "{}",
        message(&empty)
    );

    // An answer the record lists and the directory lacks.
    let directory = nhtsa_vin_guides_dir(&scratch);
    std::fs::remove_file(directory.join("565-2023-04.json")).unwrap();
    assert!(
        message(&directory)
            .contains("565-2023-04.json; run open-data fetch nhtsa-vin-guides first"),
        "{}",
        message(&directory)
    );

    // An answer with a field this build does not know.
    let directory = nhtsa_vin_guides_dir(&scratch);
    let path = directory.join("565-2023-04.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("\"Type\":", "\"Pages\":3,\"Type\":")).unwrap();
    let error = message(&directory);
    assert!(
        error.starts_with("565-2023-04.json is not an answer of GetParts as this build reads it")
            && error.contains("unknown field `Pages`"),
        "{error}"
    );

    // The directory of another source.
    let directory = nhtsa_vin_guides_dir(&scratch);
    let path = directory.join("source.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("nhtsa-vin-guides", "nhtsa-recalls")).unwrap();
    assert!(
        message(&directory).ends_with("holds the source nhtsa-recalls, expected nhtsa-vin-guides"),
        "{}",
        message(&directory)
    );
}

/// An answer of `GetParts` with `filings` filings in it.
fn answer(filings: usize) -> String {
    let results: Vec<String> = (0..filings)
        .map(|number| {
            format!(
                r#"{{"CoverLetterURL":"","LetterDate":"1/2/1986","ManufacturerId":976,"ManufacturerName":"FORD MOTOR COMPANY","ModelYearFrom":null,"ModelYearTo":null,"Name":"T{number}","Type":"565","URL":"http://vpic.nhtsa.dot.gov/mid/home/displayfile/aaa4a40c-9cf6-4cb2-b95a-{number:012}"}}"#
            )
        })
        .collect();
    format!(
        r#"{{"Count":{filings},"Message":"Results returned successfully","SearchCriteria":"Type: 565","Results":[{}]}}"#,
        results.join(",")
    )
}

/// What is in a directory, by name.
fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_fetch_asks_year_by_year_and_month_by_month_where_a_year_is_full() {
    let scratch = Scratch::new("vin-guides-fetch");
    let build_dir = scratch.path("build");
    let mut asked = Vec::new();
    // 1986 is the full year: its answer holds the most an answer can.
    let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
        asked.push(url.to_owned());
        assert!(
            path.to_string_lossy().ends_with(".part"),
            "{}",
            path.display()
        );
        let filings = if url.contains("fromDate=1/1/1986&toDate=12/31/1986") {
            CAP
        } else if url.contains("fromDate=2/1/1986") {
            CAP - 1
        } else {
            2
        };
        let text = answer(filings);
        std::fs::write(path, &text)?;
        Ok(Downloaded {
            last_modified: None,
            bytes: text.len() as u64,
        })
    };
    let paths = fetch_with(&build_dir, "1987-03-04 05:06:07", Duration::ZERO, &mut fake).unwrap();
    let directory = build_dir.join("sources").join("nhtsa-vin-guides");

    let range = |from: &str, to: &str| {
        format!(
            "https://vpic.nhtsa.dot.gov/api/vehicles/GetParts?type=565&fromDate={from}&toDate={to}&format=json"
        )
    };
    let mut expected = vec![
        range("1/1/1900", "12/31/1984"),
        range("1/1/1985", "12/31/1985"),
        range("1/1/1986", "12/31/1986"),
    ];
    for (month, last) in [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        .iter()
        .enumerate()
    {
        let month = month + 1;
        expected.push(range(
            &format!("{month}/1/1986"),
            &format!("{month}/{last}/1986"),
        ));
    }
    expected.push(range("1/1/1987", "12/31/1987"));
    assert_eq!(asked, expected);

    // The full year's own answer is not kept; its months are.
    let mut files = vec!["565-1900-1984.json".to_owned(), "565-1985.json".to_owned()];
    files.extend((1..=12).map(|month| format!("565-1986-{month:02}.json")));
    files.push("565-1987.json".to_owned());
    let mut on_disk = files.clone();
    on_disk.push("source.json".to_owned());
    on_disk.sort();
    assert_eq!(names(&directory), on_disk);
    let mut returned: Vec<_> = files.iter().map(|name| directory.join(name)).collect();
    returned.push(directory.join("source.json"));
    assert_eq!(paths, returned);

    let record = SourceRecord::read(&directory).unwrap();
    assert_eq!(record.source, "nhtsa-vin-guides");
    assert_eq!(record.fetched_at, "1987-03-04 05:06:07");
    assert_eq!(record.as_of, "1987-03-04");
    let listed: Vec<&str> = record.files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(listed, files);
    assert_eq!(record.files[2].url, range("1/1/1986", "1/31/1986"));

    // What was fetched is what the parser reads: 2 in each of the old
    // range, 1985, ten months of 1986 and 1987, 2 in January 1986, and 999
    // in February.
    let (found, counts) = read(&directory).unwrap();
    assert_eq!(counts.files, 15);
    assert_eq!(found.len(), 2 * 14 + (CAP - 1));
}

#[test]
fn a_fetch_fails_rather_than_keep_an_answer_that_may_lack_filings() {
    let scratch = Scratch::new("vin-guides-full");
    let failed = |name: &str, full: &'static str| -> (String, Vec<String>) {
        let build_dir = scratch.path(name);
        let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
            let filings = if url.contains(full) || url.contains("toDate=12/31/1985") {
                CAP
            } else {
                1
            };
            let text = answer(filings);
            std::fs::write(path, &text)?;
            Ok(Downloaded {
                last_modified: None,
                bytes: text.len() as u64,
            })
        };
        let error =
            fetch_with(&build_dir, "1985-06-01 00:00:00", Duration::ZERO, &mut fake).unwrap_err();
        (
            format!("{error:#}"),
            names(&build_dir.join("sources").join("nhtsa-vin-guides")),
        )
    };
    // A month that is full.
    let (error, left) = failed("build-month", "fromDate=3/1/1985");
    assert_eq!(
        error,
        "the answer for 3/1/1985 to 3/31/1985 holds 1000 filings, the most one answer can, so some may be missing; this build asks for nothing narrower"
    );
    // No record of the fetch, so no build reads what was saved before it
    // failed, and nothing under a temporary name.
    assert_eq!(
        left,
        ["565-1900-1984.json", "565-1985-01.json", "565-1985-02.json"]
    );
    // The range of old letters.
    let (error, left) = failed("build-old", "fromDate=1/1/1900");
    assert!(
        error.starts_with("the answer for 1/1/1900 to 12/31/1984 holds 1000 filings"),
        "{error}"
    );
    assert!(left.is_empty(), "{left:?}");

    // An answer that is not one, and a date the walk cannot start from.
    let build_dir = scratch.path("build-html");
    let mut fake = |_: &str, path: &Path| -> Result<Downloaded> {
        std::fs::write(path, "<html>Service unavailable</html>")?;
        Ok(Downloaded {
            last_modified: None,
            bytes: 32,
        })
    };
    let error =
        fetch_with(&build_dir, "1985-06-01 00:00:00", Duration::ZERO, &mut fake).unwrap_err();
    assert!(
        format!("{error:#}").contains("is not an answer of GetParts as this build reads it"),
        "{error:#}"
    );
    let error = fetch_with(&build_dir, "yesterday", Duration::ZERO, &mut fake).unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "\"yesterday\" does not start with a year"
    );
}

#[test]
fn filings_become_rows_and_reach_codes_and_makes_through_the_manufacturers_name() {
    let scratch = Scratch::new("vin-guides-shape");
    add_wmi_rows(&scratch);
    let directory = nhtsa_vin_guides_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::VinGuides).unwrap();
    assert_eq!(entry.as_of, "2026-10-12");
    assert_eq!(entry.rows, 13);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert_eq!(entry.attribution, "");

    let connection = Connection::open(scratch.out()).unwrap();
    // Oldest first, and two letters of one day by name.
    assert_eq!(
        rows(
            &connection,
            "SELECT id, name, manufacturer_id, manufacturer, kind, letter_date, year_from, year_to FROM vin_guide ORDER BY id"
        ),
        [
            "1|01-22-N11B-1500|4662|A.B. CHANCE COMPANY|565|1964-03-09|-|-",
            "2|01-22-N11B-1724|976|FORD MOTOR COMPANY|565|1984-05-30|-|-",
            "3|01-22-N11B-1670|976|FORD MOTOR COMPANY|565|1984-10-09|-|-",
            "4|01-022-N11B-1841|15309|R.V. CLASSICS, INC|565|1984-12-31|-|-",
            "5|01-022-N11B-10517|9255|BILLY\u{2019}S HOT ROD CYCLES|565|2005-04-18|-|-",
            "6|ORG380|9255|BILLY\u{2019}S HOT ROD CYCLES|565|2005-04-18|-|-",
            "7|ORG21569|958|JAGUAR LAND ROVER NA, LLC|565|2023-04-03|2024|2024",
            "8|ORG26392|976|FORD MOTOR COMPANY|565|2026-03-26|2027|2027",
            "9|ORG26610|19109|SUBARU CORPORATION|565|2026-06-01|2027|2027",
            "10|ORG26611|1053|PETE'S GOLF CARTS, LLC|565|2026-06-01|2026|-",
            "11|ORG26778|988|AMERICAN HONDA MOTOR CO., INC.|565|2026-07-27|2026|2026",
            "12|ORG26817|976|FORD MOTOR COMPANY|565|2026-08-06|2027|2027",
            "13|ORG27020|987|HONDA MOTOR CO., LTD.|565|2026-10-06|2027|2027",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT url FROM vin_guide WHERE name = 'ORG26610'"
        ),
        ["https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b"]
    );
    // Four of the eight manufacturers are in the core file by name.
    assert_eq!(
        rows(
            &connection,
            "SELECT wmi, manufacturer_id FROM vin_guide_wmi ORDER BY wmi"
        ),
        ["19X|988", "1FM|976", "4F2|976", "JF1|19109", "JHM|987"]
    );
    // Ford's letters reach Ford (460) and Mazda (473), which it built
    // under 4F2; both Honda companies reach Honda (474); Subaru's reach
    // Toyota (448), listed under JF1.
    assert_eq!(
        rows(
            &connection,
            "SELECT make_id, manufacturer_id FROM vin_guide_make ORDER BY make_id, manufacturer_id"
        ),
        ["448|19109", "460|976", "473|976", "474|987", "474|988"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_master WHERE name LIKE 'vin_guide%' ORDER BY name"
        ),
        [
            "vin_guide",
            "vin_guide_make",
            "vin_guide_manufacturer",
            "vin_guide_wmi"
        ]
    );
    drop(connection);

    // The readers give a make's and a code's filings newest first.
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    let honda = shop.vin_guides_for_make(474, 10).unwrap();
    assert_eq!(honda.total, 2);
    let listed: Vec<(&str, &str)> = honda
        .guides
        .iter()
        .map(|guide| (guide.name.as_str(), guide.manufacturer.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("ORG27020", "HONDA MOTOR CO., LTD."),
            ("ORG26778", "AMERICAN HONDA MOTOR CO., INC.")
        ]
    );
    let ford = shop.vin_guides_for_wmi("1FM", 3).unwrap();
    assert_eq!(ford.total, 4);
    let listed: Vec<&str> = ford
        .guides
        .iter()
        .map(|guide| guide.name.as_str())
        .collect();
    assert_eq!(listed, ["ORG26817", "ORG26392", "01-22-N11B-1670"]);
    assert_eq!(shop.vin_guides_as_of(), Some("2026-10-12"));
}

#[test]
fn the_step_fails_on_a_filing_listed_twice() {
    let scratch = Scratch::new("vin-guides-twice");
    let directory = nhtsa_vin_guides_dir(&scratch);
    std::fs::copy(
        directory.join("565-2023-04.json"),
        directory.join("565-2005-04.json"),
    )
    .unwrap();
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").starts_with("storing the filing ORG21569"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}

/// Run by hand after `open-data fetch nhtsa-vin-guides`, with the real
/// core data file in `data/build`:
/// `cargo test -p open-data --test shop the_fetched_vin_guides_build -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched answers in data/build/sources/nhtsa-vin-guides and data/build/wenmar-open-2026.09.sqlite3"]
fn the_fetched_vin_guides_build() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("data/build/sources/nhtsa-vin-guides");
    let core = root.join("data/build/wenmar-open-2026.09.sqlite3");
    let scratch = Scratch::new("vin-guides-real");
    build_with(
        &core,
        &scratch.out(),
        &root.join("data"),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap();
    let connection = Connection::open(scratch.out()).unwrap();
    let count = |sql: &str| -> i64 { connection.query_row(sql, [], |row| row.get(0)).unwrap() };
    let filings = count("SELECT COUNT(*) FROM vin_guide");
    let manufacturers = count("SELECT COUNT(DISTINCT manufacturer_id) FROM vin_guide");
    let matched = count("SELECT COUNT(DISTINCT manufacturer_id) FROM vin_guide_wmi");
    let reached = count(
        "SELECT COUNT(*) FROM vin_guide WHERE manufacturer_id IN (SELECT manufacturer_id FROM vin_guide_wmi)",
    );
    let codes = count("SELECT COUNT(DISTINCT wmi) FROM vin_guide_wmi");
    let makes = count("SELECT COUNT(DISTINCT make_id) FROM vin_guide_make");
    println!(
        "{filings} filings by {manufacturers} manufacturers; {matched} manufacturers and {reached} filings reach a code; {codes} codes and {makes} makes have a filing"
    );
    assert!(filings > 25_000, "{filings}");
    // Measured on 2026-10-12: 96.0% of manufacturers and 96.1% of filings.
    assert!(
        matched * 100 >= manufacturers * 90,
        "{matched} of {manufacturers}"
    );
    assert!(reached * 100 >= filings * 90, "{reached} of {filings}");
}
