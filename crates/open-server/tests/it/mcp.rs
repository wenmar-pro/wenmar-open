use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};

use crate::common::{self, TestApp, body_json, body_text, header};

async fn rpc(app: &TestApp, message: Value) -> Value {
    let response = app.post_json("/mcp", &message.to_string()).await;
    assert_eq!(response.status(), StatusCode::OK, "{message}");
    assert!(header(&response, "content-type").starts_with("application/json"));
    body_json(response).await
}

async fn call(app: &TestApp, tool: &str, arguments: Value) -> Value {
    let answer = rpc(
        app,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }),
    )
    .await;
    assert_eq!(answer["id"], 7);
    answer["result"].clone()
}

/// The JSON a tool returned as text.
fn text(result: &Value) -> Value {
    assert_eq!(result["content"][0]["type"], "text");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn a_client_can_initialize_and_list_the_tools() {
    let app = common::app().await;
    let answer = rpc(
        &app,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" }
            }
        }),
    )
    .await;
    assert_eq!(answer["jsonrpc"], "2.0");
    assert_eq!(answer["id"], 1);
    assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(answer["result"]["serverInfo"]["name"], "wenmar-open");
    assert_eq!(
        answer["result"]["capabilities"],
        json!({ "tools": { "listChanged": false } })
    );

    // The notification that follows is accepted with no body.
    let response = app
        .post_json(
            "/mcp",
            r#"{ "jsonrpc": "2.0", "method": "notifications/initialized" }"#,
        )
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(body_text(response).await, "");

    let answer = rpc(
        &app,
        json!({ "jsonrpc": "2.0", "id": "two", "method": "tools/list" }),
    )
    .await;
    assert_eq!(answer["id"], "two");
    let tools = answer["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    // A few gateway tools, one per noun: not one per endpoint.
    assert_eq!(names, ["wenmar_vin", "wenmar_vehicles"]);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["required"], json!(["action"]));
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
}

#[tokio::test]
async fn an_unknown_protocol_version_is_answered_with_the_newest_known() {
    let app = common::app().await;
    let answer = rpc(
        &app,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "1999-01-01" }
        }),
    )
    .await;
    assert_eq!(
        answer["result"]["protocolVersion"],
        open_server::mcp::PROTOCOL_VERSIONS[0]
    );
    let answer = rpc(&app, json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })).await;
    assert_eq!(answer["result"], json!({}));
}

#[tokio::test]
async fn the_vin_tool_gives_what_the_api_gives() {
    let app = common::app().await;
    let (_, served) = app.json("/v1/vin/KM8K2CAB4PU001140").await;
    let result = call(
        &app,
        "wenmar_vin",
        json!({ "action": "decode", "vin": "KM8K2CAB4PU001140" }),
    )
    .await;
    assert_eq!(result["isError"], false);
    assert_eq!(text(&result), served);
    assert_eq!(result["structuredContent"], served);

    let result = call(
        &app,
        "wenmar_vin",
        json!({ "action": "batch", "vins": ["KM8K2CAB4PU001140", "nope"] }),
    )
    .await;
    let items = text(&result);
    assert_eq!(items[0]["model"], "Kona");
    assert_eq!(items[1]["error"]["code"], "invalid_vin");
    // A list is wrapped, because structured content must be an object.
    assert_eq!(result["structuredContent"]["items"], items);
}

#[tokio::test]
async fn the_vehicles_tool_gives_what_the_api_gives() {
    let app = common::app().await;
    let cases = [
        (json!({ "action": "years" }), "/v1/vehicles/years"),
        (
            json!({ "action": "makes", "year": 2019 }),
            "/v1/vehicles/makes?year=2019",
        ),
        (
            json!({ "action": "models", "make": "chevy" }),
            "/v1/vehicles/models?make=chevy",
        ),
        (
            json!({ "action": "submodels", "make": "honda", "model": "civic", "year": 2019 }),
            "/v1/vehicles/submodels?make=honda&model=civic&year=2019",
        ),
        (
            json!({ "action": "engines", "make": "honda", "model": "civic", "year": "2019", "submodel": "si" }),
            "/v1/vehicles/engines?make=honda&model=civic&year=2019&submodel=si",
        ),
        (
            json!({ "action": "search", "query": "2019 civic si" }),
            "/v1/vehicles/search?q=2019+civic+si",
        ),
        (
            json!({ "action": "entry", "id": "2019_honda_civic_si" }),
            "/v1/vehicles/2019_honda_civic_si",
        ),
    ];
    for (arguments, path) in cases {
        let (status, served) = app.json(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        let result = call(&app, "wenmar_vehicles", arguments).await;
        assert_eq!(result["isError"], false, "{path}");
        assert_eq!(text(&result), served, "{path}");
    }
}

#[tokio::test]
async fn a_tool_that_fails_says_so_in_a_result_the_model_can_read() {
    let app = common::app().await;
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
            json!({ "action": "batch", "vins": vec!["KM8K2CAB4PU001140"; 51] }),
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
    for (tool, arguments, code) in cases {
        let result = call(&app, tool, arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}");
        assert_eq!(text(&result)["error"]["code"], code, "{arguments}");
    }
}

#[tokio::test]
async fn messages_that_are_not_valid_get_json_rpc_errors() {
    let app = common::app().await;
    let answer = rpc(
        &app,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }),
    )
    .await;
    assert_eq!(answer["error"]["code"], -32601);
    let answer = rpc(
        &app,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "rm_rf" } }),
    )
    .await;
    assert_eq!(answer["error"]["code"], -32602);
    let answer = rpc(
        &app,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "wenmar_vin", "arguments": [1] } }),
    )
    .await;
    assert_eq!(answer["error"]["code"], -32602);

    for (body, code) in [
        ("not json", -32700),
        ("", -32700),
        ("[]", -32600),
        (
            r#"[{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]"#,
            -32600,
        ),
        ("42", -32600),
        ("{}", -32600),
        (r#"{ "id": 1 }"#, -32600),
    ] {
        let response = app.post_json("/mcp", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
        let answer = body_json(response).await;
        assert_eq!(answer["error"]["code"], code, "{body}");
        assert_eq!(answer["id"], Value::Null, "{body}");
    }
}

#[tokio::test]
async fn there_are_no_streams_or_sessions() {
    let app = common::app().await;
    let response = app.get("/mcp").await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header(&response, "allow"), "POST");
    let response = app
        .send(Request::delete("/mcp").body(Body::empty()).unwrap())
        .await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    // No session id and no cookie are ever handed out.
    let response = app
        .post_json(
            "/mcp",
            r#"{ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }"#,
        )
        .await;
    assert!(response.headers().get("mcp-session-id").is_none());
    assert!(response.headers().get("set-cookie").is_none());
    assert_eq!(header(&response, "cache-control"), "no-store");
}

#[tokio::test]
async fn an_oversized_message_is_refused() {
    let app = common::app().await;
    let huge = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": "wenmar_vin", "arguments": { "action": "decode", "vin": "A".repeat(1_000_000) } }
    });
    let response = app.post_json("/mcp", &huge.to_string()).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn llms_txt_points_at_the_api_and_the_mcp_endpoint() {
    let app = common::app().await;
    let response = app.get("/llms.txt").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header(&response, "content-type").starts_with("text/plain"));
    let text = body_text(response).await;
    assert!(text.starts_with("# Wenmar Open\n\n> "), "{text}");
    assert!(text.contains("https://open.example/v1/openapi.json"));
    assert!(text.contains("https://open.example/mcp"));
    assert!(text.contains("Data version 2026.09"));
    assert!(text.contains("600 requests a minute"));
}
