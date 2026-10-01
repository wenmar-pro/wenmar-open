//! Request logging that never records a whole VIN.
//!
//! The last six characters of a VIN are the serial number of one vehicle.
//! Logs keep the first eleven, which name the maker, the model and the
//! plant, and nothing after them.
//!
//! A VIN can be typed anywhere, not only where the API expects one, so the
//! rule is about the address and not about VINs: a part of the path is
//! logged whole only where it is a fixed part of the route that matched.
//! Every other part is the caller's text and is cut to eleven characters,
//! and so is the method. Query strings and bodies are not logged.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

/// How many characters of a VIN, or of anything else the caller chose, may
/// be logged.
pub const VIN_KEPT: usize = 11;

/// Most characters of a path that are logged.
pub const LONGEST: usize = 200;

/// The path as it may be written to a log.
///
/// `route` is the route that matched, such as `/v1/vin/{vin}`, when one did.
pub fn loggable(path: &str, route: Option<&str>) -> String {
    // The route's parts line up with the path's only when there are as many
    // of them. Otherwise nothing in the path is taken as fixed.
    let fixed: Vec<&str> = route
        .map(|route| route.split('/').collect::<Vec<&str>>())
        .filter(|parts| parts.len() == path.split('/').count())
        .unwrap_or_default();
    let mut logged = String::new();
    let mut length = 0;
    for (index, part) in path.split('/').enumerate() {
        if index > 0 {
            logged.push('/');
            length += 1;
        }
        let whole = fixed.get(index).is_some_and(|fixed| *fixed == part);
        let kept = if whole { usize::MAX } else { VIN_KEPT };
        for character in part.chars().take(kept) {
            logged.push(character);
            length += 1;
        }
        if length > LONGEST {
            let mut short: String = logged.chars().take(LONGEST).collect();
            short.push('…');
            return short;
        }
    }
    logged
}

/// One line per request: method, path, status and time taken.
pub fn line(
    method: &str,
    path: &str,
    route: Option<&str>,
    status: u16,
    milliseconds: u128,
) -> String {
    // The method is the caller's text too: anything may be sent as one.
    let method: String = method.chars().take(VIN_KEPT).collect();
    format!(
        "{method} {} {status} {milliseconds}ms",
        loggable(path, route)
    )
}

pub async fn log(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let route = request.extensions().get::<MatchedPath>().cloned();
    let response = next.run(request).await;
    if uri.path() != "/health" {
        tracing::info!(
            "{}",
            line(
                method.as_str(),
                uri.path(),
                route.as_ref().map(MatchedPath::as_str),
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
        assert_eq!(
            loggable("/v1/vin/KM8K2CAB4PU001140", None),
            "/v1/vin/KM8K2CAB4PU"
        );
        assert_eq!(loggable("/vin/KM8K2CAB4PU001140", None), "/vin/KM8K2CAB4PU");
        assert_eq!(
            loggable("/vin/KM8K2CAB4PU001140.md", None),
            "/vin/KM8K2CAB4PU"
        );
        let line = line("GET", "/v1/vin/KM8K2CAB4PU001140", None, 200, 3);
        assert_eq!(line, "GET /v1/vin/KM8K2CAB4PU 200 3ms");
        assert!(!line.contains("001140"));
    }

    #[test]
    fn whatever_is_typed_in_place_of_a_vin_is_cut_too() {
        // Spaces and dashes are allowed in the input, so the serial number
        // could sit further along. Cutting the raw text is always safe.
        assert_eq!(
            loggable("/v1/vin/KM8-K2CAB-4PU-001140", None),
            "/v1/vin/KM8-K2CAB-4"
        );
        assert_eq!(
            loggable("/v1/vin/%20KM8K2CAB4PU001140", None),
            "/v1/vin/%20KM8K2CAB"
        );
        assert_eq!(
            loggable("/v1/vin/é€KM8K2CAB4PU001140", None),
            "/v1/vin/é€KM8K2CAB4"
        );
        assert_eq!(loggable("/v1/vin/", None), "/v1/vin/");
    }

    #[test]
    fn other_paths_are_logged_as_they_are() {
        assert_eq!(loggable("/v1/vin/batch", None), "/v1/vin/batch");
        assert_eq!(loggable("/v1/vehicles/makes", None), "/v1/vehicles/makes");
        assert_eq!(
            loggable("/makes/honda/civic/2019", None),
            "/makes/honda/civic/2019"
        );
        assert_eq!(loggable("/", None), "/");
    }

    #[test]
    fn a_vin_sent_to_any_other_address_is_cut_too() {
        for (path, route) in [
            ("/v1/vehicles/1HGCM82633A004352", Some("/v1/vehicles/{id}")),
            ("/v1/vehicles/1HGCM82633A004352", None),
            ("/V1/VIN/1HGCM82633A004352", None),
            ("//v1/vin/1HGCM82633A004352", None),
            ("/api/v1/vin/1HGCM82633A004352", None),
            ("/v1/vins/1HGCM82633A004352", None),
            ("/1HGCM82633A004352", None),
            ("/v1/vin%2F1HGCM82633A004352", None),
            ("/v1/vin/1HGCM82633A004352/", None),
            ("/v1/vin/1HGCM82633A004352/x", Some("/v1/vin/{vin}")),
            ("/v1/vin/1HG-CM82633A-004352", Some("/v1/vin/{vin}")),
        ] {
            let logged = loggable(path, route);
            assert!(!logged.contains("004352"), "{path} is logged as {logged}");
            assert!(!logged.contains("4352"), "{path} is logged as {logged}");
        }
        assert_eq!(
            loggable("/v1/vehicles/1HGCM82633A004352", Some("/v1/vehicles/{id}")),
            "/v1/vehicles/1HGCM82633A"
        );
        assert_eq!(
            loggable("/v1/vin/1HGCM82633A004352", Some("/v1/vin/{vin}")),
            "/v1/vin/1HGCM82633A"
        );
        assert_eq!(
            loggable("/api/v1/vin/1HGCM82633A004352", None),
            "/api/v1/vin/1HGCM82633A"
        );
    }

    #[test]
    fn the_fixed_parts_of_a_route_are_logged_whole() {
        assert_eq!(
            loggable("/v1/vehicles/submodels", Some("/v1/vehicles/submodels")),
            "/v1/vehicles/submodels"
        );
        assert_eq!(
            loggable("/v1/openapi.json", Some("/v1/openapi.json")),
            "/v1/openapi.json"
        );
        // The part the caller chose is cut; the parts the route names are not.
        assert_eq!(
            loggable(
                "/v1/vehicles/2019_honda_civic_si",
                Some("/v1/vehicles/{id}")
            ),
            "/v1/vehicles/2019_honda_"
        );
        // An address that is no route is all the caller's choice.
        assert_eq!(loggable("/v1/openapi.jsonx", None), "/v1/openapi.jso");
    }

    #[test]
    fn a_very_long_path_is_logged_short() {
        let many_parts = "/abc".repeat(15_000);
        let one_part = format!("/{}", "A".repeat(60_000));
        for path in [many_parts, one_part] {
            let logged = loggable(&path, None);
            assert!(
                logged.chars().count() <= LONGEST + 1,
                "{} characters",
                logged.chars().count()
            );
        }
        assert!(loggable(&"/abc".repeat(15_000), None).ends_with('…'));
    }

    #[test]
    fn a_method_that_is_really_a_vin_is_cut() {
        let line = line("1HGCM82633A004352", "/", None, 405, 1);
        assert_eq!(line, "1HGCM82633A / 405 1ms");
    }
}
