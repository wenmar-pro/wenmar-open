//! "Report an error": the two addresses a page of data offers.
//!
//! One opens the data-correction issue form on GitHub with the page and the
//! vehicle filled in. The other, there only when `OPEN_FEEDBACK_EMAIL` is
//! set, opens the visitor's mail program with the same. The server sends
//! nothing itself.

use crate::config::Config;

/// The issue form in `.github/ISSUE_TEMPLATE/` the link opens. Its field
/// ids `page` and `vehicle` are the query parameters used here.
pub const ISSUE_TEMPLATE: &str = "data_correction.yml";

/// Where a visitor can report an error on one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportLinks {
    /// The issue form, filled in.
    pub issue: String,
    /// A `mailto:` address, filled in. `None` when no address is set.
    pub mail: Option<String>,
    /// The form on this site, filled in. `None` when no token is set.
    pub form: Option<String>,
}

/// Text made safe for a query string and for `mailto:`: every byte that is
/// not an ASCII letter, a digit or one of `-_.~` becomes `%XX`. A space is
/// `%20`, never `+`, because a mail program shows `+` as it is.
pub fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// The part of a VIN that may go into a public report: the first 11
/// characters, which name the maker, the model and the plant, and not the
/// one vehicle.
pub fn vin_stem(vin: &str) -> &str {
    vin.get(..11).unwrap_or(vin)
}

/// The links for a page at the full address `page`.
pub fn links_to(config: &Config, page: &str, vehicle: &str) -> ReportLinks {
    let title = format!("Data correction: {vehicle}");
    let issue = format!(
        "https://github.com/{}/issues/new?template={ISSUE_TEMPLATE}&title={}&page={}&vehicle={}",
        config.github_repo,
        encode(&title),
        encode(page),
        encode(vehicle)
    );
    let mail = config.feedback_email.as_ref().map(|email| {
        let body = format!(
            "Page: {page}\r\nVehicle: {vehicle}\r\n\r\nWhat is wrong:\r\n\r\n\r\nWhat it should be, and how I know:\r\n"
        );
        format!("mailto:{email}?subject={}&body={}", encode(&title), encode(&body))
    });
    let form = config.github_token.as_ref().map(|_| {
        format!(
            "/feedback?page={}&vehicle={}",
            encode(page),
            encode(vehicle)
        )
    });
    ReportLinks { issue, mail, form }
}

/// The links for the page at `path` on this site.
pub fn links(config: &Config, path: &str, vehicle: &str) -> ReportLinks {
    links_to(config, &format!("{}{path}", config.base_url), vehicle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(email: Option<&'static str>) -> Config {
        Config::from_lookup(move |name| match name {
            "OPEN_DATA" => Some("d".to_owned()),
            "OPEN_BASE_URL" => Some("https://open.example".to_owned()),
            "OPEN_FEEDBACK_EMAIL" => email.map(str::to_owned),
            _ => None,
        })
        .unwrap()
    }

    #[test]
    fn everything_but_plain_letters_is_encoded() {
        assert_eq!(encode("Civic-Si_2.0~"), "Civic-Si_2.0~");
        assert_eq!(encode("a b&c=d/é\n<"), "a%20b%26c%3Dd%2F%C3%A9%0A%3C");
        assert_eq!(
            encode("\"<img src=x onerror=alert(1)>"),
            "%22%3Cimg%20src%3Dx%20onerror%3Dalert%281%29%3E"
        );
        assert_eq!(vin_stem("KM8K2CAB4PU001140"), "KM8K2CAB4PU");
        assert_eq!(vin_stem("KM8"), "KM8");
    }

    #[test]
    fn the_issue_link_fills_in_the_form() {
        let found = links(&config(None), "/makes/honda/civic/2019", "2019 Honda Civic");
        assert_eq!(
            found.issue,
            "https://github.com/wenmar-pro/wenmar-open/issues/new?template=data_correction.yml&title=Data%20correction%3A%202019%20Honda%20Civic&page=https%3A%2F%2Fopen.example%2Fmakes%2Fhonda%2Fcivic%2F2019&vehicle=2019%20Honda%20Civic"
        );
        assert_eq!(found.mail, None);
    }

    #[test]
    fn the_mail_link_is_there_only_with_an_address() {
        let config = config(Some("data@open.example"));
        let mail = links(&config, "/makes/honda/civic/2019", "2019 Honda Civic")
            .mail
            .unwrap();
        assert!(
            mail.starts_with(
                "mailto:data@open.example?subject=Data%20correction%3A%202019%20Honda%20Civic&body=Page%3A%20https%3A%2F%2Fopen.example%2Fmakes%2Fhonda%2Fcivic%2F2019%0D%0AVehicle%3A%202019%20Honda%20Civic%0D%0A%0D%0AWhat%20is%20wrong%3A%0D%0A"
            ),
            "{mail}"
        );
        assert!(mail.ends_with("how%20I%20know%3A%0D%0A"), "{mail}");
        assert!(!mail.contains([' ', '\n', '\r', '<', '+']), "{mail}");
    }

    #[test]
    fn the_form_the_link_opens_has_the_fields_the_link_fills() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/ISSUE_TEMPLATE")
            .join(ISSUE_TEMPLATE);
        let form = std::fs::read_to_string(&path).unwrap();
        for id in ["page", "vehicle", "field", "wrong", "correct"] {
            assert!(form.contains(&format!("    id: {id}\n")), "no field {id}");
        }
    }
}
