//! What the hosted `/mcp` endpoint and `wenmar-open mcp` have in common: the
//! two gateway tools as an MCP client sees them, and the shapes of the
//! messages that carry their results.
//!
//! Nothing here reads data or does I/O. Each server looks the answer up its
//! own way and hands the JSON to [`tool_result`] or [`tool_error`], so a
//! tool's definition and the shape of its result cannot differ between the
//! hosted server and the local one.

use serde_json::{Value, json};

/// The protocol revision that has no handshake: every request names its
/// version in `_meta`.
pub const MODERN_VERSIONS: [&str; 1] = ["2026-07-28"];

/// Revisions that open with an `initialize` handshake, newest first.
pub const LEGACY_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
/// A request named a protocol version this server does not speak.
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// The name both servers give themselves.
pub const SERVER_NAME: &str = "wenmar-open";

/// What a model is told about the server as a whole.
pub const INSTRUCTIONS: &str = "Wenmar Open is free vehicle data for auto repair shops, from NHTSA's vPIC. Use wenmar_vin to decode a VIN. Use wenmar_vehicles to step through year, make, model, submodel and engine, to search by free text, or to look up a vehicle id. Every call is read-only and needs no key.";

/// The names of the tools, in the order `tools/list` gives them.
pub const TOOL_NAMES: [&str; 2] = ["wenmar_vin", "wenmar_vehicles"];

/// The tools, as `tools/list` returns them.
pub fn tools() -> Value {
    json!([
        {
            "name": "wenmar_vin",
            "title": "VIN decoder",
            "description": "Decode vehicle identification numbers into year, make, model, trim, engine, drivetrain, safety equipment and the matching catalog entry. action=decode takes `vin` (and optionally `year` to override the model year). action=batch takes `vins`, at most 50, and returns one result per VIN in order. A wrong check digit is not an error: the result has valid=false and a warning.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["decode", "batch"] },
                    "vin": { "type": "string", "description": "A 17-character VIN. Spaces and dashes are ignored." },
                    "vins": { "type": "array", "items": { "type": "string" }, "maxItems": 50 },
                    "year": { "type": "integer", "description": "Model year to use instead of the one worked out from the VIN." }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_vehicles",
            "title": "Vehicle catalog",
            "description": "Look up vehicles by year, make, model, submodel and engine. Actions: years; makes (optional `year`); models (`make`, optional `year`); submodels (`make`, `model`, `year`); engines (`make`, `model`, `year`, optional `submodel`); search (`query`, such as \"2019 civic si\" or \"chevy 1500\"); entry (`id`, such as \"2019_honda_civic_si\"). `make` and `model` take a name, an alias or an id. `term` narrows a list to names starting with it. `scope` is light (cars, MPVs and trucks; the default), all, or a vPIC vehicle type id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["years", "makes", "models", "submodels", "engines", "search", "entry"] },
                    "year": { "type": "integer" },
                    "make": { "type": "string" },
                    "model": { "type": "string" },
                    "submodel": { "type": "string" },
                    "term": { "type": "string" },
                    "query": { "type": "string" },
                    "id": { "type": "string" },
                    "scope": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        }
    ])
}

/// A JSON-RPC answer that carries a result.
pub fn rpc_result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// A JSON-RPC answer that carries an error.
pub fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool's result: the JSON as text, and as structured content. A list is
/// wrapped as `{ "items": [...] }`, because the revisions with a handshake
/// require structured content to be an object.
pub fn tool_result(value: Value) -> Value {
    let text = value.to_string();
    let structured = match value {
        Value::Object(_) => value,
        other => json!({ "items": other }),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false
    })
}

/// A tool call that failed in a way the model can read and correct. `body`
/// is the API's error object, `{ "error": { "code", "message", "details" } }`.
pub fn tool_error(body: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": body.to_string() }],
        "structuredContent": body,
        "isError": true
    })
}

/// How a server names itself: `serverInfo` in a handshake, and
/// `_meta["io.modelcontextprotocol/serverInfo"]` without one.
pub fn server_info(version: &str) -> Value {
    json!({ "name": SERVER_NAME, "title": "Wenmar Open", "version": version })
}

/// The answer to `initialize`. The version is the client's when it is one
/// of `versions`, and otherwise the first of `versions`, which is the
/// newest the server speaks. `versions` must not be empty.
pub fn initialize(params: &Value, versions: &[&str], server_version: &str) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .and_then(|asked| versions.iter().find(|known| **known == asked))
        .or(versions.first())
        .copied()
        .unwrap_or_default();
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": server_info(server_version),
        "instructions": INSTRUCTIONS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_two_gateway_tools_and_each_needs_an_action() {
        let tools = tools();
        let tools = tools.as_array().unwrap();
        let names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, TOOL_NAMES);
        for tool in tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            assert_eq!(tool["inputSchema"]["required"], json!(["action"]));
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
        }
    }

    #[test]
    fn a_list_is_wrapped_and_an_object_is_not() {
        let list = tool_result(json!([2020, 2019]));
        assert_eq!(list["content"][0]["text"], "[2020,2019]");
        assert_eq!(list["structuredContent"], json!({ "items": [2020, 2019] }));
        assert_eq!(list["isError"], false);
        let object = tool_result(json!({ "id": "x" }));
        assert_eq!(object["structuredContent"], json!({ "id": "x" }));
    }

    #[test]
    fn a_failed_tool_carries_the_error_object_both_ways() {
        let body = json!({ "error": { "code": "not_found", "message": "No.", "details": {} } });
        let result = tool_error(body.clone());
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"], body);
        let text: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, body);
    }

    #[test]
    fn initialize_answers_a_known_version_with_itself_and_an_unknown_one_with_the_newest() {
        let known = initialize(
            &json!({ "protocolVersion": "2025-06-18" }),
            &LEGACY_VERSIONS,
            "0.1.0",
        );
        assert_eq!(known["protocolVersion"], "2025-06-18");
        assert_eq!(known["serverInfo"]["name"], "wenmar-open");
        assert_eq!(known["serverInfo"]["version"], "0.1.0");
        assert_eq!(
            known["capabilities"],
            json!({ "tools": { "listChanged": false } })
        );
        for params in [
            json!({ "protocolVersion": "1999-01-01" }),
            json!({}),
            json!(null),
            json!({ "protocolVersion": 7 }),
        ] {
            let answer = initialize(&params, &LEGACY_VERSIONS, "0.1.0");
            assert_eq!(answer["protocolVersion"], "2025-11-25", "{params}");
        }
        // A server with no versions still answers, with an empty one.
        assert_eq!(initialize(&json!({}), &[], "0")["protocolVersion"], "");
    }
}
