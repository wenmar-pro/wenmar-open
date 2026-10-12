use open_data::shop::build::build_with;
use open_data::shop::coverage;
use open_data::shop::nhtsa_investigations::{
    FIELDS, InvestigationCounts, InvestigationRow, check_layout, layout, parse, shape,
};
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, licence};

use crate::common::{Scratch, fixture, info, nhtsa_investigations_dir, rows};

fn read(
    directory: &std::path::Path,
) -> anyhow::Result<(Vec<InvestigationRow>, InvestigationCounts)> {
    let mut found = Vec::new();
    let counts = parse(directory, &mut |row| {
        found.push(row);
        Ok(())
    })?;
    Ok((found, counts))
}

/// The rows of one investigation, in file order.
fn of<'r>(rows: &'r [InvestigationRow], action: &str) -> Vec<&'r InvestigationRow> {
    rows.iter().filter(|row| row.action == action).collect()
}

#[test]
fn the_field_order_is_the_one_inv_txt_gives() {
    let text = std::fs::read_to_string(fixture("nhtsa-investigations").join("INV.txt")).unwrap();
    assert_eq!(layout(&text), FIELDS);
    check_layout(&text).unwrap();
    // The positions this build relies on, counted from 1 as INV.txt does.
    for (number, name) in [
        (1, "NHTSA ACTION NUMBER"),
        (4, "YEAR"),
        (7, "ODATE"),
        (8, "CDATE"),
        (9, "CAMPNO"),
        (11, "SUMMARY"),
    ] {
        assert_eq!(FIELDS[number - 1], name);
    }
}

#[test]
fn reads_every_row_of_the_archive() {
    let scratch = Scratch::new("inv-rows");
    let (found, counts) = read(&nhtsa_investigations_dir(&scratch)).unwrap();
    assert_eq!(
        counts,
        InvestigationCounts {
            records: 23,
            junk: 0
        }
    );
    let actions: Vec<&str> = found.iter().map(|row| row.action.as_str()).collect();
    let mut once = actions.clone();
    once.dedup();
    assert_eq!(
        once,
        [
            "AQ18006", "C87001", "DP23005", "DP90001", "EA05013", "EA23001", "EA88041", "ID94001",
            "PE18008", "RQ98022"
        ]
    );

    // An open investigation: no day closed. Its campaign number is written
    // short in the file and is read whole.
    let open = of(&found, "EA23001");
    assert_eq!(open.len(), 2);
    assert_eq!(
        (
            open[0].make_text.as_deref(),
            open[0].model_text.as_deref(),
            open[0].year
        ),
        (Some("RAM"), Some("1500"), Some(2019))
    );
    assert_eq!(open[1].model_text.as_deref(), Some("2500"));
    assert_eq!(open[0].opened.as_deref(), Some("2023-03-17"));
    assert_eq!(open[0].closed, None);
    assert_eq!(open[0].campaign.as_deref(), Some("24V957000"));
    assert_eq!(
        open[0].component.as_deref(),
        Some("FUEL SYSTEM, DIESEL:DELIVERY:FUEL PUMP")
    );
    assert_eq!(
        open[0].manufacturer.as_deref(),
        Some("Chrysler (FCA US, LLC)")
    );
    assert_eq!(
        open[0].subject.as_deref(),
        Some("High pressure fuel pump failure")
    );
    assert!(open[0].summary.as_deref().unwrap().len() > 3000);
    // What an investigation says of itself is the same on every row.
    assert_eq!(open[0].summary, open[1].summary);

    // A closed one that led to a recall.
    let closed = of(&found, "PE18008");
    assert_eq!(closed.len(), 4);
    assert_eq!(closed[0].closed.as_deref(), Some("2018-12-17"));
    assert_eq!(closed[3].year, Some(2018));
    assert_eq!(closed[3].campaign.as_deref(), Some("18V568000"));

    // One vehicle listed twice, once for each component.
    let components: Vec<&str> = of(&found, "DP23005")
        .iter()
        .filter(|row| row.model_text.as_deref() == Some("PRIUS") && row.year == Some(2013))
        .map(|row| row.component.as_deref().unwrap())
        .collect();
    assert_eq!(
        components,
        [
            "SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS",
            "SERVICE BRAKES, HYDRAULIC:POWER ASSIST"
        ]
    );

    // 9999 means no model year.
    let tires = of(&found, "DP90001");
    assert_eq!(tires[0].year, None);
    assert_eq!(tires[1].make_text.as_deref(), Some("LEE"));

    // Rows with no vehicle at all: one with dates and a subject, one with
    // nothing but two campaigns.
    let bare = of(&found, "C87001");
    assert_eq!(bare.len(), 1);
    assert_eq!(bare[0].make_text, None);
    assert_eq!(bare[0].opened.as_deref(), Some("1987-04-27"));
    assert_eq!(bare[0].subject.as_deref(), Some("REAR SUSPENSION OVERLOAD"));
    assert_eq!(bare[0].summary, None, "the placeholder is not a summary");
    let stub = of(&found, "ID94001");
    assert_eq!(
        stub.iter()
            .map(|row| row.campaign.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["94E013000", "94E013001"]
    );
    assert_eq!(stub[0].subject, None, "a subject of ID is no subject");
    assert_eq!(stub[0].opened, None);
    // `===` is the other placeholder.
    assert_eq!(of(&found, "EA88041")[0].summary, None);
}

#[test]
fn refuses_a_directory_it_cannot_read() {
    let scratch = Scratch::new("inv-refuse");
    let message = |directory: &std::path::Path| format!("{:#}", read(directory).unwrap_err());

    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        message(&empty).ends_with(
            "run open-data fetch nhtsa-investigations first: No such file or directory (os error 2)"
        ),
        "{}",
        message(&empty)
    );

    let directory = nhtsa_investigations_dir(&scratch);
    let layout_path = directory.join("INV.txt");
    let mut text = std::fs::read_to_string(&layout_path).unwrap();
    text.push_str("12 NEW_FIELD CHAR(3) Something NHTSA added\n");
    std::fs::write(&layout_path, text).unwrap();
    let error = message(&directory);
    assert!(
        error.contains("INV.txt: the fields are not the ones this build reads: found 12 ("),
        "{error}"
    );

    let directory = nhtsa_investigations_dir(&scratch);
    std::fs::write(directory.join("FLAT_INV.zip"), b"not a zip").unwrap();
    assert!(message(&directory).contains("FLAT_INV.zip is not a zip file"));
    std::fs::remove_file(directory.join("FLAT_INV.zip")).unwrap();
    assert!(
        message(&directory)
            .contains("FLAT_INV.zip; run open-data fetch nhtsa-investigations first")
    );
}

