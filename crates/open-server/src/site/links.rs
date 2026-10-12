//! Links to each maker's own pages, from `data/catalog/makes.yaml`.
//!
//! They are compiled in: see `links_build.rs` beside this crate's
//! `Cargo.toml`. The data file does not hold them.

use wenmar_vehicles::text::normalize;

use crate::site::markdown::escape;

/// One make's links. `norm` is the matching form of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MakeLinks {
    pub norm: &'static str,
    /// The day a person last opened every link, as `2026-10-12`.
    pub checked: &'static str,
    pub recall_lookup_url: Option<&'static str>,
    pub manual_url: Option<&'static str>,
    pub maintenance_url: Option<&'static str>,
    pub service_info_url: Option<&'static str>,
    pub parts_url: Option<&'static str>,
    pub customer_line: Option<&'static str>,
    /// The address of one model year's manuals, with `{year}` and
    /// `{model}` to fill in. See [`fill`].
    pub manual_url_template: Option<&'static str>,
    pub maintenance_url_template: Option<&'static str>,
}

include!(concat!(env!("OUT_DIR"), "/make_links.rs"));

/// An address pattern filled in for one model year. `model` is the
/// catalog's id form of the model, such as `f-150`.
pub fn fill(template: &str, year: u16, model: &str) -> String {
    template
        .replace("{year}", &year.to_string())
        .replace("{model}", model)
}

/// The links of the make called `make`, among `all`.
pub fn find<'a>(all: &'a [MakeLinks], make: &str) -> Option<&'a MakeLinks> {
    let norm = normalize(make);
    all.iter().find(|links| links.norm == norm)
}

/// The links recorded for the make called `make`.
pub fn for_make(make: &str) -> Option<&'static MakeLinks> {
    find(&MAKE_LINKS, make)
}

/// What a page shows: the make as it is written, each link with its words,
/// the telephone line, and the day they were checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkList {
    pub make: String,
    pub items: Vec<(String, &'static str)>,
    pub customer_line: Option<&'static str>,
    pub checked: &'static str,
}

impl MakeLinks {
    pub fn list(&self, make: &str) -> LinkList {
        let items = [
            (
                format!("Check a VIN for recalls at {make}"),
                self.recall_lookup_url,
            ),
            (format!("Owner's manuals from {make}"), self.manual_url),
            (
                format!("Maintenance schedules from {make}"),
                self.maintenance_url,
            ),
            (
                format!("Service information from {make}"),
                self.service_info_url,
            ),
            (format!("Parts catalogue from {make}"), self.parts_url),
        ]
        .into_iter()
        .filter_map(|(words, address)| address.map(|address| (words, address)))
        .collect();
        LinkList {
            make: make.to_owned(),
            items,
            customer_line: self.customer_line,
            checked: self.checked,
        }
    }
}

/// The section a Markdown page ends with.
pub fn markdown(list: &LinkList) -> String {
    let make = escape(&list.make);
    let mut text = format!("\n## From {make}\n\n");
    for (words, address) in &list.items {
        text.push_str(&format!("- [{}]({address})\n", escape(words)));
    }
    if let Some(line) = list.customer_line {
        text.push_str(&format!("- {make} customer line: {}\n", escape(line)));
    }
    text.push_str(&format!(
        "\nThese lead to the maker's own site. Checked {}.\n",
        list.checked
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: [MakeLinks; 1] = [MakeLinks {
        norm: "mercedesbenz",
        checked: "2026-10-12",
        recall_lookup_url: Some("https://example.org/recalls?a=1&b=2"),
        manual_url: None,
        maintenance_url: Some("https://example.org/maintenance"),
        service_info_url: None,
        parts_url: None,
        customer_line: Some("1 (800) 555-0100"),
        manual_url_template: Some("https://example.org/owners/{year}/{model}/manuals"),
        maintenance_url_template: None,
    }];

    #[test]
    fn a_make_is_found_by_its_name_however_it_is_written() {
        assert_eq!(find(&SAMPLE, "Mercedes-Benz"), Some(&SAMPLE[0]));
        assert_eq!(find(&SAMPLE, "MERCEDES BENZ"), Some(&SAMPLE[0]));
        assert_eq!(find(&SAMPLE, "Mercedes"), None);
        assert_eq!(find(&[], "Honda"), None);
    }

    #[test]
    fn only_what_was_recorded_is_listed_in_a_fixed_order() {
        let list = SAMPLE[0].list("Mercedes-Benz");
        assert_eq!(
            list.items,
            vec![
                (
                    "Check a VIN for recalls at Mercedes-Benz".to_owned(),
                    "https://example.org/recalls?a=1&b=2"
                ),
                (
                    "Maintenance schedules from Mercedes-Benz".to_owned(),
                    "https://example.org/maintenance"
                ),
            ]
        );
        assert_eq!(
            markdown(&list),
            "\n## From Mercedes\\-Benz\n\n- [Check a VIN for recalls at Mercedes\\-Benz](https://example.org/recalls?a=1&b=2)\n- [Maintenance schedules from Mercedes\\-Benz](https://example.org/maintenance)\n- Mercedes\\-Benz customer line: 1 \\(800\\) 555\\-0100\n\nThese lead to the maker's own site. Checked 2026-10-12.\n"
        );
    }

    #[test]
    fn every_make_with_links_has_something_to_show() {
        for links in &MAKE_LINKS {
            let list = links.list("A make");
            assert!(
                !list.items.is_empty()
                    || list.customer_line.is_some()
                    || links.manual_url_template.is_some()
                    || links.maintenance_url_template.is_some(),
                "{}",
                links.norm
            );
        }
    }

    #[test]
    fn a_pattern_is_filled_with_the_year_and_the_models_id_form() {
        assert_eq!(
            fill(SAMPLE[0].manual_url_template.unwrap(), 2001, "sampler"),
            "https://example.org/owners/2001/sampler/manuals"
        );
        assert_eq!(
            fill(
                "https://example.org/m?model={model}&also={model}",
                2001,
                "f-150"
            ),
            "https://example.org/m?model=f-150&also=f-150"
        );
        // A pattern does not add to the make's own list.
        assert_eq!(SAMPLE[0].list("Mercedes-Benz").items.len(), 2);
    }
}
