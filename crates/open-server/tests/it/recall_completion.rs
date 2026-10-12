//! Remedy progress on a recall campaign: the manufacturer's newest report
//! to NHTSA, in the campaign's answer and on its page. The figures are for
//! the whole campaign and a named period, never for one vehicle.

use std::ffi::OsString;
use std::path::Path;

use axum::http::StatusCode;
use serde_json::{Value, json};
use tempfile::TempDir;
use wenmar_open_cli::env::Env;
use wenmar_shopdata::answer::SITE;

use crate::common::{
    self, TestApp, assert_basics, assert_head, assert_targets, body_json, markdown, page,
};

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

#[tokio::test]
async fn a_campaign_answer_carries_the_newest_report_as_numbers_and_its_period() {
    let app = common::reports_app().await;
    let body = ok(&app, "/v1/recalls/nhtsa/24V456000").await;
    assert_eq!(
        body["completion"],
        json!({
            "kind": "annual",
            "report_number": 2,
            "period_from": "2025-2",
            "period_to": "2026-1",
            "submitted": "2026-04-13",
            "involved": 800,
            "remedied": 536,
            "unreachable": 12,
            "removed": 3,
            "remedied_percent": 67,
            "as_of": "2026-10-12"
        })
    );
    // The rest of the answer is as it was, "may apply" included.
    assert_eq!(body["may_apply"], true);
    assert_eq!(body["units"], 800);
    assert_eq!(body["as_of"], "2026-10-11");

    // A quarterly report covers one quarter. Letters in either case.
    let quarterly = ok(&app, "/v1/recalls/nhtsa/19v888000").await;
    assert_eq!(quarterly["completion"]["kind"], "quarterly");
    assert_eq!(quarterly["completion"]["period_from"], "2021-2");
    assert_eq!(quarterly["completion"]["period_to"], "2021-2");
    assert_eq!(quarterly["completion"]["remedied"], 333_125);
    assert_eq!(quarterly["completion"]["remedied_percent"], 95);

    // No vehicles involved: the counts stand and there is no share.
    let trailer = ok(&app, "/v1/recalls/nhtsa/19V500000").await;
    assert_eq!(trailer["completion"]["involved"], 0);
    assert_eq!(trailer["completion"]["remedied"], 22);
    assert_eq!(trailer["completion"]["remedied_percent"], Value::Null);
    assert_eq!(trailer["completion"]["submitted"], Value::Null);

    // A campaign with no report, and a Canadian campaign whose number a
    // report happens to carry: null, and nothing else changes.
    for path in ["/v1/recalls/nhtsa/23V123000", "/v1/recalls/tc/2023101"] {
        let body = ok(&app, path).await;
        assert_eq!(body["completion"], Value::Null, "{path}");
        assert!(body.as_object().unwrap().contains_key("completion"));
    }
}

#[tokio::test]
async fn a_server_without_the_reports_answers_null_and_is_otherwise_the_same() {
    let with = common::reports_app().await;
    let without = common::recalls_app().await;
    for path in [
        "/v1/recalls/nhtsa/24V456000",
        "/v1/recalls/nhtsa/19V888000",
        "/v1/recalls/tc/2023101",
    ] {
        let mut held = ok(&with, path).await;
        let bare = ok(&without, path).await;
        assert_eq!(bare["completion"], Value::Null, "{path}");
        held["completion"] = Value::Null;
        assert_eq!(held, bare, "{path}");
    }
    // The lists and a model year's recalls do not carry it at all.
    for path in [
        "/v1/recalls?limit=3",
        "/v1/vehicles/2023_hyundai_kona/recalls",
    ] {
        let body = ok(&with, path).await;
        assert_eq!(body, ok(&without, path).await, "{path}");
        assert!(body["recalls"][0].get("completion").is_none(), "{path}");
    }
    // The datasets the server holds say which case this is.
    let meta = ok(&with, "/v1/meta").await;
    assert_eq!(meta["datasets"]["recall_reports"]["as_of"], "2026-10-12");
    assert_eq!(meta["datasets"]["recall_reports"]["rows"], 4);
    assert_eq!(
        meta["datasets"]["recall_reports"]["licence"],
        "public-domain"
    );
}

