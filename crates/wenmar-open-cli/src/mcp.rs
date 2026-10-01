//! `wenmar-open mcp`: an MCP server on standard input and output.
//!
//! One JSON-RPC message per line in, one per line out, and nothing else on
//! standard output. It offers the two tools of the hosted `/mcp` endpoint,
//! defined once in the `open-mcp` crate, and answers them from the local
//! data file or the hosted API like every other command.
//!
//! It speaks both kinds of protocol revision. A client that opens with
//! `initialize` gets the handshake of the revisions up to 2025-11-25. A
//! client that names its version in each request's `_meta`, as 2026-07-28
//! does, is answered without one.

use std::io::{BufRead, Write};

use open_mcp::{
    INVALID_PARAMS, INVALID_REQUEST, LEGACY_VERSIONS, METHOD_NOT_FOUND, MODERN_VERSIONS,
    PARSE_ERROR, TOOL_NAMES, UNSUPPORTED_PROTOCOL_VERSION, rpc_error, rpc_result,
};
use serde_json::{Value, json};

use crate::backend::Backend;
use crate::error::{CliError, IO};
use crate::request::Request;

/// The longest message read, in bytes. The longest a client has reason to
/// send is a batch of 50 VINs.
pub const LONGEST_MESSAGE: usize = 1024 * 1024;

const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
const SERVER_KEY: &str = "io.modelcontextprotocol/serverInfo";
/// How long a client may keep the list of tools, in milliseconds. It never
/// changes while the process runs.
const TOOLS_TTL_MS: u64 = 3_600_000;

fn server_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Every version this server speaks, newest first.
fn supported() -> Vec<&'static str> {
    MODERN_VERSIONS
        .iter()
        .chain(LEGACY_VERSIONS.iter())
        .copied()
        .collect()
}

/// Runs one `tools/call`. `Err` is a protocol error; a tool that fails
/// answers `Ok` with `isError`.
fn call_tool(
    params: &Value,
    call: &mut dyn FnMut(&str, &Value) -> Result<Value, CliError>,
) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !TOOL_NAMES.contains(&name) {
        return Err((INVALID_PARAMS, format!("Unknown tool: {name:.60}")));
    }
    let empty = json!({});
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(arguments) if arguments.is_object() => arguments,
        Some(_) => return Err((INVALID_PARAMS, "arguments must be an object".to_owned())),
    };
    Ok(match call(name, arguments) {
        Ok(value) => open_mcp::tool_result(value),
        Err(error) => open_mcp::tool_error(error.body()),
    })
}

/// Answers one message. `None` when the message needs no answer.
///
/// `call` runs a tool: it is given the tool's name, which is one of
/// [`TOOL_NAMES`], and its arguments, which are an object.
pub fn handle(
    line: &[u8],
    call: &mut dyn FnMut(&str, &Value) -> Result<Value, CliError>,
) -> Option<Value> {
    let null = Value::Null;
    let Ok(message) = serde_json::from_slice::<Value>(line) else {
        return Some(rpc_error(&null, PARSE_ERROR, "The message is not JSON."));
    };
    if message.is_array() {
        return Some(rpc_error(
            &null,
            INVALID_REQUEST,
            "Batches are not supported. Send one message.",
        ));
    }
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(rpc_error(&null, INVALID_REQUEST, "Not a JSON-RPC request."));
    };
    // A message with no id is a notification, such as
    // `notifications/initialized`. It gets no answer.
    let id = match message.get("id") {
        None | Some(Value::Null) => return None,
        Some(id) if id.is_string() || id.is_number() => id,
        Some(_) => {
            return Some(rpc_error(
                &null,
                INVALID_REQUEST,
                "id must be text or a number.",
            ));
        }
    };
    let params = message.get("params").unwrap_or(&null);
    let meta = params.get("_meta").unwrap_or(&null);

    // A request that names its protocol version is of the revisions with
    // no handshake.
    let modern = match meta.get(VERSION_KEY) {
        None => false,
        Some(asked) => {
            if !asked
                .as_str()
                .is_some_and(|asked| MODERN_VERSIONS.contains(&asked))
            {
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": UNSUPPORTED_PROTOCOL_VERSION,
                        "message": "Unsupported protocol version",
                        "data": { "supported": supported(), "requested": asked }
                    }
                }));
            }
            if !meta.get(CAPABILITIES_KEY).is_some_and(Value::is_object) {
                return Some(rpc_error(
                    id,
                    INVALID_PARAMS,
                    "_meta must carry io.modelcontextprotocol/clientCapabilities.",
                ));
            }
            true
        }
    };

    let outcome = match method {
        "initialize" => Ok(open_mcp::initialize(
            params,
            &LEGACY_VERSIONS,
            server_version(),
        )),
        "server/discover" if modern => Ok(json!({
            "supportedVersions": supported(),
            "capabilities": { "tools": { "listChanged": false } },
            "instructions": open_mcp::INSTRUCTIONS,
            "ttlMs": TOOLS_TTL_MS,
            "cacheScope": "public"
        })),
        "server/discover" => Err((INVALID_PARAMS, format!("_meta must carry {VERSION_KEY}."))),
        "ping" => Ok(json!({})),
        "tools/list" if modern => Ok(json!({
            "tools": open_mcp::tools(),
            "ttlMs": TOOLS_TTL_MS,
            "cacheScope": "public"
        })),
        "tools/list" => Ok(json!({ "tools": open_mcp::tools() })),
        "tools/call" => call_tool(params, call),
        _ => Err((METHOD_NOT_FOUND, "Method not found.".to_owned())),
    };
    Some(match outcome {
        Ok(mut result) => {
            if let (true, Some(object)) = (modern, result.as_object_mut()) {
                object.insert("resultType".to_owned(), json!("complete"));
                object.insert(
                    "_meta".to_owned(),
                    json!({ SERVER_KEY: open_mcp::server_info(server_version()) }),
                );
            }
            rpc_result(id, result)
        }
        Err((code, message)) => rpc_error(id, code, &message),
    })
}

