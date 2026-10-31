//! What the hosted `/mcp` endpoint and `wenmar-open mcp` have in common: the
//! gateway tools as an MCP client sees them, and the shapes of the
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
/// A required header is missing or does not say what the body says. Only
/// a server on HTTP answers with it.
pub const HEADER_MISMATCH: i64 = -32020;

/// Where a request of the revision with no handshake names its protocol
/// version and the client's capabilities, in `params._meta`.
pub const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
pub const CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
/// Where a result of that revision names the server, in its own `_meta`.
pub const SERVER_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// How long a client may keep the list of tools, in milliseconds. It never
/// changes while a server runs.
pub const TOOLS_TTL_MS: u64 = 3_600_000;

/// Which kind of protocol revision a request is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    /// Revisions up to 2025-11-25, which open with `initialize`.
    Handshake,
    /// Revision 2026-07-28: every request names its version.
    NoHandshake,
}

/// The name both servers give themselves.
pub const SERVER_NAME: &str = "wenmar-open";

/// What a model is told about the server as a whole.
pub const INSTRUCTIONS: &str = "Wenmar Open is free vehicle data for auto repair shops, from NHTSA's vPIC. Use wenmar_vin to decode a VIN. Use wenmar_vehicles to step through year, make, model, submodel and engine, to search by free text, or to look up a vehicle id. The overview action of either gives everything held for one vehicle in one answer. Use wenmar_recalls to list the recall campaigns from NHTSA and Transport Canada that may apply to a model year. A recall listed may apply: only the manufacturer can confirm it for one VIN. Use wenmar_bulletins to list the manufacturer service bulletins, warranty extensions and software updates that NHTSA publishes for a model year, or to search them by trouble code or by text. Use wenmar_codes to read what a generic OBD-II trouble code means and what bulletins and recalls a model year has for it. Use wenmar_labor for Wenmar's estimate of the hours a repair job takes on a vehicle: an estimate, not a quote. Use wenmar_payroll_deductions to work out the income tax, CPP, CPP2 and EI on one pay in Ontario by CRA's T4127 formulas: an estimate, and the employer remains liable. Every call is read-only and needs no key.";

/// The names of the tools, in the order `tools/list` gives them.
pub const TOOL_NAMES: [&str; 7] = [
    "wenmar_vin",
    "wenmar_vehicles",
    "wenmar_recalls",
    "wenmar_bulletins",
    "wenmar_codes",
    "wenmar_labor",
    "wenmar_payroll_deductions",
];

/// The Server Card schema a card must conform to. The schema's own pattern
/// pins the `/v1/` family, so a breaking change to the shape arrives as a
/// new one.
pub const CARD_SCHEMA: &str =
    "https://static.modelcontextprotocol.io/schemas/v1/server-card.schema.json";

/// The media type of a Server Card, and of the AI Catalog that points at one.
pub const CARD_TYPE: &str = "application/mcp-server-card+json";
pub const CATALOG_TYPE: &str = "application/ai-catalog+json";

/// One line, at most a hundred characters, on what this server is for. The
/// schema sets that limit, and `registry server.json` holds the same words.
pub const CARD_DESCRIPTION: &str =
    "Free VIN decoder, recalls, bulletins, trouble codes, labour times and payroll. No key.";