#[tokio::test]
async fn the_recalls_tool_carries_the_report_as_the_api_does() {
    let app = common::reports_app().await;
    let message = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "wenmar_recalls",
            "arguments": { "action": "campaign", "source": "nhtsa", "campaign": "24V456000" }
        }
    });
    let response = app.post_json("/mcp", &message.to_string()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let answer = body_json(response).await;
    let served = ok(&app, "/v1/recalls/nhtsa/24V456000").await;
    assert_eq!(answer["result"]["structuredContent"], served);
    assert_eq!(
        answer["result"]["structuredContent"]["completion"]["remedied_percent"],
        67
    );
}

#[tokio::test]
async fn the_campaign_page_shows_the_report_with_its_period_and_claims_nothing_of_one_vehicle() {
    let app = common::reports_app().await;
    let path = "/recalls/nhtsa/24V456000";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    let table = "<table>\n<caption>Remedy progress the manufacturer reported</caption>\n<tbody>\n\
<tr><th scope=\"row\">Report</th><td>Annual report 2, for 2025 quarter 2 to 2026 quarter 1</td></tr>\n\
<tr><th scope=\"row\">Filed with NHTSA</th><td>2026-04-13</td></tr>\n\
<tr><th scope=\"row\">Vehicles in the campaign</th><td>800</td></tr>\n\
<tr><th scope=\"row\">Reported remedied</th><td>536 (67%)</td></tr>\n\
<tr><th scope=\"row\">Reported unreachable</th><td>12</td></tr>\n\
<tr><th scope=\"row\">Reported removed</th><td>3</td></tr>\n\
</tbody>\n</table>\n\
<p class=\"note\">These are the manufacturer&#39;s totals for the whole campaign at the end of 2026 quarter 1, as filed with NHTSA. They do not say whether one vehicle has been remedied: check the VIN with the official lookups. The reports are from data as of 2026-10-12.</p>\n";
    assert_eq!(html.matches(table).count(), 1, "{html}");
    // After the campaign's own facts and before what is wrong.
    let facts = html.find("<caption>Campaign</caption>").unwrap();
    let shown = html.find(table).unwrap();
    let wrong = html.find("<h2>What is wrong</h2>").unwrap();
    assert!(facts < shown && shown < wrong);
    // The page still says "may apply", and nothing says a vehicle is or is
    // not repaired.
    assert!(
        html.contains("A recall listed here may apply to a vehicle of this year, make and model.")
    );
    let lower = html.to_lowercase();
    for claim in [
        "your vehicle",
        "this vehicle has",
        "this vehicle is",
        "has been repaired",
        "is still open",
        "open recall",
        "chance",
        "likely",
    ] {
        assert!(!lower.contains(claim), "{claim:?}");
    }
    // Nothing of it reaches the head: the description is as before.
    let head = html.split("</head>").next().unwrap();
    assert!(!head.contains("67%") && !head.to_lowercase().contains("remedied"));

    // A quarterly report names its one quarter.
    let (_, html) = page(&app, "/recalls/nhtsa/19V888000").await;
    assert!(
        html.contains(
            "<tr><th scope=\"row\">Report</th><td>Quarterly report 6, for 2021 quarter 2</td></tr>"
        ),
        "{html}"
    );
    assert!(
        html.contains("<tr><th scope=\"row\">Reported remedied</th><td>333125 (95%)</td></tr>")
    );
    assert!(html.contains("at the end of 2021 quarter 2, as filed with NHTSA."));
    // No vehicles involved: no share, and no day filed.
    let (_, html) = page(&app, "/recalls/nhtsa/19V500000").await;
    assert!(
        html.contains("<tr><th scope=\"row\">Reported remedied</th><td>22</td></tr>"),
        "{html}"
    );
    assert!(!html.contains("Filed with NHTSA"));
    assert_basics(&html, "/recalls/nhtsa/19V500000");

    // The Markdown version has the same figures and the same caution.
    let text = markdown(&app, "/recalls/nhtsa/24V456000.md").await;
    assert!(text.contains(
        "\n## Remedy progress the manufacturer reported\n\n- Report: Annual report 2, for 2025 quarter 2 to 2026 quarter 1\n- Filed with NHTSA: 2026\\-04\\-13\n- Vehicles in the campaign: 800\n- Reported remedied: 536 \\(67%\\)\n- Reported unreachable: 12\n- Reported removed: 3\n\nThese are the manufacturer's totals for the whole campaign at the end of 2026 quarter 1, as filed with NHTSA. They do not say whether one vehicle has been remedied: check the VIN with the official lookups. The reports are from data as of 2026\\-10\\-12.\n\n## Vehicles this campaign lists\n"
    ), "{text}");
}

