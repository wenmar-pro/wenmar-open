use open_data::shop::build::{ShopInputs, build_shop};
use rusqlite::Connection;

use crate::common::{Scratch, info, mfr_comms_dir, nhtsa_recalls_dir, rows, tc_recalls_dir};

/// A shop file with every fixture source, built as `build-shop` builds it.
// Other plans add fields to `ShopInputs`; until one does, the last line of
// the literal changes nothing.
#[allow(clippy::needless_update)]
fn built(name: &str) -> (Scratch, Connection) {
    let scratch = Scratch::new(name);
    let inputs = ShopInputs {
        nhtsa_recalls: Some(nhtsa_recalls_dir(&scratch)),
        tc_recalls: Some(tc_recalls_dir(&scratch)),
        nhtsa_mfr_comms: Some(mfr_comms_dir(&scratch)),
        ..ShopInputs::default()
    };
    build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &inputs,
    )
    .unwrap();
    let connection = Connection::open(scratch.out()).unwrap();
    (scratch, connection)
}

#[test]
fn the_components_of_the_file_are_stored() {
    let (_scratch, connection) = built("components-list");
    assert_eq!(
        rows(
            &connection,
            "SELECT id, name, parent FROM component ORDER BY id"
        ),
        [
            "air-bags|Air bags|-",
            "electrical|Electrical system|-",
            "engine|Engine|-",
            "exhaust-emissions|Exhaust and emissions|-",
            "powertrain|Transmission and driveline|-",
            "software|Software|electrical",
            "timing-belt|Timing belt or chain|engine",
            "transmission-control-module|Transmission control module|powertrain",
            "visibility|Glass, wipers and mirrors|-",
        ]
    );
}

#[test]
fn a_bulletin_is_about_every_component_above_each_text_it_names() {
    let (_scratch, connection) = built("components-bulletins");
    assert_eq!(
        rows(
            &connection,
            "SELECT bulletin_id, component_id FROM bulletin_component ORDER BY 1, 2"
        ),
        [
            "609932|engine",
            "10085226|electrical",
            "10085226|software",
            "10106674|electrical",
            "10106674|powertrain",
            "10106674|software",
            "10106674|transmission-control-module",
            // ENGINE AND ENGINE COOLING:EXHAUST SYSTEM is under two systems.
            "10152456|engine",
            "10152456|exhaust-emissions",
            "10158006|electrical",
            "10158006|powertrain",
            "10194916|visibility",
            "10220350|visibility",
            "11011137|engine",
            "11026354|powertrain",
            "11031510|engine",
            "11031644|electrical",
        ]
    );
    // UNKNOWN OR OTHER is no component: these two bulletins have none.
    assert_eq!(
        rows(
            &connection,
            "SELECT id FROM bulletin WHERE id NOT IN (SELECT bulletin_id FROM bulletin_component) ORDER BY id"
        ),
        ["11006904", "11009827"]
    );
}

#[test]
fn a_recall_is_about_components_too_in_either_country() {
    let (_scratch, connection) = built("components-recalls");
    assert_eq!(
        rows(
            &connection,
            "SELECT r.source, r.campaign, c.component_id FROM recall_component c
             JOIN recall r ON r.id = c.recall_id ORDER BY r.id, c.component_id"
        ),
        [
            "nhtsa|05V483000|visibility",
            "nhtsa|17V669000|powertrain",
            "nhtsa|17V669000|transmission-control-module",
            "nhtsa|19V888000|electrical",
            "nhtsa|26V375000|powertrain",
            "nhtsa|26V375000|transmission-control-module",
            // Transport Canada names the system alone.
            "tc|2018601|engine",
            "tc|2019631|electrical",
        ]
    );
}

#[test]
fn codes_and_topics_come_from_the_summaries() {
    let (_scratch, connection) = built("components-text");
    assert_eq!(
        rows(
            &connection,
            "SELECT bulletin_id, code FROM bulletin_code ORDER BY 1, 2"
        ),
        ["10106674|P0326", "10152456|P0471", "11011137|P0126"]
    );
    // "service interval" is maintenance; a timing belt that is too tight
    // (609932) is not.
    assert_eq!(
        rows(
            &connection,
            "SELECT bulletin_id, topic FROM bulletin_topic ORDER BY 1, 2"
        ),
        ["11026354|maintenance"]
    );
}

#[test]
fn a_file_with_no_source_has_no_component_tables() {
    let scratch = Scratch::new("components-none");
    build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &ShopInputs::default(),
    )
    .unwrap();
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name"
        ),
        ["meta"]
    );
}

#[test]
fn a_build_without_the_components_file_says_so() {
    let scratch = Scratch::new("components-missing");
    std::fs::remove_file(scratch.curated().join("components.yaml")).unwrap();
    let inputs = ShopInputs {
        nhtsa_recalls: Some(nhtsa_recalls_dir(&scratch)),
        ..ShopInputs::default()
    };
    let error = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &inputs,
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("components.yaml"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}
