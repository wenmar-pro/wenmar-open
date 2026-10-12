use open_data::shop::mfr_comms::{
    FIELDS, MfrCommCounts, MfrCommRow, TYPES, archives, check_layout, layout, parse,
};

use crate::common::{Scratch, fixture, mfr_comms_dir};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<MfrCommRow>, MfrCommCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

fn fixture_rows(name: &str) -> (Vec<MfrCommRow>, MfrCommCounts) {
    let scratch = Scratch::new(name);
    read(&mfr_comms_dir(&scratch)).unwrap()
}

/// The rows of one communication, in file order.
fn communication(rows: &[MfrCommRow], nhtsa_id: u32) -> Vec<&MfrCommRow> {
    rows.iter().filter(|row| row.nhtsa_id == nhtsa_id).collect()
}

#[test]
fn the_field_order_is_the_one_tsbs_txt_gives() {
    let text = std::fs::read_to_string(fixture("nhtsa-mfr-comms").join("TSBS.txt")).unwrap();
    assert_eq!(layout(&text), FIELDS);
    check_layout(&text).unwrap();
    // The positions this build relies on, counted from 1 as TSBS.txt does.
    for (number, name) in [
        (1, "NHTSA ID Number"),
        (3, "Date Added to File"),
        (4, "TSB/Document ID"),
        (5, "Mfr Communication Date"),
        (7, "Communication Type"),
        (8, "Make"),
        (9, "Model"),
        (10, "Model Year"),
        (11, "NHTSA Components"),
        (14, "Summary"),
    ] {
        assert_eq!(FIELDS[number - 1], name);
    }
    // A change in the file's list is refused, and the message names it.
    let changed = text.replace("\"Summary\"", "\"Concise Summary\"");
    let error = check_layout(&changed).unwrap_err().to_string();
    assert!(
        error.starts_with("the fields are not the ones this build reads: found 14 ("),
        "{error}"
    );
    assert!(error.contains("Concise Summary"), "{error}");
}

#[test]
fn reads_every_archive_source_json_lists_in_order() {
    let scratch = Scratch::new("comms-counts");
    let directory = mfr_comms_dir(&scratch);
    assert_eq!(
        archives(&directory).unwrap(),
        [
            "TSBS_RECEIVED_2000-2004.zip",
            "TSBS_RECEIVED_2015-2019.zip",
            "TSBS_RECEIVED_2020-2024.zip",
            "TSBS_RECEIVED_2025-2026.zip",
        ]
    );
    // A file left from an earlier fetch under an old name is not read.
    std::fs::write(directory.join("TSBS_RECEIVED_2025-2025.zip"), b"stale").unwrap();
    let (rows, counts) = read(&directory).unwrap();
    assert_eq!(
        counts,
        MfrCommCounts {
            files: 4,
            records: 46
        }
    );
    assert_eq!(rows.len(), 46);
    // File order, and the records of a communication together.
    let mut ids: Vec<u32> = rows.iter().map(|row| row.nhtsa_id).collect();
    ids.dedup();
    assert_eq!(
        ids,
        [
            609932, 10085226, 10106674, 10152456, 10158006, 10194916, 10220350, 11006904, 11009827,
            11011137, 11026354, 11031510, 11031644
        ]
    );
    // Every type the files hold is one of the six.
    for row in &rows {
        assert!(TYPES.contains(&row.kind.as_str()), "{}", row.kind);
    }
}

#[test]
fn a_communication_spans_a_row_for_each_vehicle_and_component() {
    let (rows, _) = fixture_rows("comms-spans");
    // Four model years, two components each.
    let found = communication(&rows, 10085226);
    assert_eq!(found.len(), 8);
    let mut years: Vec<i32> = found.iter().filter_map(|row| row.year).collect();
    years.sort_unstable();
    years.dedup();
    assert_eq!(years, [2012, 2013, 2014, 2016]);
    let mut components: Vec<&str> = found
        .iter()
        .filter_map(|row| row.component.as_deref())
        .collect();
    components.sort_unstable();
    components.dedup();
    assert_eq!(components, ["ELECTRICAL SYSTEM:SOFTWARE", "EQUIPMENT"]);
    let first = found[0];
    assert_eq!(first.make_text, "HONDA");
    assert_eq!(first.model_text, "CR-V");
    assert_eq!(first.kind, "Service Bulletin/Repair Instructions");
    assert_eq!(first.added, "2016-11-30");
    // What a communication says of itself is the same on every row.
    for row in &found {
        assert_eq!(row.summary, first.summary);
        assert_eq!(row.document_id, first.document_id);
        assert_eq!(row.added, first.added);
    }

    // Two makes in one communication, and the manufacturer's campaign id.
    let found = communication(&rows, 11031510);
    let vehicles: Vec<(&str, &str, Option<i32>)> = found
        .iter()
        .map(|row| (row.make_text.as_str(), row.model_text.as_str(), row.year))
        .collect();
    assert_eq!(
        vehicles,
        [
            ("CHEVROLET", "SILVERADO 1500", Some(2019)),
            ("GMC", "SIERRA 1500", Some(2019)),
        ]
    );
    assert_eq!(found[0].kind, "Service Campaign");
    assert_eq!(found[0].mfr_campaign.as_deref(), Some("N262548050"));
    assert_eq!(found[0].document_id.as_deref(), Some("N262548050"));
    assert_eq!(found[0].added, "2026-04-23");
    assert_eq!(found[0].date.as_deref(), Some("2026-04-23"));
}

