//! Answers from the hosted API, over HTTP.

use std::time::Duration;

use serde_json::{Value, json};
use wenmar_vehicles::VehicleId;

use crate::error::{BAD_RESPONSE, CliError, NETWORK, NOT_FOUND, USAGE};
use crate::request::{Action, Lookup, Request, current_year};

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

/// Whether `url` is an address this tool will make a request to.
pub fn is_http(url: &str) -> bool {
    ["http://", "https://"].iter().any(|scheme| {
        url.strip_prefix(scheme)
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
    })
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
            Request::Vehicles { action, lookup } => self.vehicles(*action, lookup),
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
    fn requests_are_marked_with_the_tool_and_its_version_only() {
        assert_eq!(
            user_agent(),
            format!("wenmar-open/{}", env!("CARGO_PKG_VERSION"))
        );
    }
}
