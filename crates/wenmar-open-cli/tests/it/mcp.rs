use serde_json::{Value, json};
use wenmar_open_cli::env::Env;

use crate::common::{self, KONA};
use crate::server::{self, Reply};

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
    // The same two tools as the hosted endpoint, from the same definition.
    assert_eq!(names, ["wenmar_vin", "wenmar_vehicles"]);
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