#[test]
fn fields_are_read_as_they_are_and_placeholders_become_nothing() {
    let (rows, _) = fixture_rows("comms-fields");
    // Model year 9999 is no year.
    let dealer = communication(&rows, 10220350)[0];
    assert_eq!(dealer.year, None);
    assert_eq!(dealer.model_text, "HONDA");
    assert_eq!(dealer.component.as_deref(), Some("VISIBILITY"));
    assert_eq!(dealer.mfr_campaign, None);
    assert_eq!(dealer.replaces, None);

    // The manufacturer's own date differs from the day NHTSA added it.
    let old = communication(&rows, 609932)[0];
    assert_eq!(old.added, "2000-04-11");
    assert_eq!(old.date.as_deref(), Some("1998-08-04"));
    assert_eq!(old.document_id.as_deref(), Some("96057"));
    assert!(
        old.summary
            .as_deref()
            .unwrap()
            .starts_with("THE TIMING BELT TENSIONED TOO TIGHT")
    );

    // The manufacturer's own component words, with or without a system.
    let heater = communication(&rows, 11006904)[0];
    assert_eq!(heater.mfr_system, None);
    assert_eq!(heater.mfr_subsystem.as_deref(), Some("Heater"));
    assert_eq!(heater.component.as_deref(), Some("UNKNOWN OR OTHER"));
    let over_the_air = communication(&rows, 11031644);
    assert_eq!(over_the_air[0].kind, "Over The Air");
    assert_eq!(
        over_the_air[0].mfr_campaign.as_deref(),
        Some("IPMA-25.22.7.3.10B")
    );
    assert_eq!(
        over_the_air[0].summary.as_deref(),
        Some("Quality improvements include: -Enhanced BlueCruise performance")
    );
    assert_eq!(over_the_air.len(), 4);
    let kinds: Vec<&str> = [11009827u32, 11011137]
        .iter()
        .map(|id| communication(&rows, *id)[0].kind.as_str())
        .collect();
    assert_eq!(kinds, ["Emissions", "Warranty Program/Extension"]);
    // A component with a comma in its name is one component.
    assert!(rows.iter().all(|row| row.component.is_some()));
    // A trailing blank in a component or a document id is trimmed.
    let cascadia = communication(&rows, 10158006);
    assert!(
        cascadia
            .iter()
            .any(|row| row.component.as_deref()
                == Some("EQUIPMENT:OTHER:OWNERS/SERVICE/OTHER MANUAL"))
    );
    assert_eq!(
        communication(&rows, 10194916)[0].document_id.as_deref(),
        Some("A19-091")
    );
}

#[test]
fn damaged_text_is_put_right() {
    let (rows, _) = fixture_rows("comms-text");
    // Typed in Windows-1252: U+0092 is a right single quote.
    let knock = communication(&rows, 10106674)[0].summary.clone().unwrap();
    assert!(
        knock.contains("MISINTERPRETS THE SENSOR\u{2019}S SIGNALS AND SETS DTC P0326."),
        "{knock}"
    );
    // UTF-8 read as Latin-1 twice over, and runs of blanks made one.
    let condenser = communication(&rows, 10194916)[0].summary.clone().unwrap();
    assert!(
        condenser.contains("under any state\u{2019}s law"),
        "{condenser}"
    );
    for row in &rows {
        for text in [&row.summary, &row.document_id, &row.component] {
            let text = text.as_deref().unwrap_or("");
            assert!(!text.chars().any(char::is_control), "{text}");
            assert!(!text.contains("  "), "{text}");
            assert!(
                !text.contains('\u{c2}') && !text.contains('\u{e2}'),
                "{text}"
            );
        }
    }
}

#[test]
fn a_directory_that_is_not_a_fetch_is_refused() {
    let scratch = Scratch::new("comms-refused");
    let directory = mfr_comms_dir(&scratch);

    // No layout file.
    let layout_path = directory.join("TSBS.txt");
    let text = std::fs::read_to_string(&layout_path).unwrap();
    std::fs::remove_file(&layout_path).unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(
        error.contains("run open-data fetch nhtsa-mfr-comms first"),
        "{error}"
    );
    // Another layout.
    std::fs::write(&layout_path, text.replace("\"Make\"", "\"Maker\"")).unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(
        error.contains("the fields are not the ones this build reads"),
        "{error}"
    );
    std::fs::write(&layout_path, &text).unwrap();
    read(&directory).unwrap();

    // An archive that is not a zip.
    std::fs::write(directory.join("TSBS_RECEIVED_2015-2019.zip"), b"not a zip").unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(
        error.contains("TSBS_RECEIVED_2015-2019.zip is not a zip file"),
        "{error}"
    );
    // An archive source.json lists that is not there.
    std::fs::remove_file(directory.join("TSBS_RECEIVED_2015-2019.zip")).unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(error.contains("TSBS_RECEIVED_2015-2019.zip"), "{error}");
    // No source.json.
    std::fs::remove_file(directory.join("source.json")).unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(error.contains("run open-data fetch first"), "{error}");
}

/// Run by hand after `open-data fetch nhtsa-mfr-comms`:
/// `cargo test --release -p open-data --test shop the_fetched_mfr_comms_files_parse -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched files in data/build/sources/nhtsa-mfr-comms"]
fn the_fetched_mfr_comms_files_parse() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/nhtsa-mfr-comms");
    let mut communications = std::collections::BTreeSet::new();
    let mut unknown_types = 0u64;
    let counts = parse(&directory, &mut |row| {
        communications.insert(row.nhtsa_id);
        if !TYPES.contains(&row.kind.as_str()) {
            unknown_types += 1;
        }
        Ok(())
    })
    .unwrap();
    println!(
        "{counts:?}, {} communications, {unknown_types} records of an unknown type",
        communications.len()
    );
    assert!(counts.files >= 7, "{counts:?}");
    assert!(counts.records > 5_800_000, "{counts:?}");
    assert!(communications.len() > 260_000, "{}", communications.len());
}
