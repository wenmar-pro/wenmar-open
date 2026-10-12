//! Reads the links to each maker's own pages from `data/catalog/makes.yaml`,
//! checks them, and writes them out as Rust for the server to compile in.
//!
//! The data file has no column for a link and its schema does not change,
//! so the server carries the links itself. The build script reads the file
//! once, here, and a link that is not as described below stops the build
//! with the make and the reason. So a mistake is found by whoever made it,
//! and never by a visitor.
//!
//! This file is compiled into the build script, and into the crate's unit
//! tests, which run the same checks on samples that are wrong.

use serde::Deserialize;

/// Where the file is, from this crate's directory.
pub const PATH: &str = "../../data/catalog/makes.yaml";
/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "make_links.rs";
/// The longest address accepted.
const LONGEST: usize = 300;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    makes: Vec<Make>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Make {
    name: String,
    /// Read by `open-data`, not here.
    #[serde(default)]
    #[allow(dead_code)]
    aliases: Vec<String>,
    #[serde(default)]
    links_checked: Option<String>,
    #[serde(default)]
    recall_lookup_url: Option<String>,
    #[serde(default)]
    manual_url: Option<String>,
    #[serde(default)]
    maintenance_url: Option<String>,
    #[serde(default)]
    service_info_url: Option<String>,
    #[serde(default)]
    parts_url: Option<String>,
    #[serde(default)]
    customer_line: Option<String>,
}

