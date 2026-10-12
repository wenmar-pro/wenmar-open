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
    assert_eq!(names, ["wenmar_vin", "wenmar_vehicles", "wenmar_recalls"]);
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

/// Claude's connector dialog takes the bare domain. A refusal there reads
/// to the client as a sign-in prompt, so the site's own address and `/mcp/`
/// answer as `/mcp` does, and the home page is still a page.
#[tokio::test]
async fn the_bare_domain_and_a_trailing_slash_are_the_mcp_endpoint_too() {
    let app = common::app().await;
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {} }
    })
    .to_string();
    for path in ["/", "/mcp/"] {
        let response = app.post_json(path, &initialize).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert!(header(&response, "content-type").starts_with("application/json"));
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
        let answer = body_json(response).await;
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18", "{path}");
        assert_eq!(answer["result"]["serverInfo"]["name"], "wenmar-open");
    }
    let response = app.post_json("/", "not json").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], -32700);
    let response = app.get("/mcp/").await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let response = app.get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header(&response, "content-type").starts_with("text/html"));
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
    assert!(text.contains("`wenmar_recalls` (actions `vehicle`, `vin`, `campaign`, `search`)"));
    assert!(text.contains("confirm by VIN with the manufacturer"));
}

const MODERN: &str = "2026-07-28";
const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const SERVER_KEY: &str = "io.modelcontextprotocol/serverInfo";

fn modern_meta() -> Value {
    json!({
        VERSION_KEY: MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "0" }
    })
}

/// `POST`s one message with the given headers.
async fn raw(app: &TestApp, headers: &[(&str, &str)], message: &Value) -> (StatusCode, Value) {
    let mut request = Request::post("/mcp")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .send(request.body(Body::from(message.to_string())).unwrap())
        .await;
    assert!(response.headers().get("mcp-session-id").is_none());
    assert!(response.headers().get("set-cookie").is_none());
    assert_eq!(header(&response, "cache-control"), "no-store");
    let status = response.status();
    (status, body_json(response).await)
}

/// A request of the revision with no handshake, with the headers that
/// revision requires over HTTP.
async fn modern(app: &TestApp, method: &str, mut params: Value) -> (StatusCode, Value) {
    params["_meta"] = modern_meta();
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let mut headers = vec![("mcp-protocol-version", MODERN), ("mcp-method", method)];
    let name = message["params"]["name"].as_str().map(str::to_owned);
    if let Some(name) = &name {
        headers.push(("mcp-name", name.as_str()));
    }
    raw(app, &headers, &message).await
}

