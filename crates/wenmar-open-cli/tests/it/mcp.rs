use serde_json::{Value, json};
use wenmar_open_cli::env::Env;

use crate::common::{self, KONA};
use crate::server::{self, Reply, Seen};

/// Sends each message on its own line and returns the answers, in order.
fn session(env: &Env, args: &[&str], messages: &[Value]) -> Vec<Value> {
    let mut input = String::new();
    for message in messages {
        input.push_str(&message.to_string());
        input.push('\n');
    }
    let run = common::run_with_input(env, args, input.as_bytes());
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stderr, "");
    run.stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}")))
        .collect()
}

fn call(id: u64, tool: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": tool, "arguments": arguments }
    })
}

/// The JSON a tool returned as text.
fn text(answer: &Value) -> Value {
    let result = &answer["result"];
    assert_eq!(result["content"][0]["type"], "text");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn a_client_can_initialize_and_list_the_tools() {
    let fixture = common::data_dir();
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "test", "version": "0" }
                }
            }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "id": "two", "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" }),
        ],
    );
    // Three answers: the notification gets none.
    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0]["jsonrpc"], "2.0");
    assert_eq!(answers[0]["id"], 1);
    assert_eq!(answers[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "wenmar-open");
    assert_eq!(
        answers[0]["result"]["capabilities"],
        json!({ "tools": { "listChanged": false } })
    );
    assert_eq!(answers[1]["id"], "two");
    let tools = answers[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    // The same tools as the hosted endpoint, from the same definition.
    assert_eq!(
        names,
        [
            "wenmar_vin",
            "wenmar_vehicles",
            "wenmar_recalls",
            "wenmar_bulletins"
        ]
    );
    assert_eq!(answers[1]["result"]["tools"], open_mcp::tools());
    assert_eq!(answers[2]["result"], json!({}));
}

#[test]
fn a_client_of_the_revision_without_a_handshake_is_answered_too() {
    let fixture = common::data_dir();
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": { "_meta": meta } }),
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "wenmar_vin", "arguments": { "action": "decode", "vin": KONA }, "_meta": meta }
            }),
        ],
    );
    assert_eq!(answers[0]["result"]["resultType"], "complete");
    assert_eq!(answers[0]["result"]["supportedVersions"][0], "2026-07-28");
    assert_eq!(answers[1]["result"]["resultType"], "complete");
    assert_eq!(answers[1]["result"]["structuredContent"]["model"], "Kona");
}

#[test]
fn the_tools_give_what_the_commands_give() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    let cases: Vec<(&str, Value, Vec<&str>)> = vec![
        (
            "wenmar_vin",
            json!({ "action": "decode", "vin": KONA }),
            vec!["vin", "decode", KONA],
        ),
        (
            "wenmar_vin",
            json!({ "action": "decode", "vin": KONA, "year": 1993 }),
            vec!["vin", "decode", KONA, "--year", "1993"],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "years" }),
            vec!["vehicles", "years"],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "makes", "year": 2019 }),
            vec!["vehicles", "makes", "--year", "2019"],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "models", "make": "chevy" }),
            vec!["vehicles", "models", "--make", "chevy"],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "submodels", "make": "honda", "model": "civic", "year": 2019 }),
            vec![
                "vehicles",
                "submodels",
                "--make",
                "honda",
                "--model",
                "civic",
                "--year",
                "2019",
            ],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "engines", "make": "honda", "model": "civic", "year": "2019", "submodel": "si" }),
            vec![
                "vehicles",
                "engines",
                "--make",
                "honda",
                "--model",
                "civic",
                "--year",
                "2019",
                "--submodel",
                "si",
            ],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "search", "query": "2019 civic si" }),
            vec!["vehicles", "search", "2019", "civic", "si"],
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "entry", "id": "2019_honda_civic_si" }),
            vec!["vehicles", "entry", "2019_honda_civic_si"],
        ),
    ];
    let messages: Vec<Value> = cases
        .iter()
        .enumerate()
        .map(|(index, (tool, arguments, _))| call(index as u64, tool, arguments.clone()))
        .collect();
    let answers = session(&env, &["mcp"], &messages);
    assert_eq!(answers.len(), cases.len());
    for (answer, (_, arguments, args)) in answers.iter().zip(&cases) {
        let printed = common::run(&env, args).json();
        assert_eq!(answer["result"]["isError"], false, "{arguments}");
        assert_eq!(text(answer), printed, "{arguments}");
        // A list is wrapped, because structured content must be an object.
        let structured = &answer["result"]["structuredContent"];
        if printed.is_array() {
            assert_eq!(structured["items"], printed, "{arguments}");
        } else {
            assert_eq!(*structured, printed, "{arguments}");
        }
    }
}

