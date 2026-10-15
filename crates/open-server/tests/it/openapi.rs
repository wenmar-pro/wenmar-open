//! The OpenAPI description is generated from the routes. A copy is kept in
//! the repository so a change to the API shows up in review.

use std::path::Path;

use axum::http::StatusCode;
use serde_json::Value;

use crate::common::{self, body_text};

#[test]
fn the_description_matches_the_committed_copy() {
    let generated = open_server::openapi_json();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
    if std::env::var_os("UPDATE_OPENAPI").is_some() {
        std::fs::write(&path, format!("{generated}\n")).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed.trim_end() == generated.trim_end(),
        "crates/open-server/openapi.json is out of date. If the API change is intended, run:\n  UPDATE_OPENAPI=1 cargo test -p open-server --test it openapi\nand commit the file."
    );
}

#[test]
fn the_description_lists_every_route_and_no_others() {
    let description: Value = serde_json::from_str(&open_server::openapi_json()).unwrap();
    let mut paths: Vec<&str> = description["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "/v1/bulletins",
            "/v1/bulletins/{nhtsa_id}",
            "/v1/codes",
            "/v1/codes/{code}",
            "/v1/jobs",
            "/v1/jobs/{job}",
            "/v1/makes/{make}",
            "/v1/meta",
            "/v1/models/{make}/{model}",
            "/v1/payroll/deductions",
            "/v1/recalls",
            "/v1/recalls/recent",
            "/v1/recalls/{source}/{campaign}",
            "/v1/recalls/{source}/{campaign}/investigations",
            "/v1/vehicles/engines",
            "/v1/vehicles/makes",
            "/v1/vehicles/models",
            "/v1/vehicles/search",
            "/v1/vehicles/submodels",
            "/v1/vehicles/trims",
            "/v1/vehicles/years",
            "/v1/vehicles/{id}",
            "/v1/vehicles/{id}/bulletins",
            "/v1/vehicles/{id}/codes/{code}",
            "/v1/vehicles/{id}/complaints",
            "/v1/vehicles/{id}/fluids",
            "/v1/vehicles/{id}/fuel",
            "/v1/vehicles/{id}/investigations",
            "/v1/vehicles/{id}/labor",
            "/v1/vehicles/{id}/labor/{job}",
            "/v1/vehicles/{id}/overview",
            "/v1/vehicles/{id}/recalls",
            "/v1/vehicles/{id}/safety",
            "/v1/vehicles/{id}/specs",
            "/v1/vehicles/{id}/tires",
            "/v1/vin/batch",
            "/v1/vin/{vin}",
            "/v1/vin/{vin}/bulletins",
            "/v1/vin/{vin}/fluids",
            "/v1/vin/{vin}/fuel",
            "/v1/vin/{vin}/labor",
            "/v1/vin/{vin}/labor/{job}",
            "/v1/vin/{vin}/overview",
            "/v1/vin/{vin}/parts-card",
            "/v1/vin/{vin}/recalls",
            "/v1/vin/{vin}/specs",
            "/v1/vin/{vin}/tires",
        ]
    );
    // `/health` is for the deploy proxy and is not part of the API.
    assert!(description["paths"].get("/health").is_none());
    let schemas = description["components"]["schemas"].as_object().unwrap();
    for name in [
        "VinDecode",
        "ErrorBody",
        "Entry",
        "Make",
        "MetaResponse",
        "DatasetInfo",
        "VehicleRecalls",
        "VinRecalls",
        "RecallCampaign",
        "RecallCompletion",
        "RecallList",
        "Recall",
        "RecallLookup",
        "Investigation",
        "VehicleInvestigations",
        "RecallInvestigations",
        "Overview",
        "OverviewRecalls",
        "RecallCount",
        "ModelYears",
        "RecentRecalls",
        "VinRecallCount",
        "VehicleSafety",
        "SafetyRating",
        "VehicleFuel",
        "VinFuel",
        "FuelRating",
        "VehicleSpecs",
        "VinSpecs",
        "VehicleFluids",
        "VinFluids",
        "EngineOil",
        "FluidRecord",
        "VehicleTires",
        "VinTires",
        "TireFitment",
        "PartsCard",
        "VehicleBulletins",
        "VinBulletins",
        "BulletinDetail",
        "BulletinList",
        "Bulletin",
        "BulletinDocument",
        "BulletinCounts",
        "JobList",
        "JobDetail",
        "VehicleLabor",
        "VinLabor",
        "LaborFigure",
        "Code",
        "CodeList",
        "VehicleCode",
        "CodeRelated",
        "VehicleComplaints",
        "ComponentComplaints",
    ] {
        assert!(schemas.contains_key(name), "no schema for {name}");
    }
    assert_eq!(description["info"]["title"], "Wenmar Open");
}

#[tokio::test]
async fn the_description_is_served_as_generated() {
    let app = common::app().await;
    let response = app.get("/v1/openapi.json").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        common::header(&response, "content-type"),
        "application/json"
    );
    assert_eq!(body_text(response).await, open_server::openapi_json());
}