/// The Server Card (SEP-2127): what this server is and where to reach it,
/// for a client that has a domain and no connection yet.
///
/// Every value comes from the same place the running server reads it, so the
/// card cannot claim a name, a version or a protocol revision that
/// `server/discover` would contradict. It carries no tools, resources,
/// prompts or capabilities: the SEP excludes primitives on purpose, because a
/// static document cannot say what a client would be allowed to reach.
pub fn server_card(base: &str, legacy: &[&'static str], server_version: &str) -> Value {
    json!({
        "$schema": CARD_SCHEMA,
        "name": "com.wenmarpro/wenmar-open",
        "title": "Wenmar Open",
        "description": CARD_DESCRIPTION,
        "version": server_version,
        "websiteUrl": base,
        "repository": {
            "url": "https://github.com/wenmar-pro/wenmar-open",
            "source": "github"
        },
        "remotes": [{
            "type": "streamable-http",
            "url": format!("{base}/mcp"),
            "supportedProtocolVersions": supported(legacy)
        }]
    })
}

/// The AI Catalog (SEP-2127): the entry a client that has only a domain
/// reads to learn that this domain serves an MCP server at all. It carries
/// no fields of its own beyond the card's address, so the card cannot drift
/// from what it says.
pub fn ai_catalog(base: &str) -> Value {
    json!({
        "specVersion": "1.0",
        "entries": [{
            "identifier": "urn:air:open.wenmarpro.com:mcp:wenmar-open",
            "type": CARD_TYPE,
            "url": format!("{base}/mcp/server-card")
        }]
    })
}

/// The tools, as `tools/list` returns them.
pub fn tools() -> Value {
    json!([
        {
            "name": "wenmar_vin",
            "title": "VIN decoder",
            "description": "Decode vehicle identification numbers into year, make, model, trim, generation, engine (with horsepower, valve train and fuel injection), drivetrain, brakes, battery and charging, safety equipment, lights and keyless ignition, the manufacturer's notes, every NHTSA attribute with its source, and the matching catalog entry. year_conclusive=false means the VIN leaves the model year open: check the vehicle's label. action=decode takes `vin` (and optionally `year` to override the model year). action=batch takes `vins`, at most 50, and returns one result per VIN in order. A wrong check digit is not an error: the result has valid=false and a warning. action=overview takes `vin` (and optionally `year`) and gives everything held for the model year the VIN decodes to in one answer: the decode in brief, a count of the recall campaigns that may apply to that year, make and model with the three newest, and links. A block with no data is null. action=fuel, specs, fluids or tires takes `vin` (and optionally `year`) and gives that data for the model year the VIN decodes to: fuel consumption ratings from EPA and Natural Resources Canada (optional `country`, us or ca), specifications from EPA's files, fluids (the engine oil grade EPA records, and records a person read from the maker's own documents, each with its source and the day it was checked), or the original-equipment tire fitments. When the decode settles the engine, fuel, specs and fluids hold only what is for that engine and `engine_settled` is true; tires are never narrowed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["decode", "batch", "overview", "fuel", "specs", "fluids", "tires"] },
                    "vin": { "type": "string", "description": "A 17-character VIN. Spaces and dashes are ignored." },
                    "vins": { "type": "array", "items": { "type": "string" }, "maxItems": 50 },
                    "year": { "type": "integer", "description": "Model year to use instead of the one worked out from the VIN." },
                    "country": { "type": "string", "description": "For action=fuel: us for EPA's ratings, ca for Natural Resources Canada's. Leave out for both." }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_vehicles",
            "title": "Vehicle catalog",
            "description": "Look up vehicles by year, make, model, submodel and engine. Actions: years; makes (optional `year`); models (`make`, optional `year`); submodels (`make`, `model`, `year`); engines (`make`, `model`, `year`, optional `submodel`); search (`query`, such as \"2019 civic si\" or \"chevy 1500\"); entry (`id`, such as \"2019_honda_civic_si\"); overview (`id`: everything held for that model year in one answer, with a count of the recall campaigns that may apply and the three newest; a block with no data is null); model_years (`make`, `model`: every model year of one model, newest first, each with its vehicle id and the number of recall campaigns that may apply); safety (`id`: NHTSA's crash test ratings for that model year, one entry for each vehicle NHTSA rated, by body style and drive, each with its stars; an entry with no stars is a vehicle NHTSA lists and has not tested); fuel (`id`, optional `country` us or ca: fuel consumption ratings from EPA and Natural Resources Canada, each saying in `confidence` how far it was matched: model, engine or submodel); specs (`id`: transmission, drive, recommended fuel and more from EPA's files, one entry per EPA rating); fluids (`id`: the engine oil grade EPA records for each engine, and records a person read from the maker's own documents, each with its `source` and the day it was `checked`; an empty list means nobody has recorded it); tires (`id`: every original-equipment tire size on record with the trims it covers; none is picked). `make` and `model` take a name, an alias or an id. `term` narrows a list to names starting with it. `scope` is light (cars, MPVs and trucks; the default), all, or a vPIC vehicle type id. action=complaints takes `id` and gives what owners reported to NHTSA about that model year as counts by component, with the median mileage owners gave: these are owners' reports, not verified, and a count is not a rate.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["years", "makes", "models", "submodels", "engines", "search", "entry", "overview", "model_years", "safety", "complaints", "fuel", "specs", "fluids", "tires"] },
                    "year": { "type": "integer" },
                    "make": { "type": "string" },
                    "model": { "type": "string" },
                    "submodel": { "type": "string" },
                    "term": { "type": "string" },
                    "query": { "type": "string" },
                    "id": { "type": "string" },
                    "country": { "type": "string", "description": "For action=fuel: us or ca. Leave out for both." },
                    "scope": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_recalls",
            "title": "Recalls",
            "description": "Recall campaigns from NHTSA (United States) and Transport Canada that may apply to a vehicle. A campaign lists a year, a make and a model, so every answer has may_apply: true. It may apply; confirm by VIN with the manufacturer, or at the addresses in `lookup`. This is never a lookup by VIN. action=vehicle takes `id`, a vehicle id such as \"2018_ford_f-150\". action=vin takes `vin` (and optionally `year`): it decodes the VIN and lists the recalls of its model year. action=campaign takes `source` (nhtsa or tc) and `campaign`, such as \"19V888000\", and gives that campaign with every vehicle it lists. action=search lists campaigns newest first, narrowed by any of `make`, `model` (needs `make`), `year`, `component` and `query` (free text); `limit` is at most 100 and 25 when left out; `cursor` is `next_cursor` of the page before. action=recent takes `since`, a day such as \"2026-09-01\", and lists the campaigns reported on or after it, newest first, each with the vehicles it lists; it also takes `make`, `limit` and `cursor`. `country` is us or ca on every action but campaign; left out, both.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["vehicle", "vin", "campaign", "search", "recent"] },
                    "id": { "type": "string", "description": "A vehicle id, such as 2018_ford_f-150." },
                    "vin": { "type": "string", "description": "A 17-character VIN. Spaces and dashes are ignored." },
                    "year": { "type": "integer", "description": "For vin: the model year to use. For search: only campaigns that list this model year." },
                    "source": { "type": "string", "enum": ["nhtsa", "tc"] },
                    "campaign": { "type": "string", "description": "The publisher's campaign number." },
                    "make": { "type": "string" },
                    "model": { "type": "string" },
                    "component": { "type": "string" },
                    "query": { "type": "string" },
                    "country": { "type": "string", "enum": ["us", "ca"] },
                    "limit": { "type": "integer" },
                    "cursor": { "type": "string" },
                    "since": { "type": "string", "description": "For recent: a day, such as 2026-09-01. Campaigns reported on or after it are listed." }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_bulletins",
            "title": "Service bulletins",
            "description": "Manufacturer communications NHTSA publishes: service bulletins, service campaigns, warranty extensions and over-the-air updates. Each has NHTSA's summary, the manufacturer's own document number, and `document`, the address NHTSA's copy of the document is expected at (not confirmed; null for old ones). The document itself is the manufacturer's and is not returned. A bulletin lists model years: it is not a recall and does not say a vehicle is affected. action=vehicle takes `id`, a vehicle id such as \"2018_ford_f-150\", and gives one page of that model year's bulletins, newest first, with `counts` by type, component, trouble code and topic. action=vin takes `vin` (and optionally `year`) and does the same for the model year the VIN decodes to. action=get takes `nhtsa_id` and gives one bulletin with every vehicle it lists. action=search lists all bulletins newest first, narrowed by `make`, `model` (needs `make`) and `year`. Every action but get takes `type` (service_bulletin, service_campaign, warranty_extension, over_the_air, emissions, other), `component` (an id from `counts.components`, such as brakes), `code` (a trouble code such as P0420), `topic` (maintenance), `query` (free text in the summary, such as a symptom), `limit` (at most 100, 25 when left out) and `cursor` (`next_cursor` of the page before). This tool needs the hosted service: a local data file does not hold bulletins.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["vehicle", "vin", "get", "search"] },
                    "id": { "type": "string", "description": "A vehicle id, such as 2018_ford_f-150." },
                    "vin": { "type": "string", "description": "A 17-character VIN. Spaces and dashes are ignored." },
                    "year": { "type": "integer", "description": "For vin: the model year to use. For search: only bulletins that list this model year." },
                    "nhtsa_id": { "type": "integer", "description": "NHTSA's id number for the communication." },
                    "make": { "type": "string" },
                    "model": { "type": "string" },
                    "type": { "type": "string", "enum": ["service_bulletin", "service_campaign", "warranty_extension", "over_the_air", "emissions", "other"] },
                    "component": { "type": "string" },
                    "code": { "type": "string" },
                    "topic": { "type": "string", "enum": ["maintenance"] },
                    "query": { "type": "string" },
                    "limit": { "type": "integer" },
                    "cursor": { "type": "string" }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_codes",
            "title": "Trouble codes",
            "description": "Generic OBD-II trouble codes, described in Wenmar Open's own words: a title, what the code means, common causes, first checks, the components it concerns and related codes. Only codes every maker uses the same way are described (P0, P2, P3400 and up, B0, C0, U0, U3). Manufacturer-specific codes are not described. action=get takes `code`, such as \"P0420\", and gives its description. action=search lists the described codes by code, narrowed by `query` (text in the code, title or meaning) and `system` (powertrain, body, chassis, network, or an area such as powertrain/emissions); pass `next_cursor` back as `cursor`. action=vehicle takes `id`, a vehicle id such as \"2018_ford_f-150\", and `code`, any code, a manufacturer-specific one included: it gives the description (null when there is none), the bulletins of that model year whose NHTSA summary names the code, and the bulletins and recalls of that model year about the same components. A part of that answer is null where the data is not held. A description is general and is not a diagnosis; a bulletin or recall listed may apply to the model year and is not confirmed for one vehicle.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["get", "search", "vehicle"] },
                    "code": { "type": "string", "description": "A trouble code, such as P0420. Either case." },
                    "id": { "type": "string", "description": "A vehicle id, such as 2018_ford_f-150." },
                    "query": { "type": "string" },
                    "system": { "type": "string" },
                    "limit": { "type": "integer" },
                    "cursor": { "type": "string" }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_labor",
            "title": "Labour estimates",
            "description": "Wenmar's own estimates of the hours a repair job takes: written by people who do the work, measured from repair orders, or worked out by AI from the repair steps; never copied from a labour guide. An estimate, not a quote. Rust, seized fasteners and added equipment change the time. action=jobs lists the jobs an estimate can be about; with `job` it gives one job: what it includes, what it excludes, and its typical hours by class of vehicle. action=vehicle takes `id`, a vehicle id such as \"2018_ford_f-150\" (an id that ends with an engine asks for that engine), and optionally `job`: for each job it gives `hours` (low, typical, high), `basis` (expert, observed or ai: an AI estimate worked out from the repair steps, not checked by a person; say so, with its confidence_pct) and `scope`, which says what the figure is for: engine, generation, model or class. A class figure is typical for vehicles of that class and was not worked out on this vehicle: say so. A job with no figure for the vehicle is not listed. action=vin takes `vin` (and optionally `year`, `job`) and does the same for the vehicle the VIN decodes to. No answer holds a price: multiply by the shop's own rate. Credit the data as `attribution` says: Labour estimates by Wenmar Pro, CC BY 4.0.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["jobs", "vehicle", "vin"] },
                    "job": { "type": "string", "description": "A job id, such as brake-pads-front." },
                    "id": { "type": "string", "description": "A vehicle id, such as 2018_ford_f-150." },
                    "vin": { "type": "string", "description": "A 17-character VIN. Spaces and dashes are ignored." },
                    "year": { "type": "integer", "description": "For vin: the model year to use." }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_payroll_deductions",
            "title": "Ontario payroll deductions",
            "description": "Works out the deductions from one regular pay in Ontario by CRA's T4127 formulas (Option 1): federal and Ontario income tax, CPP, CPP2 and EI, the net pay, the employer's CPP, CPP2 and EI, and every T4127 factor used with what it means. Takes `frequency` (weekly, biweekly, semi-monthly or monthly) and `gross`, the gross pay for the period. `federal_claim` and `provincial_claim` are the TD1 claim amounts: left out, each is the basic personal amount (claim code 1), and 0 is claim code 0. `pay_date` (YYYY-MM-DD) chooses the tables; left out, it is today, and a day outside the tables held is an error that names their dates. `ytd_cpp`, `ytd_cpp2`, `ytd_ei` and `ytd_pensionable` are what was deducted and earned on earlier pays this calendar year. Money is a number or text such as \"2400.00\". Ontario only; not for a bonus, commission or retroactive pay. The answer is an estimate and the employer remains liable to CRA for what it deducts and remits: say so, and point to CRA's Payroll Deductions Online Calculator at `pdoc_url`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "frequency": { "type": "string", "enum": ["weekly", "biweekly", "semi-monthly", "monthly"] },
                    "gross": { "type": ["number", "string"], "description": "Gross pay for one pay period, in dollars." },
                    "federal_claim": { "type": ["number", "string"], "description": "Federal TD1 claim amount. Left out: claim code 1." },
                    "provincial_claim": { "type": ["number", "string"], "description": "Ontario TD1ON claim amount. Left out: claim code 1." },
                    "pay_date": { "type": "string", "description": "YYYY-MM-DD. Left out: today." },
                    "ytd_cpp": { "type": ["number", "string"], "description": "CPP (base and first additional) deducted earlier this year." },
                    "ytd_cpp2": { "type": ["number", "string"], "description": "CPP2 deducted earlier this year." },
                    "ytd_ei": { "type": ["number", "string"], "description": "EI deducted earlier this year." },
                    "ytd_pensionable": { "type": ["number", "string"], "description": "Pensionable earnings earlier this year." },
                    "province": { "type": "string", "enum": ["ON"], "description": "Ontario only for now. Left out: ON." }
                },
                "required": ["frequency", "gross"]
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

/// Every version a server speaks, newest first: the revision with no
/// handshake, then `legacy`, the handshake revisions it speaks.
pub fn supported(legacy: &[&'static str]) -> Vec<&'static str> {
    MODERN_VERSIONS
        .iter()
        .chain(legacy.iter())
        .copied()
        .collect()
}

/// A JSON-RPC answer that carries `error`, a whole error object. Use it
/// for an error that has `data`; [`rpc_error`] is for one that has none.
pub fn rpc_failure(id: &Value, error: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}

/// Which kind of revision a request is of, read from its `params`.
///
/// A request that names a protocol version in `_meta` is of the kind with
/// no handshake. It must name a version this crate knows, and carry the
/// client's capabilities beside it. `Err` is the JSON-RPC error object to
/// answer with.
pub fn era(params: &Value, legacy: &[&'static str]) -> Result<Era, Value> {
    let meta = params.get("_meta").unwrap_or(&Value::Null);
    let Some(asked) = meta.get(VERSION_KEY) else {
        return Ok(Era::Handshake);
    };
    if !asked
        .as_str()
        .is_some_and(|asked| MODERN_VERSIONS.contains(&asked))
    {
        return Err(json!({
            "code": UNSUPPORTED_PROTOCOL_VERSION,
            "message": "Unsupported protocol version",
            "data": { "supported": supported(legacy), "requested": asked }
        }));
    }
    if !meta.get(CAPABILITIES_KEY).is_some_and(Value::is_object) {
        return Err(json!({
            "code": INVALID_PARAMS,
            "message": format!("_meta must carry {CAPABILITIES_KEY}.")
        }));
    }
    Ok(Era::NoHandshake)
}

/// The answer to `server/discover`: what the server speaks and offers. The
/// server's name is not here; it goes in the result's `_meta`, which
/// [`complete`] adds.
pub fn discover(legacy: &[&'static str]) -> Value {
    json!({
        "supportedVersions": supported(legacy),
        "capabilities": { "tools": { "listChanged": false } },
        "instructions": INSTRUCTIONS,
        "ttlMs": TOOLS_TTL_MS,
        "cacheScope": "public"
    })
}

/// The answer to `tools/list`. The revision with no handshake also says
/// how long the list may be kept.
pub fn tools_list(era: Era) -> Value {
    match era {
        Era::Handshake => json!({ "tools": tools() }),
        Era::NoHandshake => json!({
            "tools": tools(),
            "ttlMs": TOOLS_TTL_MS,
            "cacheScope": "public"
        }),
    }
}

/// A result as the revision with no handshake requires it: marked
/// complete, and naming the server in `_meta`.
pub fn complete(mut result: Value, server_version: &str) -> Value {
    if let Some(object) = result.as_object_mut() {
        object.insert("resultType".to_owned(), json!("complete"));
        object.insert(
            "_meta".to_owned(),
            json!({ SERVER_KEY: server_info(server_version) }),
        );
    }
    result
}

/// The name an `Mcp-Name` header carries. A name with characters a header
/// cannot hold is sent as `=?base64?...?=`; anything else is the name
/// itself. `None` when what claims to be base64 is not, or is not text.
pub fn header_name(value: &str) -> Option<String> {
    let Some(encoded) = value
        .strip_prefix("=?base64?")
        .and_then(|rest| rest.strip_suffix("?="))
    else {
        return Some(value.to_owned());
    };
    let mut bytes = Vec::with_capacity(encoded.len());
    let (mut held, mut bits) = (0u32, 0u32);
    for character in encoded.trim_end_matches('=').bytes() {
        let six = match character {
            b'A'..=b'Z' => character - b'A',
            b'a'..=b'z' => character - b'a' + 26,
            b'0'..=b'9' => character - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        held = (held << 6) | u32::from(six);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(u8::try_from(held >> bits).ok()?);
            held &= (1 << bits) - 1;
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_tool_describes_and_never_diagnoses() {
        let tools = tools();
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wenmar_codes")
            .unwrap();
        let description = tool["description"].as_str().unwrap();
        assert!(description.contains("own words"), "{description}");
        assert!(
            description.contains("Manufacturer-specific codes are not described"),
            "{description}"
        );
        assert!(description.contains("not a diagnosis"), "{description}");
        assert_eq!(
            tool["inputSchema"]["properties"]["action"]["enum"],
            json!(["get", "search", "vehicle"])
        );
        assert!(INSTRUCTIONS.contains("wenmar_codes"));
    }

    #[test]
    fn the_labor_tool_says_an_estimate_is_not_a_quote() {
        let tools = tools();
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wenmar_labor")
            .unwrap();
        let description = tool["description"].as_str().unwrap();
        assert!(
            description.contains("Labour estimates by Wenmar Pro, CC BY 4.0"),
            "{description}"
        );
        assert!(description.contains("`scope`"), "{description}");
        assert!(
            description.contains("never copied from a labour guide"),
            "{description}"
        );
        assert!(
            description.contains("expert, observed or ai"),
            "{description}"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["action"]["enum"],
            json!(["jobs", "vehicle", "vin"])
        );
        assert!(INSTRUCTIONS.contains("wenmar_labor"));
    }

    #[test]
    fn the_payroll_tool_says_estimate_and_liable_and_needs_no_action() {
        let tools = tools();
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wenmar_payroll_deductions")
            .unwrap();
        let description = tool["description"].as_str().unwrap();
        for words in [
            "estimate",
            "the employer remains liable",
            "pdoc_url",
            "claim code 1",
            "Ontario only",
        ] {
            assert!(description.contains(words), "{words}: {description}");
        }
        assert_eq!(
            tool["inputSchema"]["required"],
            json!(["frequency", "gross"])
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["frequency"]["enum"],
            json!(["weekly", "biweekly", "semi-monthly", "monthly"])
        );
        assert!(tool["inputSchema"]["properties"].get("action").is_none());
        assert_eq!(
            tool["annotations"],
            json!({ "readOnlyHint": true, "openWorldHint": false })
        );
        assert!(INSTRUCTIONS.contains("wenmar_payroll_deductions"));
    }

    #[test]
    fn there_are_seven_tools_and_each_gateway_tool_needs_an_action() {
        let tools = tools();
        let tools = tools.as_array().unwrap();
        let names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, TOOL_NAMES);
        for tool in tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            if tool["name"] != "wenmar_payroll_deductions" {
                assert_eq!(tool["inputSchema"]["required"], json!(["action"]));
            }
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
        }
    }

    #[test]
    fn the_recalls_tool_says_may_apply_and_never_that_a_recall_applies() {
        let tools = tools();
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wenmar_recalls")
            .unwrap();
        let description = tool["description"].as_str().unwrap();
        assert!(description.contains("may apply"), "{description}");
        assert!(
            description.contains("confirm by VIN with the manufacturer"),
            "{description}"
        );
        assert!(
            description.contains("never a lookup by VIN"),
            "{description}"
        );
        for claim in ["applies to", "is affected", "open recall", "your vehicle"] {
            assert!(!description.contains(claim), "{claim}");
        }
        assert_eq!(
            tool["inputSchema"]["properties"]["action"]["enum"],
            json!(["vehicle", "vin", "campaign", "search", "recent"])
        );
        assert!(INSTRUCTIONS.contains("wenmar_recalls"));
        assert!(INSTRUCTIONS.contains("may apply"));
    }

    #[test]
    fn the_bulletins_tool_says_what_a_bulletin_is_not() {
        let tools = tools();
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wenmar_bulletins")
            .unwrap();
        let description = tool["description"].as_str().unwrap();
        assert!(description.contains("it is not a recall"), "{description}");
        assert!(description.contains("not confirmed"), "{description}");
        for claim in ["applies to", "is affected by"] {
            assert!(!description.contains(claim), "{claim}");
        }
        assert_eq!(
            tool["inputSchema"]["properties"]["action"]["enum"],
            json!(["vehicle", "vin", "get", "search"])
        );
        assert!(INSTRUCTIONS.contains("wenmar_bulletins"));
    }

    #[test]
    fn the_overview_actions_are_on_the_three_tools_and_no_tool_was_added() {
        let tools = tools();
        let tool = |name: &str| {
            tools
                .as_array()
                .unwrap()
                .iter()
                .find(|tool| tool["name"] == name)
                .unwrap()
                .clone()
        };
        let actions = |name: &str| {
            tool(name)["inputSchema"]["properties"]["action"]["enum"]
                .as_array()
                .unwrap()
                .clone()
        };
        assert_eq!(TOOL_NAMES.len(), 7);
        assert!(actions("wenmar_vin").contains(&json!("overview")));
        for action in ["entry", "overview", "model_years"] {
            assert!(
                actions("wenmar_vehicles").contains(&json!(action)),
                "{action}"
            );
        }
        assert_eq!(
            tool("wenmar_recalls")["inputSchema"]["properties"]["since"]["type"],
            "string"
        );
        // An overview counts the recalls that may apply. It says no more.
        for name in ["wenmar_vin", "wenmar_vehicles"] {
            let described = tool(name);
            let description = described["description"].as_str().unwrap();
            assert!(description.contains("overview"), "{name}");
            assert!(description.contains("may apply"), "{name}");
            for claim in ["applies to", "is affected", "open recall", "your vehicle"] {
                assert!(!description.contains(claim), "{name}: {claim}");
            }
        }
    }

    #[test]
    fn the_vehicles_tool_has_a_safety_action_for_nhtsas_ratings() {
        let tools = tools();
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wenmar_vehicles")
            .unwrap();
        let actions = tool["inputSchema"]["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert!(actions.contains(&json!("safety")), "{actions:?}");
        assert!(actions.contains(&json!("entry")), "{actions:?}");
        let description = tool["description"].as_str().unwrap();
        assert!(description.contains("safety (`id`"), "{description}");
        assert!(description.contains("NHTSA"), "{description}");
        // An action of a tool that exists: no tool was added.
        assert_eq!(TOOL_NAMES.len(), 7);
    }

    #[test]
    fn fuel_specs_fluids_and_tires_are_actions_of_the_two_tools_that_exist() {
        let tools = tools();
        for name in ["wenmar_vin", "wenmar_vehicles"] {
            let tool = tools
                .as_array()
                .unwrap()
                .iter()
                .find(|tool| tool["name"] == name)
                .unwrap();
            let actions = tool["inputSchema"]["properties"]["action"]["enum"]
                .as_array()
                .unwrap();
            for action in ["fuel", "specs", "fluids", "tires"] {
                assert!(actions.contains(&json!(action)), "{name}: {actions:?}");
            }
            assert!(
                tool["inputSchema"]["properties"]["country"].is_object(),
                "{name}"
            );
            let description = tool["description"].as_str().unwrap();
            for said in ["EPA", "Natural Resources Canada", "the day it was"] {
                assert!(description.contains(said), "{name}: {said}");
            }
        }
        // Actions of tools that exist: no tool was added.
        assert_eq!(TOOL_NAMES.len(), 7);
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

    fn modern_params() -> Value {
        json!({ "_meta": { VERSION_KEY: "2026-07-28", CAPABILITIES_KEY: {} } })
    }

    #[test]
    fn a_request_says_which_kind_of_revision_it_is_of() {
        let legacy = ["2025-11-25", "2025-06-18"];
        for params in [json!(null), json!({}), json!({ "_meta": {} }), json!([1])] {
            assert_eq!(era(&params, &legacy), Ok(Era::Handshake), "{params}");
        }
        assert_eq!(era(&modern_params(), &legacy), Ok(Era::NoHandshake));
        // A version this server does not speak, with the ones it does.
        for asked in [
            json!("2027-01-01"),
            json!("2025-11-25"),
            json!(7),
            json!(null),
        ] {
            let error =
                era(&json!({ "_meta": { VERSION_KEY: asked.clone() } }), &legacy).unwrap_err();
            assert_eq!(error["code"], -32022, "{asked}");
            assert_eq!(error["data"]["requested"], asked);
            assert_eq!(
                error["data"]["supported"],
                json!(["2026-07-28", "2025-11-25", "2025-06-18"])
            );
        }
        // The capabilities are required beside the version.
        let error = era(&json!({ "_meta": { VERSION_KEY: "2026-07-28" } }), &legacy).unwrap_err();
        assert_eq!(error["code"], -32602);
    }

    #[test]
    fn discovery_and_results_have_the_shape_of_the_revision_with_no_handshake() {
        let found = discover(&["2025-11-25"]);
        assert_eq!(
            found["supportedVersions"],
            json!(["2026-07-28", "2025-11-25"])
        );
        assert_eq!(
            found["capabilities"],
            json!({ "tools": { "listChanged": false } })
        );
        assert_eq!(found["cacheScope"], "public");
        assert_eq!(found["ttlMs"], 3_600_000);
        assert!(found.get("serverInfo").is_none(), "it goes in _meta");

        assert!(tools_list(Era::Handshake).get("ttlMs").is_none());
        assert_eq!(tools_list(Era::NoHandshake)["cacheScope"], "public");
        assert_eq!(tools_list(Era::NoHandshake)["tools"], tools());

        let result = complete(json!({ "tools": [] }), "0.1.0");
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["_meta"][SERVER_KEY]["name"], "wenmar-open");
        assert_eq!(result["_meta"][SERVER_KEY]["version"], "0.1.0");
        assert_eq!(result["tools"], json!([]));
        let failure = rpc_failure(&json!(3), json!({ "code": -32020, "message": "m" }));
        assert_eq!(
            failure,
            json!({ "jsonrpc": "2.0", "id": 3, "error": { "code": -32020, "message": "m" } })
        );
    }

    #[test]
    fn a_name_in_a_header_is_read_plain_or_from_base64() {
        assert_eq!(header_name("wenmar_vin").as_deref(), Some("wenmar_vin"));
        assert_eq!(
            header_name("=?base64?d2VubWFyX3Zpbg==?=").as_deref(),
            Some("wenmar_vin")
        );
        assert_eq!(
            header_name("=?base64?d2VubWFyX3ZlaGljbGVz?=").as_deref(),
            Some("wenmar_vehicles")
        );
        assert_eq!(header_name("=?base64??=").as_deref(), Some(""));
        // Not base64, and base64 of bytes that are not text.
        assert_eq!(header_name("=?base64?!!!?="), None);
        assert_eq!(header_name("=?base64?/w==?="), None);
        // Something that only looks like the start of it is a plain name.
        assert_eq!(header_name("=?base64?abc").as_deref(), Some("=?base64?abc"));
    }
}
