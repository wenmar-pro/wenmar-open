//! Answers from the hosted API, over HTTP.

use std::time::Duration;

use serde_json::{Value, json};
use wenmar_vehicles::VehicleId;

use crate::error::{BAD_RESPONSE, CliError, NETWORK, NOT_FOUND, USAGE};
use crate::request::{
    Action, BulletinAction, BulletinLookup, Lookup, RecallAction, RecallLookup, Request, Topic,
    current_year,
};

use crate::request::{CodeAction, CodeLookup};

/// The longest answer read from the API, in bytes.
const LONGEST_ANSWER: u64 = 4 * 1024 * 1024;
/// How long a request may take in all.
const TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The hosted API at one base URL.
pub struct Remote {
    base: String,
    agent: ureq::Agent,
}

/// What every request this tool makes is marked with. Nothing else about
/// the computer or the person is sent.
pub fn user_agent() -> String {
    format!("wenmar-open/{}", env!("CARGO_PKG_VERSION"))
}

/// An HTTP agent that reports a status of 400 or more as an answer, not as
/// a failure, so the body can be read.
pub fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(timeout)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .user_agent(user_agent())
        .build()
        .into()
}

/// An agent for a large download. The transfer as a whole has no limit,
/// but the answer must begin within `response` and the body arrive within
/// `body`, so a server that stops sending does not keep the tool waiting
/// for ever.
pub fn download_agent(response: Duration, body: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(response))
        .timeout_recv_body(Some(body))
        .user_agent(user_agent())
        .build()
        .into()
}

/// Whether `url` is an address this tool will make a request to.
pub fn is_http(url: &str) -> bool {
    ["http://", "https://"].iter().any(|scheme| {
        url.strip_prefix(scheme)
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
    })
}

