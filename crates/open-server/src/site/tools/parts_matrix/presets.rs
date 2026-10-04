//! The three example matrices of the parts matrix page, and the example
//! the page shows before anything is typed.
//!
//! They are illustrations made for this site, not survey results, and the
//! page says so. Each is an ordinary filled-in form: a preset is a link to
//! the page with these figures in its query string. The two scales are
//! built so that, on the mix of parts spend given with them, the blended
//! margin lands inside the parts target; the flat one is the single markup
//! that gives the usual target. `shop-math` checks the same figures in its
//! own tests.

use crate::site::tools::field::Sent;
use crate::site::tools::parts_matrix::form::{KIND, PART};

/// One example matrix. Each row is the cost up to, the markup and the
/// share of parts spend, as they are typed into the form.
pub struct Preset {
    /// The words of the link.
    pub label: &'static str,
    pub rows: &'static [(&'static str, &'static str, &'static str)],
}

pub static PRESETS: [Preset; 3] = [
    Preset {
        label: "Example: a sliding scale",
        rows: &[
            ("5.00", "150", "5"),
            ("25.00", "100", "20"),
            ("100.00", "80", "35"),
            ("250.00", "60", "25"),
            ("500.00", "50", "10"),
            ("1000.00", "40", "5"),
        ],
    },
    Preset {
        label: "Example: a steeper scale, for a shop that sells many cheap parts",
        rows: &[
            ("5.00", "200", "15"),
            ("25.00", "120", "30"),
            ("100.00", "70", "30"),
            ("250.00", "50", "15"),
            ("500.00", "35", "7"),
            ("1000.00", "25", "3"),
        ],
    },
    Preset {
        label: "Example: a flat markup, for comparison",
        rows: &[("1000.00", "100", "100")],
    },
];

/// The cost of the one part the page's own example prices.
const EXAMPLE_PART: &str = "42.50";

impl Preset {
    /// The preset as a filled-in form, with `part` as the one part's cost.
    fn sent_with(&self, part: &str) -> Sent {
        let mut pairs = vec![(KIND.to_owned(), "markup".to_owned())];
        for (index, (cost, rate, share)) in self.rows.iter().enumerate() {
            let number = index + 1;
            pairs.push((format!("cost{number}"), (*cost).to_owned()));
            pairs.push((format!("rate{number}"), (*rate).to_owned()));
            pairs.push((format!("share{number}"), (*share).to_owned()));
        }
        pairs.push((PART.to_owned(), part.to_owned()));
        Sent::new(pairs)
    }

    /// The preset as a filled-in form.
    pub fn sent(&self) -> Sent {
        self.sent_with("")
    }

    /// The address of the page at `path` with the preset filled in.
    pub fn href(&self, path: &str) -> String {
        format!("{path}?{}", self.sent().query())
    }
}

/// What the page shows with no query string: the sliding scale, and one
/// part priced by it.
pub fn example() -> Sent {
    PRESETS[0].sent_with(EXAMPLE_PART)
}

#[cfg(test)]
mod tests {
    use shop_math::targets::{PARTS, Standing};

    use super::*;
    use crate::site::tools::parts_matrix::form::Form;

    #[test]
    fn the_presets_land_where_the_page_says() {
        let landed: Vec<String> = PRESETS
            .iter()
            .map(|preset| {
                let (form, result) = Form::read(&preset.sent());
                assert!(form.troubles().is_empty(), "{}", preset.label);
                let result = result.unwrap();
                assert_eq!(result.tiers.len(), preset.rows.len());
                assert!(result.part.is_none());
                let blended = result.blended_margin.unwrap();
                assert_eq!(
                    PARTS.standing(blended),
                    Standing::Inside,
                    "{}",
                    preset.label
                );
                blended.to_string()
            })
            .collect();
        // The figures plan 1 records for the three.
        assert_eq!(landed, ["43.662%", "49.418%", "50%"]);
        // The flat one is the usual target itself.
        assert_eq!(landed[2], PARTS.usual.to_string());
    }

    #[test]
    fn every_preset_is_labelled_an_example_and_is_a_plain_query_string() {
        for preset in &PRESETS {
            assert!(preset.label.starts_with("Example: "), "{}", preset.label);
            let href = preset.href("/tools/parts-matrix");
            let query = href
                .strip_prefix("/tools/parts-matrix?kind=markup&cost1=")
                .unwrap();
            // Nothing in it needs escaping in an address.
            assert!(
                query
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".=&".contains(&byte)),
                "{href}"
            );
            assert!(href.ends_with("&part="), "{href}");
        }
        assert_eq!(
            PRESETS[2].href("/tools/parts-matrix"),
            "/tools/parts-matrix?kind=markup&cost1=1000.00&rate1=100&share1=100&part="
        );
    }

    #[test]
    fn the_example_is_the_sliding_scale_with_one_part_priced() {
        let (form, result) = Form::read(&example());
        assert!(form.troubles().is_empty());
        assert_eq!(form.part.value, "42.50");
        let result = result.unwrap();
        let part = result.part.unwrap();
        assert_eq!(
            (part.row, part.price.to_string(), part.profit.to_string()),
            (3, "$76.50".to_owned(), "$34.00".to_owned())
        );
        assert_eq!(result.blended_margin.unwrap().to_string(), "43.662%");
        assert!(Form::was_sent(&example()));
    }
}