#[test]
fn a_batch_answers_in_order_with_decodes_and_errors() {
    let fixture = common::data_dir();
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[call(
            1,
            "wenmar_vin",
            json!({ "action": "batch", "vins": [KONA, "nope", "ZZZK2CAB4PU001140"] }),
        )],
    );
    let items = text(&answers[0]);
    assert_eq!(items[0]["model"], "Kona");
    assert_eq!(items[1]["error"]["code"], "invalid_vin");
    assert_eq!(items[2]["error"]["code"], "not_found");
    assert_eq!(answers[0]["result"]["structuredContent"]["items"], items);
}

#[test]
fn a_tool_that_fails_says_so_in_a_result_the_model_can_read() {
    let fixture = common::data_dir();
    let cases = [
        (
            "wenmar_vin",
            json!({ "action": "decode", "vin": "nope" }),
            "invalid_vin",
        ),
        (
            "wenmar_vin",
            json!({ "action": "decode" }),
            "validation_failed",
        ),
        (
            "wenmar_vin",
            json!({ "action": "explode" }),
            "validation_failed",
        ),
        ("wenmar_vin", json!({}), "validation_failed"),
        (
            "wenmar_vin",
            json!({ "action": "decode", "vin": 17 }),
            "validation_failed",
        ),
        (
            "wenmar_vin",
            json!({ "action": "batch", "vins": vec![KONA; 51] }),
            "validation_failed",
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "models" }),
            "validation_failed",
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "entry", "id": "x" }),
            "not_found",
        ),
        (
            "wenmar_vehicles",
            json!({ "action": "makes", "year": "soon" }),
            "validation_failed",
        ),
    ];
    let messages: Vec<Value> = cases
        .iter()
        .map(|(tool, arguments, _)| call(7, tool, arguments.clone()))
        .collect();
    let answers = session(&common::env(&fixture), &["mcp"], &messages);
    for (answer, (_, arguments, code)) in answers.iter().zip(&cases) {
        assert_eq!(answer["id"], 7);
        assert_eq!(answer["result"]["isError"], true, "{arguments}");
        assert_eq!(text(answer)["error"]["code"], *code, "{arguments}");
    }
}