/// One line of input.
enum Line {
    /// The input has ended.
    End,
    /// A whole line, without its line ending.
    Message(Vec<u8>),
    /// A line longer than [`LONGEST_MESSAGE`]. It was read to its end and
    /// thrown away.
    TooLong,
}

/// Reads one line without ever holding more than [`LONGEST_MESSAGE`] bytes.
fn read_line(input: &mut dyn BufRead) -> std::io::Result<Line> {
    let mut line = Vec::new();
    let mut too_long = false;
    loop {
        let available = match input.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            return Ok(match (too_long, line.is_empty()) {
                (true, _) => Line::TooLong,
                (false, true) => Line::End,
                (false, false) => Line::Message(line),
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let taken = newline.map_or(available.len(), |position| position + 1);
        let content = available
            .get(..newline.unwrap_or(taken))
            .unwrap_or_default();
        if !too_long {
            if line.len() + content.len() > LONGEST_MESSAGE {
                too_long = true;
                line.clear();
            } else {
                line.extend_from_slice(content);
            }
        }
        input.consume(taken);
        if newline.is_some() {
            return Ok(if too_long {
                Line::TooLong
            } else {
                Line::Message(line)
            });
        }
    }
}

/// Serves until the input ends or the client stops reading.
pub fn serve(
    backend: &Backend,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<(), CliError> {
    let mut call = |tool: &str, arguments: &Value| {
        Request::from_tool(tool, arguments).and_then(|request| backend.run(request))
    };
    loop {
        let line = read_line(input).map_err(|error| {
            CliError::new(IO, format!("standard input could not be read: {error}"))
        })?;
        let answer = match line {
            Line::End => return Ok(()),
            Line::TooLong => Some(rpc_error(
                &Value::Null,
                INVALID_REQUEST,
                "The message is too long.",
            )),
            Line::Message(line) => {
                let line = line.trim_ascii();
                if line.is_empty() {
                    continue;
                }
                handle(line, &mut call)
            }
        };
        let Some(answer) = answer else { continue };
        let written = output
            .write_all(answer.to_string().as_bytes())
            .and_then(|()| output.write_all(b"\n"))
            .and_then(|()| output.flush());
        match written {
            Ok(()) => {}
            // The client has gone. That is how a session ends.
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => return Ok(()),
            Err(error) => return Err(CliError::from_write(&error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str, _: &Value) -> Result<Value, CliError> {
        Ok(json!({ "ok": true }))
    }

    fn answer(message: Value) -> Value {
        handle(message.to_string().as_bytes(), &mut none).unwrap()
    }

    fn modern_meta() -> Value {
        json!({
            VERSION_KEY: "2026-07-28",
            "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "0" },
            CAPABILITIES_KEY: {}
        })
    }

    #[test]
    fn a_handshake_gets_the_clients_version_when_it_is_known() {
        for version in LEGACY_VERSIONS {
            let answer = answer(json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": version, "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }
            }));
            assert_eq!(answer["result"]["protocolVersion"], version);
            assert_eq!(answer["result"]["serverInfo"]["name"], "wenmar-open");
            // Exactly the handshake's fields: nothing of the later revision.
            assert!(answer["result"].get("resultType").is_none());
        }
        let unknown = answer(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "1999-01-01" }
        }));
        assert_eq!(unknown["result"]["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn a_request_that_names_its_version_needs_no_handshake() {
        let discovered = answer(json!({
            "jsonrpc": "2.0", "id": "d", "method": "server/discover",
            "params": { "_meta": modern_meta() }
        }));
        let result = &discovered["result"];
        assert_eq!(result["resultType"], "complete");
        assert_eq!(
            result["supportedVersions"],
            json!([
                "2026-07-28",
                "2025-11-25",
                "2025-06-18",
                "2025-03-26",
                "2024-11-05"
            ])
        );
        assert_eq!(
            result["capabilities"],
            json!({ "tools": { "listChanged": false } })
        );
        assert_eq!(result["_meta"][SERVER_KEY]["name"], "wenmar-open");
        assert!(
            result["instructions"]
                .as_str()
                .unwrap()
                .contains("wenmar_vin")
        );

        let listed = answer(json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/list",
            "params": { "_meta": modern_meta() }
        }));
        assert_eq!(listed["result"]["resultType"], "complete");
        assert_eq!(listed["result"]["tools"], open_mcp::tools());

        let called = answer(json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "wenmar_vin", "arguments": { "action": "decode", "vin": "x" }, "_meta": modern_meta() }
        }));
        assert_eq!(called["result"]["resultType"], "complete");
        assert_eq!(called["result"]["isError"], false);
        assert_eq!(called["result"]["structuredContent"], json!({ "ok": true }));
    }

    #[test]
    fn a_version_this_server_does_not_speak_is_refused_with_the_ones_it_does() {
        for asked in [
            json!("2027-01-01"),
            json!("2025-11-25"),
            json!(7),
            json!(null),
        ] {
            let mut meta = modern_meta();
            meta[VERSION_KEY] = asked.clone();
            let answer = answer(json!({
                "jsonrpc": "2.0", "id": 9, "method": "tools/list", "params": { "_meta": meta }
            }));
            assert_eq!(answer["id"], 9);
            assert_eq!(answer["error"]["code"], -32022, "{asked}");
            assert_eq!(answer["error"]["data"]["requested"], asked);
            assert_eq!(answer["error"]["data"]["supported"][0], "2026-07-28");
        }
        // The capabilities are required beside the version.
        let answer = answer(json!({
            "jsonrpc": "2.0", "id": 9, "method": "tools/list",
            "params": { "_meta": { VERSION_KEY: "2026-07-28" } }
        }));
        assert_eq!(answer["error"]["code"], -32602);
    }

    #[test]
    fn a_probe_from_a_client_of_the_older_kind_is_an_error_it_can_fall_back_on() {
        let answer = answer(json!({ "jsonrpc": "2.0", "id": 1, "method": "server/discover" }));
        assert_eq!(answer["error"]["code"], -32602);
    }

    #[test]
    fn messages_that_are_not_valid_get_json_rpc_errors() {
        for (line, code) in [
            (&b"not json"[..], -32700),
            (&b"\xff\xfe{}"[..], -32700),
            (&b"[]"[..], -32600),
            (
                &br#"[{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]"#[..],
                -32600,
            ),
            (&b"42"[..], -32600),
            (&b"{}"[..], -32600),
            (&br#"{ "id": 1 }"#[..], -32600),
            (&br#"{ "id": 1, "result": {} }"#[..], -32600),
            (&br#"{ "id": {}, "method": "ping" }"#[..], -32600),
            (&br#"{ "id": [1], "method": "ping" }"#[..], -32600),
        ] {
            let answer = handle(line, &mut none).unwrap();
            assert_eq!(
                answer["error"]["code"],
                code,
                "{}",
                String::from_utf8_lossy(line)
            );
            assert_eq!(answer["id"], Value::Null);
        }
        let unknown = answer(json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }));
        assert_eq!(unknown["error"]["code"], -32601);
        let tool = answer(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "rm_rf" }
        }));
        assert_eq!(tool["error"]["code"], -32602);
        let long_name = answer(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "x".repeat(10_000) }
        }));
        assert!(long_name["error"]["message"].as_str().unwrap().len() < 100);
        let arguments = answer(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "wenmar_vin", "arguments": [1] }
        }));
        assert_eq!(arguments["error"]["code"], -32602);
    }

    #[test]
    fn a_notification_gets_no_answer() {
        for message in [
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 1 } }),
            json!({ "jsonrpc": "2.0", "id": null, "method": "ping" }),
        ] {
            assert_eq!(handle(message.to_string().as_bytes(), &mut none), None);
        }
    }

    #[test]
    fn a_tool_that_fails_answers_with_the_error_object() {
        let mut fail = |_: &str, _: &Value| -> Result<Value, CliError> {
            Err(CliError::new("not_found", "No vehicle has that id."))
        };
        let message = json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "wenmar_vehicles", "arguments": { "action": "entry", "id": "x" } }
        });
        let answer = handle(message.to_string().as_bytes(), &mut fail).unwrap();
        assert_eq!(answer["result"]["isError"], true);
        assert_eq!(
            answer["result"]["structuredContent"],
            json!({ "error": { "code": "not_found", "message": "No vehicle has that id.", "details": {} } })
        );
    }

    #[test]
    fn lines_are_read_whole_and_a_line_too_long_is_thrown_away() {
        let long = "x".repeat(LONGEST_MESSAGE + 1);
        let text = format!("first\r\n\n{long}\nlast");
        // A reader that hands out a few bytes at a time, as a pipe may.
        let mut input = std::io::BufReader::with_capacity(7, text.as_bytes());
        let mut lines = Vec::new();
        loop {
            match read_line(&mut input).unwrap() {
                Line::End => break,
                Line::Message(line) => lines.push(String::from_utf8(line).unwrap()),
                Line::TooLong => lines.push("TOO LONG".to_owned()),
            }
        }
        assert_eq!(lines, ["first\r", "", "TOO LONG", "last"]);
    }
}