#[tokio::test]
async fn a_client_with_no_handshake_discovers_lists_and_calls() {
    let app = common::app().await;
    let (status, answer) = modern(&app, "server/discover", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let found = &answer["result"];
    assert_eq!(
        found["supportedVersions"],
        json!(["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"])
    );
    assert_eq!(found["resultType"], "complete");
    assert_eq!(found["_meta"][SERVER_KEY]["name"], "wenmar-open");
    assert_eq!(
        found["capabilities"],
        json!({ "tools": { "listChanged": false } })
    );
    assert_eq!(found["cacheScope"], "public");
    assert!(
        found["instructions"]
            .as_str()
            .unwrap()
            .contains("wenmar_vin")
    );

    let (status, answer) = modern(&app, "tools/list", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = answer["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["wenmar_vin", "wenmar_vehicles", "wenmar_recalls"]);
    assert_eq!(answer["result"]["resultType"], "complete");
    assert_eq!(answer["result"]["ttlMs"], 3_600_000);

    let (status, answer) = modern(
        &app,
        "tools/call",
        json!({ "name": "wenmar_vin", "arguments": { "action": "decode", "vin": common::KONA } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let result = &answer["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["_meta"][SERVER_KEY]["name"], "wenmar-open");
    // The tool gives what the API gives.
    let (_, served) = app.json("/v1/vin/KM8K2CAB4PU001140").await;
    assert_eq!(text(result), served);
    // A tool that fails still completes, with the error to read.
    let (status, answer) = modern(
        &app,
        "tools/call",
        json!({ "name": "wenmar_vin", "arguments": { "action": "decode", "vin": "nope" } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["result"]["isError"], true);
    assert_eq!(answer["result"]["resultType"], "complete");
}

#[tokio::test]
async fn headers_that_do_not_say_what_the_body_says_are_refused() {
    let app = common::app().await;
    let list = json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": { "_meta": modern_meta() }
    });
    let call = json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {
            "_meta": modern_meta(),
            "name": "wenmar_vin",
            "arguments": { "action": "decode", "vin": common::KONA }
        }
    });
    let cases: [(&[(&str, &str)], &Value); 7] = [
        // No version header, and one that names another version.
        (&[("mcp-method", "tools/list")], &list),
        (
            &[
                ("mcp-protocol-version", "2025-11-25"),
                ("mcp-method", "tools/list"),
            ],
            &list,
        ),
        // No method header, and one that names another method.
        (&[("mcp-protocol-version", MODERN)], &list),
        (
            &[
                ("mcp-protocol-version", MODERN),
                ("mcp-method", "tools/call"),
            ],
            &list,
        ),
        // A tool call with no name header, another tool's name, and a name
        // that is not base64 where it says it is.
        (
            &[
                ("mcp-protocol-version", MODERN),
                ("mcp-method", "tools/call"),
            ],
            &call,
        ),
        (
            &[
                ("mcp-protocol-version", MODERN),
                ("mcp-method", "tools/call"),
                ("mcp-name", "wenmar_vehicles"),
            ],
            &call,
        ),
        (
            &[
                ("mcp-protocol-version", MODERN),
                ("mcp-method", "tools/call"),
                ("mcp-name", "=?base64?!!!?="),
            ],
            &call,
        ),
    ];
    for (headers, message) in cases {
        let (status, answer) = raw(&app, headers, message).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{headers:?}");
        assert_eq!(answer["error"]["code"], -32020, "{headers:?}");
        assert_eq!(answer["id"], 5);
    }
    // The same call with the name in base64 is answered.
    let (status, answer) = raw(
        &app,
        &[
            ("MCP-Protocol-Version", MODERN),
            ("Mcp-Method", "tools/call"),
            ("Mcp-Name", "=?base64?d2VubWFyX3Zpbg==?="),
        ],
        &call,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["result"]["isError"], false);
}

#[tokio::test]
async fn a_version_this_server_does_not_speak_is_refused_with_the_ones_it_does() {
    let app = common::app().await;
    // In the body.
    let mut meta = modern_meta();
    meta[VERSION_KEY] = json!("2027-01-01");
    let message = json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/list", "params": { "_meta": meta }
    });
    let (status, answer) = raw(
        &app,
        &[
            ("mcp-protocol-version", "2027-01-01"),
            ("mcp-method", "tools/list"),
        ],
        &message,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(answer["error"]["code"], -32022);
    assert_eq!(answer["error"]["data"]["requested"], "2027-01-01");
    assert_eq!(
        answer["error"]["data"]["supported"],
        json!(["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"])
    );
    // The capabilities are required beside the version.
    let message = json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/list",
        "params": { "_meta": { VERSION_KEY: MODERN } }
    });
    let (status, answer) = raw(
        &app,
        &[
            ("mcp-protocol-version", MODERN),
            ("mcp-method", "tools/list"),
        ],
        &message,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(answer["error"]["code"], -32602);
    // A method this server does not have is 404 in this revision.
    let (status, answer) = modern(&app, "resources/list", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(answer["error"]["code"], -32601);

    // In the header alone, from a client of the handshake kind.
    let list = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/list" });
    let (status, answer) = raw(&app, &[("mcp-protocol-version", "1999-01-01")], &list).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(answer["error"]["code"], -32022);
    assert_eq!(answer["error"]["data"]["requested"], "1999-01-01");
    // The header promises the new revision and the body does not keep it.
    let (status, answer) = raw(&app, &[("mcp-protocol-version", MODERN)], &list).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(answer["error"]["code"], -32602);
    // A probe for the new revision from a client of the old kind.
    let probe = json!({ "jsonrpc": "2.0", "id": 9, "method": "server/discover" });
    let (status, answer) = raw(&app, &[], &probe).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(answer["error"]["code"], -32602);
}

#[tokio::test]
async fn a_client_that_shakes_hands_is_answered_as_before() {
    let app = common::app().await;
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    // With no version header, and with the header such a client sends after
    // the handshake.
    for headers in [&[][..], &[("mcp-protocol-version", "2025-06-18")][..]] {
        let (status, answer) = raw(&app, headers, &list).await;
        assert_eq!(status, StatusCode::OK, "{headers:?}");
        assert_eq!(answer["result"]["tools"].as_array().unwrap().len(), 3);
        assert!(answer["result"].get("resultType").is_none(), "{answer}");
        assert!(answer["result"].get("_meta").is_none());
    }
    // `initialize` is the handshake whatever header comes with it.
    let hello = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-11-25", "capabilities": {} }
    });
    for headers in [
        &[][..],
        &[("mcp-protocol-version", "2025-11-25")][..],
        &[("mcp-protocol-version", MODERN)][..],
        &[("mcp-protocol-version", "1999-01-01")][..],
    ] {
        let (status, answer) = raw(&app, headers, &hello).await;
        assert_eq!(status, StatusCode::OK, "{headers:?}");
        assert_eq!(answer["result"]["protocolVersion"], "2025-11-25");
        assert!(answer["result"].get("resultType").is_none());
    }
    // A method the server does not have stays a 200 with an error in it.
    let unknown = json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" });
    let (status, answer) = raw(&app, &[], &unknown).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["error"]["code"], -32601);
}

#[tokio::test]
async fn a_request_from_any_origin_is_answered() {
    let app = common::app().await;
    // The service is public, keyless and read-only, so no origin is refused.
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    let (status, _) = raw(
        &app,
        &[("origin", "https://some-other-site.example")],
        &list,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = modern(&app, "tools/list", json!({})).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_recalls_tool_gives_what_the_api_gives() {
    let app = common::recalls_app().await;
    let cases = [
        (
            json!({ "action": "vehicle", "id": "2023_hyundai_kona" }),
            "/v1/vehicles/2023_hyundai_kona/recalls",
        ),
        (
            json!({ "action": "vehicle", "id": "2023_hyundai_kona", "country": "ca" }),
            "/v1/vehicles/2023_hyundai_kona/recalls?country=ca",
        ),
        (
            json!({ "action": "vin", "vin": common::KONA }),
            "/v1/vin/KM8K2CAB4PU001140/recalls",
        ),
        (
            json!({ "action": "vin", "vin": common::KONA, "year": 2023, "country": "us" }),
            "/v1/vin/KM8K2CAB4PU001140/recalls?year=2023&country=us",
        ),
        (
            json!({ "action": "vin", "vin": common::COACH }),
            "/v1/vin/1M8PDMPA9SP000001/recalls",
        ),
        (
            json!({ "action": "campaign", "source": "nhtsa", "campaign": "19V888000" }),
            "/v1/recalls/nhtsa/19V888000",
        ),
        (
            json!({ "action": "campaign", "source": "tc", "campaign": "2023101" }),
            "/v1/recalls/tc/2023101",
        ),
        (json!({ "action": "search" }), "/v1/recalls"),
        (
            json!({ "action": "search", "make": "hyundai", "year": 2023, "limit": 2 }),
            "/v1/recalls?make=hyundai&year=2023&limit=2",
        ),
        (
            json!({ "action": "search", "query": "harness", "country": "ca" }),
            "/v1/recalls?q=harness&country=ca",
        ),
        (
            json!({ "action": "search", "make": "chevy", "model": "silverado", "component": "battery" }),
            "/v1/recalls?make=chevy&model=silverado&component=battery",
        ),
    ];
    for (arguments, path) in cases {
        let result = call(&app, "wenmar_recalls", arguments.clone()).await;
        assert_eq!(result["isError"], false, "{arguments}");
        let (status, served) = app.json(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(text(&result), served, "{arguments}");
        assert_eq!(result["structuredContent"], served, "{arguments}");
        // Every answer says "may apply" and where a VIN can be checked.
        assert_eq!(served["may_apply"], true, "{path}");
        assert_eq!(served["lookup"]["nhtsa"], "https://www.nhtsa.gov/recalls");
        assert!(served["lookup"]["tc"].is_string(), "{path}");
    }
    // The second page of a list is asked for with the cursor of the first.
    let first = call(
        &app,
        "wenmar_recalls",
        json!({ "action": "search", "limit": 2 }),
    )
    .await;
    let cursor = text(&first)["next_cursor"].as_str().unwrap().to_owned();
    let second = call(
        &app,
        "wenmar_recalls",
        json!({ "action": "search", "limit": 2, "cursor": cursor }),
    )
    .await;
    let (_, served) = app
        .json(&format!("/v1/recalls?limit=2&cursor={cursor}"))
        .await;
    assert_eq!(text(&second), served);
}

#[tokio::test]
async fn the_recalls_tool_fails_as_the_api_does() {
    let app = common::recalls_app().await;
    let cases = [
        (json!({ "action": "vehicle" }), "validation_failed"),
        (json!({ "action": "vin" }), "validation_failed"),
        (
            json!({ "action": "campaign", "source": "nhtsa" }),
            "validation_failed",
        ),
        (json!({ "action": "recall" }), "validation_failed"),
        (json!({}), "validation_failed"),
        (
            json!({ "action": "vehicle", "id": "2023_hyundai_kona", "country": "mx" }),
            "validation_failed",
        ),
        (
            json!({ "action": "vehicle", "id": "2023_hyundai_nothing" }),
            "not_found",
        ),
        (
            json!({ "action": "vin", "vin": "KM8K2CAB4PUO01140" }),
            "invalid_vin",
        ),
        (
            json!({ "action": "campaign", "source": "epa", "campaign": "19V888000" }),
            "not_found",
        ),
        (
            json!({ "action": "campaign", "source": "nhtsa", "campaign": "00V000000" }),
            "not_found",
        ),
        (
            json!({ "action": "search", "model": "kona" }),
            "validation_failed",
        ),
        (
            json!({ "action": "search", "cursor": "nonsense" }),
            "validation_failed",
        ),
    ];
    for (arguments, code) in cases {
        let result = call(&app, "wenmar_recalls", arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}");
        assert_eq!(
            result["structuredContent"]["error"]["code"], code,
            "{arguments}"
        );
        assert_eq!(text(&result), result["structuredContent"], "{arguments}");
    }
}

#[tokio::test]
async fn a_server_with_no_recall_data_answers_the_recalls_tool_with_a_tool_error() {
    // No shop file at all.
    let app = common::app().await;
    for arguments in [
        json!({ "action": "vehicle", "id": "2023_hyundai_kona" }),
        json!({ "action": "vin", "vin": common::KONA }),
        json!({ "action": "campaign", "source": "nhtsa", "campaign": "19V888000" }),
        json!({ "action": "search" }),
    ] {
        let result = call(&app, "wenmar_recalls", arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}");
        assert_eq!(
            result["structuredContent"],
            json!({
                "error": {
                    "code": "dataset_unavailable",
                    "message": "The recalls_us dataset is not available.",
                    "details": { "dataset": "recalls_us" }
                }
            }),
            "{arguments}"
        );
    }
    // The other two tools are as they were.
    let decoded = call(
        &app,
        "wenmar_vin",
        json!({ "action": "decode", "vin": common::KONA }),
    )
    .await;
    assert_eq!(decoded["isError"], false);

    // A server with one country's recalls names the other when it is asked for.
    let app = common::app_with_shop(common::RECALL_DATASETS_US, &common::recall_rows()).await;
    let result = call(
        &app,
        "wenmar_recalls",
        json!({ "action": "vehicle", "id": "2023_hyundai_kona", "country": "ca" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert_eq!(
        result["structuredContent"]["error"]["message"],
        "The recalls_ca dataset is not available."
    );
    let result = call(
        &app,
        "wenmar_recalls",
        json!({ "action": "vehicle", "id": "2023_hyundai_kona" }),
    )
    .await;
    assert_eq!(result["isError"], false);
    assert_eq!(text(&result)["as_of"]["ca"], Value::Null);
}