#[test]
fn nothing_but_protocol_messages_is_written_whatever_is_read() {
    let fixture = common::data_dir();
    let mut input: Vec<u8> = Vec::new();
    input.extend_from_slice(b"not json\n");
    input.extend_from_slice(b"\n   \r\n");
    input.extend_from_slice(b"\xff\xfe\x00 bytes that are not text\n");
    input.extend_from_slice(b"[]\n");
    input.extend_from_slice(&vec![b'{'; 2 * 1024 * 1024]);
    input.push(b'\n');
    input.extend_from_slice(br#"{ "jsonrpc": "2.0", "id": 5, "method": "ping" }"#);
    input.extend_from_slice(b"\r\n");
    // The last message has no line ending: the client closed the stream.
    input.extend_from_slice(br#"{ "jsonrpc": "2.0", "id": 6, "method": "ping" }"#);

    let run = common::run_with_input(&common::env(&fixture), &["mcp"], &input);
    assert_eq!(run.code, 0);
    assert_eq!(run.stderr, "");
    let answers: Vec<Value> = run
        .stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let codes: Vec<Value> = answers
        .iter()
        .map(|answer| answer["error"]["code"].clone())
        .collect();
    assert_eq!(
        codes,
        [
            json!(-32700),
            json!(-32700),
            json!(-32600),
            json!(-32600),
            json!(null),
            json!(null)
        ]
    );
    assert_eq!(answers[4]["id"], 5);
    assert_eq!(answers[5]["id"], 6);
    for answer in &answers {
        assert_eq!(answer["jsonrpc"], "2.0");
    }
}

#[test]
fn an_empty_input_ends_the_server_at_once() {
    let fixture = common::data_dir();
    let run = common::run_with_input(&common::env(&fixture), &["mcp"], b"");
    assert_eq!(run.code, 0);
    assert_eq!(run.stdout, "");
}

#[test]
fn without_a_data_file_the_tools_ask_the_api() {
    let server = server::serve(|request| {
        if request.target == "/v1/vehicles/years" {
            Reply::json(200, &json!([2027, 2026]))
        } else {
            Reply::html(502, "<html>Bad gateway</html>")
        }
    });
    let fixture = common::empty_dir();
    let env = Env {
        api: Some(server.url().to_owned()),
        ..common::env(&fixture)
    };
    let answers = session(
        &env,
        &["mcp"],
        &[
            call(1, "wenmar_vehicles", json!({ "action": "years" })),
            call(2, "wenmar_vehicles", json!({ "action": "makes" })),
        ],
    );
    assert_eq!(text(&answers[0]), json!([2027, 2026]));
    // A failure of the API is the tool's failure, not the protocol's.
    assert_eq!(answers[1]["result"]["isError"], true);
    assert_eq!(text(&answers[1])["error"]["code"], "bad_response");

    // With nothing to answer from, each call says so and the server lives.
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[
            call(1, "wenmar_vehicles", json!({ "action": "years" })),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
        ],
    );
    assert_eq!(text(&answers[0])["error"]["code"], "network");
    assert_eq!(answers[1]["result"], json!({}));
}

#[test]
fn forced_offline_with_no_data_file_the_server_does_not_start() {
    let fixture = common::empty_dir();
    let run = common::run_with_input(
        &common::env(&fixture),
        &["mcp", "--offline"],
        br#"{ "jsonrpc": "2.0", "id": 1, "method": "ping" }"#,
    );
    assert_eq!(run.code, 11);
    assert_eq!(run.stdout, "");
    assert_eq!(run.error()["error"]["code"], "no_data");
}

fn recall_api(request: &Seen) -> Reply {
    let (path, _) = request
        .target
        .split_once('?')
        .unwrap_or((&request.target, ""));
    let answer = |name: &str| {
        json!({
            "may_apply": true,
            "route": name,
            "recalls": [],
            "lookup": { "nhtsa": "https://www.nhtsa.gov/recalls", "tc": "https://tc.example", "maker": null }
        })
    };
    match path {
        "/v1/vehicles/2023_hyundai_kona/recalls" => Reply::json(200, &answer("vehicle")),
        "/v1/vin/KM8K2CAB4PU001140/recalls" => Reply::json(200, &answer("vin")),
        "/v1/recalls/nhtsa/19V888000" => Reply::json(200, &answer("campaign")),
        "/v1/recalls" => Reply::json(200, &answer("search")),
        "/v1/recalls/tc/2023101" => Reply::json(
            404,
            &json!({ "error": { "code": "dataset_unavailable", "message": "The recalls_ca dataset is not available.", "details": { "dataset": "recalls_ca" } } }),
        ),
        // Something that is not the API's recall answer.
        "/v1/vehicles/2019_honda_civic/recalls" => Reply::json(200, &json!({ "recalls": [] })),
        _ => Reply::json(
            404,
            &json!({ "error": { "code": "not_found", "message": "There is nothing at this address.", "details": {} } }),
        ),
    }
}

#[test]
fn the_recalls_tool_asks_the_api_at_the_recall_routes() {
    let server = server::serve(recall_api);
    let fixture = common::empty_dir();
    let env = Env {
        api: Some(server.url().to_owned()),
        ..common::env(&fixture)
    };
    let answers = session(
        &env,
        &["mcp"],
        &[
            call(
                1,
                "wenmar_recalls",
                json!({ "action": "vehicle", "id": "2023_hyundai_kona", "country": "CA" }),
            ),
            call(
                2,
                "wenmar_recalls",
                json!({ "action": "vin", "vin": "km8k2-cab4pu001140", "year": 2023 }),
            ),
            call(
                3,
                "wenmar_recalls",
                json!({ "action": "campaign", "source": "nhtsa", "campaign": "19V888000" }),
            ),
            call(
                4,
                "wenmar_recalls",
                json!({ "action": "search", "make": "ford", "model": "f-150", "year": 2018, "component": "air bags", "query": "tail gate", "country": "us", "limit": 500, "cursor": "2019-12-12_41" }),
            ),
            call(5, "wenmar_recalls", json!({ "action": "search" })),
            // The server holds no Canadian recalls: its error is the tool's.
            call(
                6,
                "wenmar_recalls",
                json!({ "action": "campaign", "source": "tc", "campaign": "2023101" }),
            ),
            // Refused here, and never sent.
            call(
                7,
                "wenmar_recalls",
                json!({ "action": "campaign", "source": "nhtsa", "campaign": "../../v1/meta" }),
            ),
            call(
                8,
                "wenmar_recalls",
                json!({ "action": "vehicle", "id": "../meta" }),
            ),
            call(
                9,
                "wenmar_recalls",
                json!({ "action": "vin", "vin": "nope" }),
            ),
            call(10, "wenmar_recalls", json!({ "action": "explode" })),
            // An answer without may_apply did not come from the API.
            call(
                11,
                "wenmar_recalls",
                json!({ "action": "vehicle", "id": "2019_honda_civic" }),
            ),
        ],
    );
    for (index, route) in ["vehicle", "vin", "campaign", "search", "search"]
        .iter()
        .enumerate()
    {
        let result = &answers[index]["result"];
        assert_eq!(result["isError"], false, "{route}");
        assert_eq!(text(&answers[index])["route"], *route);
        assert_eq!(text(&answers[index])["may_apply"], true);
    }
    assert_eq!(
        server.targets(),
        [
            "/v1/vehicles/2023_hyundai_kona/recalls?country=ca",
            "/v1/vin/KM8K2CAB4PU001140/recalls?year=2023",
            "/v1/recalls/nhtsa/19V888000",
            "/v1/recalls?make=ford&model=f-150&year=2018&component=air%20bags&q=tail%20gate&country=us&limit=100&cursor=2019-12-12_41",
            "/v1/recalls?limit=25",
            "/v1/recalls/tc/2023101",
            "/v1/vehicles/2019_honda_civic/recalls"
        ]
    );
    let codes: Vec<&str> = answers[5..]
        .iter()
        .map(|answer| {
            assert_eq!(answer["result"]["isError"], true, "{answer}");
            answer["result"]["structuredContent"]["error"]["code"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(
        codes,
        [
            "dataset_unavailable",
            "not_found",
            "not_found",
            "invalid_vin",
            "validation_failed",
            "bad_response"
        ]
    );
    assert_eq!(
        answers[5]["result"]["structuredContent"]["error"]["message"],
        "The recalls_ca dataset is not available."
    );
}

#[test]
fn with_only_the_data_file_the_recalls_tool_says_what_to_run() {
    // The data file is here and holds no recalls: they are in a second
    // file, which `data pull --shop` downloads.
    let fixture = common::data_dir();
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[
            call(
                1,
                "wenmar_recalls",
                json!({ "action": "vehicle", "id": "2023_hyundai_kona" }),
            ),
            call(
                2,
                "wenmar_recalls",
                json!({ "action": "search", "country": "ca" }),
            ),
            call(
                3,
                "wenmar_recalls",
                json!({ "action": "campaign", "source": "tc", "campaign": "2023101" }),
            ),
            call(4, "wenmar_vin", json!({ "action": "decode", "vin": KONA })),
        ],
    );
    assert_eq!(
        answers[0]["result"]["structuredContent"],
        json!({
            "error": {
                "code": "dataset_unavailable",
                "message": "The recalls_us dataset is not available.",
                "details": { "dataset": "recalls_us", "hint": "Run: wenmar-open data pull --shop" }
            }
        })
    );
    assert_eq!(answers[0]["result"]["isError"], true);
    for index in [1, 2] {
        let error = &answers[index]["result"]["structuredContent"]["error"];
        assert_eq!(error["message"], "The recalls_ca dataset is not available.");
        assert_eq!(
            error["details"]["hint"],
            "Run: wenmar-open data pull --shop"
        );
    }
    // The other tools answer from the data file as before.
    assert_eq!(answers[3]["result"]["structuredContent"]["model"], "Kona");
}

fn overview_api(request: &Seen) -> Reply {
    let (path, _) = request
        .target
        .split_once('?')
        .unwrap_or((&request.target, ""));
    let overview = |name: &str| {
        json!({
            "route": name,
            "vehicle": null,
            "recalls": null,
            "links": { "page": null, "maker": null }
        })
    };
    match path {
        "/v1/vehicles/2023_hyundai_kona/overview" => Reply::json(200, &overview("vehicle")),
        "/v1/vin/KM8K2CAB4PU001140/overview" => Reply::json(200, &overview("vin")),
        "/v1/models/Land%20Rover/Range%20Rover%2FSport" => Reply::json(
            200,
            &json!({ "route": "model_years", "may_apply": true, "years": [] }),
        ),
        "/v1/recalls/recent" => Reply::json(
            200,
            &json!({ "route": "recent", "may_apply": true, "recalls": [] }),
        ),
        // Something that is not the API's overview.
        "/v1/vehicles/2019_honda_civic/overview" => Reply::json(200, &json!({ "vehicle": null })),
        _ => Reply::json(
            404,
            &json!({ "error": { "code": "not_found", "message": "There is nothing at this address.", "details": {} } }),
        ),
    }
}

#[test]
fn the_overview_actions_ask_the_api_at_their_routes() {
    let server = server::serve(overview_api);
    let fixture = common::empty_dir();
    let env = Env {
        api: Some(server.url().to_owned()),
        ..common::env(&fixture)
    };
    let answers = session(
        &env,
        &["mcp"],
        &[
            call(
                1,
                "wenmar_vehicles",
                json!({ "action": "overview", "id": "2023_hyundai_kona" }),
            ),
            call(
                2,
                "wenmar_vin",
                json!({ "action": "overview", "vin": "km8k2-cab4pu001140", "year": 2023 }),
            ),
            // A name with a space and a slash in it stays one part of the path.
            call(
                3,
                "wenmar_vehicles",
                json!({ "action": "model_years", "make": "Land Rover", "model": "Range Rover/Sport" }),
            ),
            call(
                4,
                "wenmar_recalls",
                json!({ "action": "recent", "since": "2026-09-01", "make": "ford", "country": "CA", "limit": 500 }),
            ),
            // Refused here, and never sent.
            call(
                5,
                "wenmar_vehicles",
                json!({ "action": "overview", "id": "../meta" }),
            ),
            call(
                6,
                "wenmar_vehicles",
                json!({ "action": "model_years", "make": "honda" }),
            ),
            call(
                7,
                "wenmar_vin",
                json!({ "action": "overview", "vin": "nope" }),
            ),
            // An answer without `links` did not come from the API.
            call(
                8,
                "wenmar_vehicles",
                json!({ "action": "overview", "id": "2019_honda_civic" }),
            ),
        ],
    );
    for (index, route) in ["vehicle", "vin", "model_years", "recent"]
        .iter()
        .enumerate()
    {
        assert_eq!(answers[index]["result"]["isError"], false, "{route}");
        assert_eq!(text(&answers[index])["route"], *route);
    }
    assert_eq!(
        server.targets(),
        [
            "/v1/vehicles/2023_hyundai_kona/overview",
            "/v1/vin/KM8K2CAB4PU001140/overview?year=2023",
            "/v1/models/Land%20Rover/Range%20Rover%2FSport",
            "/v1/recalls/recent?since=2026-09-01&country=ca&make=ford&limit=100",
            "/v1/vehicles/2019_honda_civic/overview"
        ]
    );
    let codes: Vec<&str> = answers[4..]
        .iter()
        .map(|answer| {
            assert_eq!(answer["result"]["isError"], true, "{answer}");
            answer["result"]["structuredContent"]["error"]["code"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(
        codes,
        [
            "not_found",
            "validation_failed",
            "invalid_vin",
            "bad_response"
        ]
    );
}

#[test]
fn with_only_the_data_file_the_overview_actions_answer_and_recent_says_what_to_run() {
    let fixture = common::data_dir();
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[
            call(
                1,
                "wenmar_vehicles",
                json!({ "action": "overview", "id": "2023_hyundai_kona" }),
            ),
            call(
                2,
                "wenmar_vin",
                json!({ "action": "overview", "vin": KONA }),
            ),
            call(
                3,
                "wenmar_vehicles",
                json!({ "action": "model_years", "make": "hyundai", "model": "kona" }),
            ),
            call(
                4,
                "wenmar_recalls",
                json!({ "action": "recent", "since": "2026-09-01" }),
            ),
        ],
    );
    for index in [0, 1, 2] {
        assert_eq!(answers[index]["result"]["isError"], false, "{index}");
    }
    assert_eq!(
        answers[0]["result"]["structuredContent"]["vehicle"]["id"],
        "2023_hyundai_kona"
    );
    assert_eq!(
        answers[0]["result"]["structuredContent"]["recalls"],
        Value::Null
    );
    assert_eq!(
        answers[1]["result"]["structuredContent"]["decode"]["vin"],
        KONA
    );
    assert_eq!(
        answers[2]["result"]["structuredContent"]["years"][0]["recalls"],
        Value::Null
    );
    assert_eq!(answers[3]["result"]["isError"], true);
    assert_eq!(
        answers[3]["result"]["structuredContent"]["error"]["details"]["hint"],
        "Run: wenmar-open data pull --shop"
    );
}

#[test]
fn the_safety_action_asks_the_api_at_its_route() {
    let server = server::serve(|request: &Seen| match request.target.as_str() {
        "/v1/vehicles/2023_hyundai_kona/safety" => {
            Reply::json(200, &json!({ "route": "safety", "ratings": [] }))
        }
        // Something that is not the API's answer: it has no list.
        "/v1/vehicles/2019_honda_civic/safety" => Reply::json(200, &json!({ "vehicle": null })),
        _ => Reply::json(
            404,
            &json!({ "error": { "code": "dataset_unavailable", "message": "The ratings dataset is not available.", "details": { "dataset": "ratings" } } }),
        ),
    });
    let fixture = common::empty_dir();
    let env = Env {
        api: Some(server.url().to_owned()),
        ..common::env(&fixture)
    };
    let answers = session(
        &env,
        &["mcp"],
        &[
            call(
                1,
                "wenmar_vehicles",
                json!({ "action": "safety", "id": "2023_hyundai_kona" }),
            ),
            // The server holds no ratings for this one: its error is the tool's.
            call(
                2,
                "wenmar_vehicles",
                json!({ "action": "safety", "id": "2022_hyundai_kona" }),
            ),
            // Refused here, and never sent.
            call(
                3,
                "wenmar_vehicles",
                json!({ "action": "safety", "id": "../meta" }),
            ),
            call(4, "wenmar_vehicles", json!({ "action": "safety" })),
            call(
                5,
                "wenmar_vehicles",
                json!({ "action": "safety", "id": "2019_honda_civic" }),
            ),
        ],
    );
    assert_eq!(answers[0]["result"]["isError"], false);
    assert_eq!(text(&answers[0])["route"], "safety");
    assert_eq!(
        server.targets(),
        [
            "/v1/vehicles/2023_hyundai_kona/safety",
            "/v1/vehicles/2022_hyundai_kona/safety",
            "/v1/vehicles/2019_honda_civic/safety"
        ]
    );
    let codes: Vec<&str> = answers[1..]
        .iter()
        .map(|answer| {
            assert_eq!(answer["result"]["isError"], true, "{answer}");
            answer["result"]["structuredContent"]["error"]["code"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(
        codes,
        [
            "dataset_unavailable",
            "not_found",
            "validation_failed",
            "bad_response"
        ]
    );
}

#[test]
fn with_only_the_data_file_the_safety_action_says_what_to_run() {
    let fixture = common::data_dir();
    let answers = session(
        &common::env(&fixture),
        &["mcp"],
        &[call(
            1,
            "wenmar_vehicles",
            json!({ "action": "safety", "id": "2023_hyundai_kona" }),
        )],
    );
    assert_eq!(answers[0]["result"]["isError"], true);
    assert_eq!(
        answers[0]["result"]["structuredContent"],
        json!({
            "error": {
                "code": "dataset_unavailable",
                "message": "The ratings dataset is not available.",
                "details": { "dataset": "ratings", "hint": "Run: wenmar-open data pull --shop" }
            }
        })
    );
}