#[test]
fn investigations_become_rows_with_their_vehicles_matched_and_their_campaigns() {
    let scratch = Scratch::new("inv-shape");
    let directory = nhtsa_investigations_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::Investigations).unwrap();
    assert_eq!(entry.as_of, "2026-10-12");
    assert_eq!(entry.rows, 10);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert_eq!(entry.attribution, "");

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT id, action, kind, opened, closed, is_open, manufacturer, component, subject, length(summary) FROM investigation ORDER BY id"
        ),
        [
            // Open, but an audit query: a reader does not say it of a vehicle.
            "1|AQ18006|AQ|2018-12-01|-|1|General Motors, LLC|SEATS|Sale of noncompliant school bus|1004",
            "2|C87001|C|1987-04-27|1991-07-31|0|-|-|REAR SUSPENSION OVERLOAD|-",
            "3|DP23005|DP|2023-11-22|-|1|Toyota Motor Corporation|SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS; SERVICE BRAKES, HYDRAULIC:POWER ASSIST|Brake Actuator Valve Wear|766",
            "4|DP90001|DP|1989-12-18|1990-04-23|0|Goodyear Tire & Rubber Company; LEE TIRE & RUBBER CO.|TIRES|TIRES CRACKING/DEFLATING|24",
            "5|EA05013|EA|2005-08-26|2005-10-26|0|Toyota Motor Corporation|VISIBILITY:SUN/MOON ROOF ASSEMBLY|SUN/MOON ROOF WIND DEFLECTOR BREAKAGE|488",
            "6|EA23001|EA|2023-03-17|-|1|Chrysler (FCA US, LLC)|FUEL SYSTEM, DIESEL:DELIVERY:FUEL PUMP|High pressure fuel pump failure|3109",
            "7|EA88041|EA|1988-09-15|1990-05-11|0|Ford Motor Company|VEHICLE SPEED CONTROL:LINKAGES|FILLER PIPE SEAL|-",
            "8|ID94001|ID|-|-|0|-|-|-|-",
            "9|PE18008|PE|2018-08-03|2018-12-17|0|Ford Motor Company|SEAT BELTS:PRETENSIONER|Seatbelt Pretensioner Fire|1368",
            // No day closed, but opened in 1998: not open.
            "10|RQ98022|RQ|1998-11-13|-|0|Ford Motor Company|SUSPENSION:REAR:SPRINGS:LEAF SPRING ASSEMBLY|REAR SUSPENSION SPRING BREAKAGE|1424",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT i.action, v.year, v.make_id, v.model_id, v.make_text, v.model_text, v.\"match\"
             FROM investigation_vehicle v JOIN investigation i ON i.id = v.investigation_id
             ORDER BY v.rowid"
        ),
        [
            "AQ18006|2015|467|-|CHEVROLET|EXPRESS|-",
            // Two rows for one vehicle, one per component, are one row.
            "DP23005|2013|448|-|TOYOTA|CAMRY HYBRID|-",
            "DP23005|2013|448|2209|TOYOTA|PRIUS|exact",
            // The fixture catalog has no 2014 Prius.
            "DP23005|2014|448|-|TOYOTA|PRIUS|-",
            // 9999 is stored as no year.
            "DP90001|-|-|-|GOODYEAR|GOODYEAR|-",
            "DP90001|-|-|-|LEE|LEE|-",
            "EA05013|2005|-|-|SCION|TC|-",
            "EA23001|2019|496|13620|RAM|1500|exact",
            "EA23001|2019|496|-|RAM|2500|-",
            "EA88041|1986|460|-|FORD|AEROSTAR|-",
            "EA88041|1987|460|-|FORD|AEROSTAR|-",
            "PE18008|2015|460|-|FORD|F-150|-",
            "PE18008|2016|460|-|FORD|F-150|-",
            "PE18008|2017|460|-|FORD|F-150|-",
            "PE18008|2018|460|1801|FORD|F-150|exact",
            "RQ98022|1997|460|-|FORD|F-150|-",
            "RQ98022|1998|460|-|FORD|F-150|-",
        ]
    );
    // C87001 and ID94001 list no vehicle: no vehicle row.
    assert_eq!(
        rows(
            &connection,
            "SELECT i.action, c.campaign FROM investigation_campaign c
             JOIN investigation i ON i.id = c.investigation_id ORDER BY i.action, c.campaign"
        ),
        [
            "EA05013|05V483000",
            "EA23001|24V957000",
            "EA88041|90V021000",
            "ID94001|94E013000",
            "ID94001|94E013001",
            "PE18008|18V568000",
            "RQ98022|98V161000",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT COUNT(*) FROM investigation_vehicle WHERE (model_id IS NULL) <> (\"match\" IS NULL)"
        ),
        ["0"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'investigation%' ORDER BY name"
        ),
        [
            "investigation_campaign_investigation",
            "investigation_vehicle_investigation",
            "investigation_vehicle_ymm"
        ]
    );
    drop(connection);

    // The reader reads what the build wrote, and coverage measures it.
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    let open = shop
        .investigations_for_vehicle(496, 13620, 2019, true)
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].investigation.action, "EA23001");
    assert_eq!(open[0].investigation.campaigns, ["24V957000"]);
    assert!(open[0].investigation.open);
    // Closed: not among the open ones, and there when all are asked for.
    assert!(
        shop.investigations_for_vehicle(460, 1801, 2018, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        shop.investigations_for_vehicle(460, 1801, 2018, false)
            .unwrap()[0]
            .investigation
            .action,
        "PE18008"
    );
    let led = shop.investigations_for_campaign("05V483000").unwrap();
    assert_eq!(led.len(), 1);
    assert_eq!(led[0].action, "EA05013");

    let (catalog, _) = open_data::shop::build::open_catalog(&scratch.core()).unwrap();
    let report = coverage::run(
        &shop,
        &coverage::light_makes(&catalog).unwrap(),
        coverage::TABLES,
    )
    .unwrap();
    assert_eq!(report.datasets.len(), 1);
    let found = &report.datasets[0];
    assert_eq!(found.dataset, "investigations");
    assert_eq!(
        (found.rows, found.measured, found.exact, found.unmatched),
        (17, 10, 3, 7)
    );
    assert_eq!(found.share(), "30.0");
}