#[tokio::test]
async fn without_a_report_the_campaign_page_is_as_it_was() {
    let with = common::reports_app().await;
    let without = common::recalls_app().await;
    // A campaign with no report, a Canadian campaign whose number a report
    // carries, and an old campaign: the same bytes on both servers.
    for path in [
        "/recalls/nhtsa/23V123000",
        "/recalls/nhtsa/23V123000.md",
        "/recalls/tc/2023101",
        "/recalls/tc/2023101.md",
        "/recalls/nhtsa/68V023000",
    ] {
        let (status, expected) = page(&without, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!expected.contains("Remedy progress"), "{path}");
        assert!(!expected.contains("remedied"), "{path}");
        let (status, html) = page(&with, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(html, expected, "{path}");
    }
    // A campaign that has a report shows none on a server without them.
    for path in ["/recalls/nhtsa/24V456000", "/recalls/nhtsa/24V456000.md"] {
        let (_, html) = page(&without, path).await;
        assert!(!html.contains("Remedy progress"), "{path}");
    }
    // The other pages that name these campaigns do not change either.
    for path in [
        "/recalls",
        "/makes/hyundai/kona/2023/recalls",
        "/makes/hyundai/kona/2023",
        "/vin/KM8K2CAB4PU001140",
    ] {
        let (_, expected) = page(&without, path).await;
        let (_, html) = page(&with, path).await;
        assert_eq!(html, expected, "{path}");
    }
}

/// Runs the command-line tool in this process over the files in
/// `directory`, with no API to fall back on, and gives what it printed.
fn tool(directory: &Path, terminal: bool, args: &[&str]) -> String {
    let env = Env {
        data_dir: Some(directory.to_path_buf()),
        api: Some("http://127.0.0.1:1".to_owned()),
        releases: Some("http://127.0.0.1:1".to_owned()),
        stdout_terminal: terminal,
        ..Env::default()
    };
    let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();
    args.insert(0, OsString::from("wenmar-open"));
    let mut stdin: &[u8] = b"";
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = wenmar_open_cli::run(args, &env, &mut stdin, &mut stdout, &mut stderr);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    String::from_utf8(stdout).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tool_answers_a_campaign_from_files_with_the_report_the_api_gives() {
    let (app, files): (TestApp, TempDir) = common::reports_app_at(SITE).await;
    for (path, source, campaign) in [
        ("/v1/recalls/nhtsa/24V456000", "nhtsa", "24V456000"),
        ("/v1/recalls/nhtsa/19V888000", "nhtsa", "19v888000"),
        ("/v1/recalls/nhtsa/19V500000", "nhtsa", "19V500000"),
        ("/v1/recalls/nhtsa/23V123000", "nhtsa", "23V123000"),
        ("/v1/recalls/tc/2023101", "tc", "2023101"),
    ] {
        let served = ok(&app, path).await;
        let printed = tool(
            files.path(),
            false,
            &["recalls", "campaign", source, campaign, "--offline"],
        );
        let local: Value = serde_json::from_str(&printed).unwrap();
        assert_eq!(local, served, "{path}");
    }
    assert_eq!(
        ok(&app, "/v1/recalls/nhtsa/24V456000").await["completion"]["remedied_percent"],
        67
    );
    // At a terminal the report is a short table, with what it is not.
    let text = tool(
        files.path(),
        true,
        &["recalls", "campaign", "nhtsa", "24V456000", "--offline"],
    );
    assert!(
        text.contains(
            "\nRemedy progress the manufacturer reported\n\
Kind              annual\n\
Report number     2\n\
Period from       2025-2\n\
Period to         2026-1\n\
Submitted         2026-04-13\n\
Involved          800\n\
Remedied          536\n\
Remedied percent  67\n\
Unreachable       12\n\
Removed           3\n\
As of             2026-10-12\n\
Totals for the whole campaign at the end of that period, as filed with NHTSA. They do not say whether one vehicle has been remedied.\n"
        ),
        "{text}"
    );
    let text = tool(
        files.path(),
        true,
        &["recalls", "campaign", "nhtsa", "23V123000", "--offline"],
    );
    assert!(!text.contains("Remedy progress"), "{text}");
}
