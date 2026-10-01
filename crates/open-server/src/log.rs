//! Request logging that never records a whole VIN.
//!
//! The last six characters of a VIN are the serial number of one vehicle.
//! Logs keep the first eleven, which name the maker, the model and the
//! plant, and nothing after them. Query strings and bodies are not logged.

use std::time::Instant;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Paths whose next part is a VIN, or something typed where a VIN goes.
const VIN_PREFIXES: [&str; 2] = ["/v1/vin/", "/vin/"];

/// How many characters of a VIN may be logged.
pub const VIN_KEPT: usize = 11;

/// The path as it may be written to a log.
pub fn loggable(path: &str) -> String {
    for prefix in VIN_PREFIXES {
        if let Some(rest) = path.strip_prefix(prefix) {
            let kept: String = rest.chars().take(VIN_KEPT).collect();
            return format!("{prefix}{kept}");
        }
    }
    path.to_owned()
}

/// One line per request: method, path, status and time taken.
pub fn line(method: &str, path: &str, status: u16, milliseconds: u128) -> String {
    format!("{method} {} {status} {milliseconds}ms", loggable(path))
}

pub async fn log(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    if path != "/health" {
        tracing::info!(
            "{}",
            line(
                &method,
                &path,
                response.status().as_u16(),
                started.elapsed().as_millis()
            )
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vin_is_cut_to_its_first_eleven_characters() {
        assert_eq!(loggable("/v1/vin/KM8K2CAB4PU001140"), "/v1/vin/KM8K2CAB4PU");
        assert_eq!(loggable("/vin/KM8K2CAB4PU001140"), "/vin/KM8K2CAB4PU");
        assert_eq!(loggable("/vin/KM8K2CAB4PU001140.md"), "/vin/KM8K2CAB4PU");
        let line = line("GET", "/v1/vin/KM8K2CAB4PU001140", 200, 3);
        assert_eq!(line, "GET /v1/vin/KM8K2CAB4PU 200 3ms");
        assert!(!line.contains("001140"));
    }

    #[test]
    fn whatever_is_typed_in_place_of_a_vin_is_cut_too() {
        // Spaces and dashes are allowed in the input, so the serial number
        // could sit further along. Cutting the raw text is always safe.
        assert_eq!(
            loggable("/v1/vin/KM8-K2CAB-4PU-001140"),
            "/v1/vin/KM8-K2CAB-4"
        );
        assert_eq!(
            loggable("/v1/vin/%20KM8K2CAB4PU001140"),
            "/v1/vin/%20KM8K2CAB"
        );
        assert_eq!(
            loggable("/v1/vin/é€KM8K2CAB4PU001140"),
            "/v1/vin/é€KM8K2CAB4"
        );
        assert_eq!(loggable("/v1/vin/"), "/v1/vin/");
    }

    #[test]
    fn other_paths_are_logged_as_they_are() {
        assert_eq!(loggable("/v1/vin/batch"), "/v1/vin/batch");
        assert_eq!(loggable("/v1/vehicles/makes"), "/v1/vehicles/makes");
        assert_eq!(
            loggable("/makes/honda/civic/2019"),
            "/makes/honda/civic/2019"
        );
        assert_eq!(loggable("/"), "/");
    }
}