#[test]
fn the_step_fails_without_the_record_of_the_fetch() {
    let scratch = Scratch::new("inv-no-record");
    let directory = nhtsa_investigations_dir(&scratch);
    std::fs::remove_file(directory.join("source.json")).unwrap();
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("run open-data fetch first"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}

/// Run by hand after `open-data fetch nhtsa-investigations`:
/// `cargo test -p open-data --test shop the_fetched_investigations_parse -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched files in data/build/sources/nhtsa-investigations"]
fn the_fetched_investigations_parse() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/nhtsa-investigations");
    let mut actions = std::collections::BTreeSet::new();
    let mut open = std::collections::BTreeSet::new();
    let counts = parse(&directory, &mut |row| {
        if open_data::shop::nhtsa_investigations::is_open(
            row.opened.as_deref(),
            row.closed.as_deref(),
        ) {
            open.insert(row.action.clone());
        }
        actions.insert(row.action);
        Ok(())
    })
    .unwrap();
    println!(
        "{counts:?}, {} investigations, {} open",
        actions.len(),
        open.len()
    );
    assert!(counts.records > 150_000, "{counts:?}");
    assert!(counts.junk < 100, "{counts:?}");
    assert!(actions.len() > 5_300, "{}", actions.len());
    assert!((20..400).contains(&open.len()), "{}", open.len());
}