/// Text as one part of a path. Everything but letters, digits, `-`, `_`
/// and `~` is percent-encoded, so that a name with a space, a slash or a
/// dot in it stays one part and names nothing else.
fn segment(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

impl Remote {
    /// `base` is the API's address without `/v1`, such as
    /// `https://open.wenmarpro.com`.
    pub fn new(base: &str) -> Result<Remote, CliError> {
        let base = base.trim().trim_end_matches('/');
        if !is_http(base) || base.chars().any(char::is_whitespace) {
            return Err(CliError::new(
                USAGE,
                "the API address must start with http:// or https://",
            )
            .with_hint("Check --api and WENMAR_OPEN_API."));
        }
        Ok(Remote {
            base: base.to_owned(),
            agent: agent(Some(TIMEOUT)),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn unreachable(&self, cause: &ureq::Error) -> CliError {
        CliError::new(
            NETWORK,
            format!("The API at {} could not be reached: {cause}.", self.base),
        )
        .with_hint("Check the connection, or run `wenmar-open data pull` once to work without one.")
    }

    fn not_the_api(&self, status: u16) -> CliError {
        CliError::new(
            BAD_RESPONSE,
            format!(
                "The answer from {} was not one the API gives (HTTP {status}).",
                self.base
            ),
        )
        .with_hint("Check that --api is the address of a Wenmar Open API.")
    }

    /// Reads an answer: JSON for a status under 300, and otherwise the
    /// API's error object. Anything else did not come from the API.
    fn read(
        &self,
        answer: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Value, CliError> {
        let mut response = answer.map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(LONGEST_ANSWER)
            .read_to_string()
            .map_err(|error| match error {
                ureq::Error::BodyExceedsLimit(_) => self.not_the_api(status),
                other => self.unreachable(&other),
            })?;
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Err(self.not_the_api(status));
        };
        if (200..300).contains(&status) {
            return Ok(value);
        }
        Err(CliError::from_body(&value).unwrap_or_else(|| self.not_the_api(status)))
    }

    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, CliError> {
        let url = format!("{}{path}", self.base);
        let mut request = self.agent.get(&url);
        for (name, value) in query {
            request = request.query(*name, value);
        }
        self.read(request.call())
    }

    /// `GET /v1/meta`: the data version the API is serving.
    pub fn meta(&self) -> Result<Value, CliError> {
        let meta = self.get("/v1/meta", &[])?;
        if meta.get("data_version").is_some_and(Value::is_string) {
            Ok(meta)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// Answers a request that has been through [`Request::checked`].
    pub fn run(&self, request: &Request) -> Result<Value, CliError> {
        match request {
            Request::VinDecode { vin, year } => self.decode(vin, *year),
            Request::VinBatch { vins, year } => self.batch(vins, *year),
            Request::VinOverview { vin, year } => {
                let query: Vec<(&str, String)> =
                    year.iter().map(|year| ("year", year.to_string())).collect();
                // `vin` is well formed, so it is safe in a path.
                self.overview(&format!("/v1/vin/{vin}/overview"), &query)
            }
            Request::VehicleFacts { topic, id, country } => {
                // Only text that has the form of an id is put in a path.
                if VehicleId::parse(id).is_none() {
                    return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
                }
                let query: Vec<(&str, String)> = country
                    .iter()
                    .map(|country| ("country", country.clone()))
                    .collect();
                self.facts(
                    *topic,
                    &format!("/v1/vehicles/{id}/{}", topic.name()),
                    &query,
                )
            }
            Request::VinFacts {
                topic,
                vin,
                year,
                country,
            } => {
                let mut query: Vec<(&str, String)> =
                    year.iter().map(|year| ("year", year.to_string())).collect();
                query.extend(country.iter().map(|country| ("country", country.clone())));
                // `vin` is well formed, so it is safe in a path.
                self.facts(*topic, &format!("/v1/vin/{vin}/{}", topic.name()), &query)
            }
            Request::Vehicles { action, lookup } => self.vehicles(*action, lookup),
            Request::Recalls { action, lookup } => self.recalls(*action, lookup),
            Request::Codes { action, lookup } => self.codes(*action, lookup),
            Request::Bulletins { action, lookup } => self.bulletins(*action, lookup),
        }
    }

    /// Fuel, specifications, fluids or tire fitments. Every answer has the
    /// topic's list, empty or not: one without it did not come from the API.
    fn facts(&self, topic: Topic, path: &str, query: &[(&str, String)]) -> Result<Value, CliError> {
        let answer = self.get(path, query)?;
        if answer.get(topic.list()).is_some_and(Value::is_array) {
            Ok(answer)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// An overview, of a model year or of a VIN. Every one has `links`: an
    /// answer without it did not come from the API.
    fn overview(&self, path: &str, query: &[(&str, String)]) -> Result<Value, CliError> {
        let answer = self.get(path, query)?;
        if answer.get("links").is_some_and(Value::is_object) {
            Ok(answer)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// The code routes. Only text with the form of a code or of a vehicle
    /// id is put in a path. Every answer has the field named last: one
    /// without it did not come from the API.
    fn codes(&self, action: CodeAction, lookup: &CodeLookup) -> Result<Value, CliError> {
        use wenmar_shopdata::bulletins::code_form;
        use wenmar_shopdata::codes::NOT_A_CODE;

        let code = || {
            code_form(lookup.code.as_deref().unwrap_or_default())
                .ok_or_else(|| CliError::new(NOT_FOUND, NOT_A_CODE))
        };
        let mut query: Vec<(&str, String)> = Vec::new();
        let (path, field) = match action {
            CodeAction::Get => (format!("/v1/codes/{}", code()?), "meaning"),
            CodeAction::Search => {
                for (name, value) in [
                    ("q", lookup.query.clone()),
                    ("system", lookup.system.clone()),
                    ("limit", lookup.limit.map(|limit| limit.to_string())),
                    ("cursor", lookup.cursor.clone()),
                ] {
                    if let Some(value) = value {
                        query.push((name, value));
                    }
                }
                ("/v1/codes".to_owned(), "codes")
            }
            CodeAction::Vehicle => {
                let code = code()?;
                let id = lookup.id.clone().unwrap_or_default();
                if VehicleId::parse(&id).is_none() {
                    return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
                }
                (format!("/v1/vehicles/{id}/codes/{code}"), "related")
            }
        };
        let answer = self.get(&path, &query)?;
        if answer.get(field).is_some() {
            Ok(answer)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// The recall routes. Whatever goes in the path has been checked to be
    /// an id, a VIN, a publisher or a campaign number.
    fn recalls(&self, action: RecallAction, lookup: &RecallLookup) -> Result<Value, CliError> {
        let text = |field: &Option<String>| field.clone().unwrap_or_default();
        let mut query: Vec<(&str, String)> = Vec::new();
        let mut add = |name: &'static str, value: Option<String>| {
            if let Some(value) = value {
                query.push((name, value));
            }
        };
        let path = match action {
            RecallAction::Vehicle => {
                let id = text(&lookup.id);
                if VehicleId::parse(&id).is_none() {
                    return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
                }
                add("country", lookup.country.clone());
                format!("/v1/vehicles/{id}/recalls")
            }
            RecallAction::Vin => {
                add("country", lookup.country.clone());
                add("year", lookup.year.map(|year| year.to_string()));
                format!("/v1/vin/{}/recalls", text(&lookup.vin))
            }
            RecallAction::Campaign => format!(
                "/v1/recalls/{}/{}",
                text(&lookup.source),
                text(&lookup.campaign)
            ),
            RecallAction::Search => {
                add("make", lookup.make.clone());
                add("model", lookup.model.clone());
                add("year", lookup.year.map(|year| year.to_string()));
                add("component", lookup.component.clone());
                add("q", lookup.query.clone());
                add("country", lookup.country.clone());
                add("limit", lookup.limit.map(|limit| limit.to_string()));
                add("cursor", lookup.cursor.clone());
                "/v1/recalls".to_owned()
            }
            RecallAction::Recent => {
                add("since", lookup.since.clone());
                add("country", lookup.country.clone());
                add("make", lookup.make.clone());
                add("limit", lookup.limit.map(|limit| limit.to_string()));
                add("cursor", lookup.cursor.clone());
                "/v1/recalls/recent".to_owned()
            }
        };
        let answer = self.get(&path, &query)?;
        // Every recall answer says so.
        if answer.get("may_apply") == Some(&Value::Bool(true)) {
            Ok(answer)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// The bulletin routes. Whatever goes in the path has been checked to
    /// be an id, a VIN or a number.
    fn bulletins(
        &self,
        action: BulletinAction,
        lookup: &BulletinLookup,
    ) -> Result<Value, CliError> {
        let text = |field: &Option<String>| field.clone().unwrap_or_default();
        let mut query: Vec<(&str, String)> = Vec::new();
        let mut add = |name: &'static str, value: Option<String>| {
            if let Some(value) = value {
                query.push((name, value));
            }
        };
        let path = match action {
            BulletinAction::Vehicle => {
                let id = text(&lookup.id);
                if VehicleId::parse(&id).is_none() {
                    return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
                }
                format!("/v1/vehicles/{id}/bulletins")
            }
            BulletinAction::Vin => {
                add("year", lookup.year.map(|year| year.to_string()));
                format!("/v1/vin/{}/bulletins", text(&lookup.vin))
            }
            BulletinAction::Get => match lookup.nhtsa_id {
                Some(nhtsa_id) => format!("/v1/bulletins/{nhtsa_id}"),
                None => return Err(CliError::new(NOT_FOUND, "No bulletin has that NHTSA id.")),
            },
            BulletinAction::Search => {
                add("make", lookup.make.clone());
                add("model", lookup.model.clone());
                add("year", lookup.year.map(|year| year.to_string()));
                "/v1/bulletins".to_owned()
            }
        };
        // The filters every list takes, after the ones an action has.
        if action != BulletinAction::Get {
            add("type", lookup.kind.clone());
            add("component", lookup.component.clone());
            add("code", lookup.code.clone());
            add("topic", lookup.topic.clone());
            add("q", lookup.query.clone());
            add("limit", lookup.limit.map(|limit| limit.to_string()));
            add("cursor", lookup.cursor.clone());
        }
        let answer = self.get(&path, &query)?;
        // Every bulletin answer carries the note that says what a bulletin
        // is.
        if answer.get("note").is_some_and(Value::is_string) {
            Ok(answer)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// `vin` is well formed, so it is safe in a path.
    fn decode(&self, vin: &str, year: Option<u16>) -> Result<Value, CliError> {
        let query: Vec<(&str, String)> =
            year.iter().map(|year| ("year", year.to_string())).collect();
        let decoded = self.get(&format!("/v1/vin/{vin}"), &query)?;
        if decoded.get("vin").is_some_and(Value::is_string) {
            Ok(decoded)
        } else {
            Err(self.not_the_api(200))
        }
    }

    /// Decodes each VIN in order. Only well-formed VINs are sent; the rest
    /// get the error the API would have given, without a request.
    fn batch(&self, vins: &[String], year: Option<u16>) -> Result<Value, CliError> {
        let current_year = current_year();
        let checked: Vec<Result<String, CliError>> = vins
            .iter()
            .map(|vin| {
                let one = Request::VinDecode {
                    vin: vin.clone(),
                    year,
                };
                match one.checked(current_year)? {
                    Request::VinDecode { vin, .. } => Ok(vin),
                    _ => Err(self.not_the_api(200)),
                }
            })
            .collect();
        let sound: Vec<&String> = checked.iter().filter_map(|vin| vin.as_ref().ok()).collect();
        let mut decoded = if sound.is_empty() {
            Vec::new()
        } else if let Some(year) = year {
            // The batch route takes no year, so each VIN is asked for alone.
            sound
                .iter()
                .map(|vin| {
                    self.decode(vin, Some(year))
                        .unwrap_or_else(|error| error.body())
                })
                .collect()
        } else {
            let url = format!("{}/v1/vin/batch", self.base);
            let body = json!({ "vins": sound }).to_string();
            let answer = self
                .agent
                .post(&url)
                .content_type("application/json")
                .send(body);
            match self.read(answer)? {
                Value::Array(items) if items.len() == sound.len() => items,
                _ => return Err(self.not_the_api(200)),
            }
        }
        .into_iter();
        Ok(Value::Array(
            checked
                .into_iter()
                .map(|vin| match vin {
                    Ok(_) => decoded.next().unwrap_or(Value::Null),
                    Err(error) => error.body(),
                })
                .collect(),
        ))
    }

    fn vehicles(&self, action: Action, lookup: &Lookup) -> Result<Value, CliError> {
        if action == Action::Entry {
            let id = lookup.id.clone().unwrap_or_default();
            // Only text that has the form of an id is put in a path.
            if VehicleId::parse(&id).is_none() {
                return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
            }
            let entry = self.get(&format!("/v1/vehicles/{id}"), &[])?;
            return if entry.get("id").is_some_and(Value::is_string) {
                Ok(entry)
            } else {
                Err(self.not_the_api(200))
            };
        }
        if action == Action::Safety {
            let id = lookup.id.clone().unwrap_or_default();
            // Only text that has the form of an id is put in a path.
            if VehicleId::parse(&id).is_none() {
                return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
            }
            let safety = self.get(&format!("/v1/vehicles/{id}/safety"), &[])?;
            // Every answer has the list, empty or not.
            return if safety.get("ratings").is_some_and(Value::is_array) {
                Ok(safety)
            } else {
                Err(self.not_the_api(200))
            };
        }
        if action == Action::Complaints {
            let id = lookup.id.clone().unwrap_or_default();
            // Only text that has the form of an id is put in a path.
            if VehicleId::parse(&id).is_none() {
                return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
            }
            let complaints = self.get(&format!("/v1/vehicles/{id}/complaints"), &[])?;
            // Every answer has the list, empty or not.
            return if complaints.get("components").is_some_and(Value::is_array) {
                Ok(complaints)
            } else {
                Err(self.not_the_api(200))
            };
        }
        if action == Action::Overview {
            let id = lookup.id.clone().unwrap_or_default();
            // Only text that has the form of an id is put in a path.
            if VehicleId::parse(&id).is_none() {
                return Err(CliError::new(NOT_FOUND, "No vehicle has that id."));
            }
            return self.overview(&format!("/v1/vehicles/{id}/overview"), &[]);
        }
        if action == Action::ModelYears {
            let part = |field: &Option<String>| segment(field.as_deref().unwrap_or_default());
            let path = format!("/v1/models/{}/{}", part(&lookup.make), part(&lookup.model));
            let years = self.get(&path, &[])?;
            return if years.get("may_apply") == Some(&Value::Bool(true)) {
                Ok(years)
            } else {
                Err(self.not_the_api(200))
            };
        }
        let mut query: Vec<(&str, String)> = Vec::new();
        let mut add = |name: &'static str, value: Option<String>| {
            if let Some(value) = value {
                query.push((name, value));
            }
        };
        add("q", lookup.query.clone());
        add("make", lookup.make.clone());
        add("model", lookup.model.clone());
        add("year", lookup.year.map(|year| year.to_string()));
        add("submodel", lookup.submodel.clone());
        add("scope", lookup.scope.clone());
        add("term", lookup.term.clone());
        add("limit", lookup.limit.map(|limit| limit.to_string()));
        let list = self.get(&format!("/v1/vehicles/{}", action.name()), &query)?;
        if list.is_array() {
            Ok(list)
        } else {
            Err(self.not_the_api(200))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_addresses_are_accepted() {
        for base in [
            "https://open.wenmarpro.com",
            "http://localhost:3000/",
            " http://127.0.0.1:1 ",
        ] {
            assert!(Remote::new(base).is_ok(), "{base}");
        }
        assert_eq!(
            Remote::new("https://open.wenmarpro.com//").unwrap().base(),
            "https://open.wenmarpro.com"
        );
        for base in [
            "",
            "open.wenmarpro.com",
            "ftp://example.test",
            "file:///etc/passwd",
            "https://",
            "http:///path",
            "https://exa mple.test",
            "javascript:alert(1)",
        ] {
            let error = Remote::new(base).err().unwrap();
            assert_eq!(error.code, "usage", "{base}");
        }
    }

    #[test]
    fn a_name_in_a_path_is_one_part_of_it() {
        assert_eq!(segment("f-150"), "f-150");
        assert_eq!(segment("Land Rover"), "Land%20Rover");
        assert_eq!(segment("a/b"), "a%2Fb");
        assert_eq!(segment(".."), "%2E%2E");
        assert_eq!(segment("Citroën?x=1#y"), "Citro%C3%ABn%3Fx%3D1%23y");
        assert_eq!(segment(""), "");
    }

    #[test]
    fn requests_are_marked_with_the_tool_and_its_version_only() {
        assert_eq!(
            user_agent(),
            format!("wenmar-open/{}", env!("CARGO_PKG_VERSION"))
        );
    }
}