impl Make {
    /// The five addresses with their field names, in the order a page lists them.
    fn addresses(&self) -> [(&'static str, &Option<String>); 5] {
        [
            ("recall_lookup_url", &self.recall_lookup_url),
            ("manual_url", &self.manual_url),
            ("maintenance_url", &self.maintenance_url),
            ("service_info_url", &self.service_info_url),
            ("parts_url", &self.parts_url),
        ]
    }

    fn any(&self) -> bool {
        self.customer_line.is_some() || self.addresses().iter().any(|(_, value)| value.is_some())
    }
}

/// The matching form of a make's name: the same as
/// `wenmar_vehicles::text::normalize`, which a build script cannot call.
fn norm(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// A day written `2026-10-12`.
fn is_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(at, byte)| {
            if at == 4 || at == 7 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
        && (1..=12).contains(&text[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&text[8..10].parse::<u8>().unwrap_or(0))
}

/// An address safe in an `href` and in a Markdown link.
fn is_address(text: &str) -> bool {
    text.len() <= LONGEST
        && text.strip_prefix("https://").is_some_and(|rest| {
            rest.contains('.')
                && rest
                    .chars()
                    .all(|c| !c.is_whitespace() && !c.is_control() && !"\"<>()".contains(c))
        })
}

/// A telephone number as a maker prints it: `1-800-555-0100`, `1 (800) 33-HONDA`.
fn is_phone(text: &str) -> bool {
    (7..=40).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " +-().".contains(c))
        && text.chars().filter(char::is_ascii_alphanumeric).count() >= 7
}

fn check(file: &File) -> Result<(), String> {
    for make in &file.makes {
        let name = &make.name;
        match (&make.links_checked, make.any()) {
            (None, false) => continue,
            (None, true) => return Err(format!("{name} has a link but no links_checked")),
            (Some(_), false) => return Err(format!("{name} has links_checked but no link")),
            (Some(day), true) if !is_day(day) => {
                return Err(format!(
                    "{name}: links_checked must be a day written \"2026-10-12\", but it is {day:?}"
                ));
            }
            (Some(_), true) => {}
        }
        for (field, value) in make.addresses() {
            if let Some(address) = value
                && !is_address(address)
            {
                return Err(format!(
                    "{name}: {field} must be an https:// address of at most {LONGEST} characters with no space, quote, bracket or parenthesis, but it is {address:?}"
                ));
            }
        }
        if let Some(line) = &make.customer_line
            && !is_phone(line)
        {
            return Err(format!(
                "{name}: customer_line must be a telephone number of 7 to 40 characters, but it is {line:?}"
            ));
        }
    }
    Ok(())
}

fn option(value: &Option<String>) -> String {
    match value {
        Some(text) => format!("Some({text:?})"),
        None => "None".to_owned(),
    }
}

fn write(file: &File) -> String {
    let with_links: Vec<&Make> = file
        .makes
        .iter()
        .filter(|make| make.links_checked.is_some())
        .collect();
    let mut code = String::from(
        "// Written by links_build.rs from data/catalog/makes.yaml. Change that file, not this one.\n",
    );
    code.push_str(&format!(
        "pub static MAKE_LINKS: [MakeLinks; {}] = [\n",
        with_links.len()
    ));
    for make in with_links {
        code.push_str(&format!(
            "MakeLinks {{ norm: {:?}, checked: {:?}, recall_lookup_url: {}, manual_url: {}, maintenance_url: {}, service_info_url: {}, parts_url: {}, customer_line: {} }},\n",
            norm(&make.name),
            make.links_checked.clone().unwrap_or_default(),
            option(&make.recall_lookup_url),
            option(&make.manual_url),
            option(&make.maintenance_url),
            option(&make.service_info_url),
            option(&make.parts_url),
            option(&make.customer_line),
        ));
    }
    code.push_str("];\n");
    code
}

/// The links as Rust, or why the file cannot be used.
pub fn generate(yaml: &str) -> Result<String, String> {
    let file: File = serde_saphyr::from_str(yaml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "makes:\n  - name: Toyota\n    aliases: [yota]\n  - name: Mercedes-Benz\n    links_checked: \"2026-10-12\"\n    recall_lookup_url: https://example.org/recalls?a=1&b=2\n    customer_line: \"1 (800) 555-0100\"\n";

    #[test]
    fn only_makes_with_links_are_written() {
        let code = generate(SAMPLE).unwrap();
        assert!(
            code.contains("pub static MAKE_LINKS: [MakeLinks; 1] = [\n"),
            "{code}"
        );
        assert!(code.contains("MakeLinks { norm: \"mercedesbenz\", checked: \"2026-10-12\", recall_lookup_url: Some(\"https://example.org/recalls?a=1&b=2\"), manual_url: None, maintenance_url: None, service_info_url: None, parts_url: None, customer_line: Some(\"1 (800) 555-0100\") },\n"), "{code}");
        assert!(
            generate("makes: []\n")
                .unwrap()
                .ends_with("pub static MAKE_LINKS: [MakeLinks; 0] = [\n];\n")
        );
    }

    #[test]
    fn a_mistake_is_refused_with_the_make_and_the_reason() {
        for (lines, expected) in [
            (
                "    manual_url: https://example.org/m\n",
                "Honda has a link but no links_checked",
            ),
            (
                "    links_checked: \"2026-10-12\"\n",
                "Honda has links_checked but no link",
            ),
            (
                "    links_checked: \"12 Oct 2026\"\n    manual_url: https://example.org/m\n",
                "links_checked must be a day",
            ),
            (
                "    links_checked: \"2026-13-01\"\n    manual_url: https://example.org/m\n",
                "links_checked must be a day",
            ),
            (
                "    links_checked: \"2026-10-12\"\n    manual_url: http://example.org/m\n",
                "Honda: manual_url must be an https:// address",
            ),
            (
                "    links_checked: \"2026-10-12\"\n    parts_url: \"https://example.org/a b\"\n",
                "Honda: parts_url must be",
            ),
            (
                "    links_checked: \"2026-10-12\"\n    parts_url: \"https://example.org/a)b\"\n",
                "Honda: parts_url must be",
            ),
            (
                "    links_checked: \"2026-10-12\"\n    service_info_url: \"https://localhost\"\n",
                "Honda: service_info_url must be",
            ),
            (
                "    links_checked: \"2026-10-12\"\n    customer_line: \"call us\"\n",
                "Honda: customer_line must be",
            ),
            (
                "    links_checked: \"2026-10-12\"\n    manuals_url: https://example.org/m\n",
                "manuals_url",
            ),
        ] {
            let error = generate(&format!("makes:\n  - name: Honda\n{lines}")).unwrap_err();
            assert!(error.contains(expected), "{lines}: {error}");
        }
    }

    #[test]
    fn the_matching_form_is_the_catalogs() {
        for name in [
            "Mercedes-Benz",
            "Land Rover",
            "MINI",
            "smart",
            "B & B <Trailers>",
        ] {
            assert_eq!(norm(name), wenmar_vehicles::text::normalize(name), "{name}");
        }
    }

    #[test]
    fn the_file_in_the_repository_is_accepted_and_is_part_of_the_build_id() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PATH);
        let code = generate(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // What the build script wrote is what this code writes now.
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/make_links.rs"))
        );
        assert_eq!(OUT, "make_links.rs");
        for part in ["links_build.rs", PATH] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }
}
