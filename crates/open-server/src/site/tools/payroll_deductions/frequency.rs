//! The one page per pay frequency: `/tools/payroll-deductions/ontario/…`.
//!
//! Each is the payroll calculator with a landing of its own: its own title,
//! address, heading, intro, worked example and a form preset to that
//! frequency, and links to the other three frequencies. They are not
//! `Tool`s: they are landings for the one payroll calculator, [`super`].

use payroll_calc::Frequency;

use crate::site::Page;
use crate::site::markdown::Doc;
use crate::site::seo;
use crate::site::tools::field::Sent;
use crate::state::AppState;

/// One page of the one per pay frequency.
pub struct FrequencyPage {
    /// The frequency this page is for.
    pub frequency: Frequency,
    /// The page's heading, as the `<h1>` and the base of its title.
    pub heading: &'static str,
    /// The description a search result shows.
    pub description: &'static str,
    /// The page's intro, under the heading.
    pub intro: &'static str,
}

/// The four frequency pages, in the order of [`Frequency::ALL`].
pub const PAGES: [FrequencyPage; 4] = [
    FrequencyPage {
        frequency: Frequency::Weekly,
        heading: "Ontario weekly payroll deductions 2026",
        description: "Works out the federal and Ontario income tax, CPP, CPP2 and EI on a weekly pay, the net pay and what the employer pays on top, by CRA's T4127 formulas.",
        intro: "The payroll deductions on one weekly pay in Ontario: the federal and Ontario income tax, CPP, CPP2 and EI, the net pay, what the employer pays on top, and every T4127 factor used.",
    },
    FrequencyPage {
        frequency: Frequency::Biweekly,
        heading: "Ontario biweekly payroll deductions 2026",
        description: "Works out the federal and Ontario income tax, CPP, CPP2 and EI on a biweekly pay, the net pay and what the employer pays on top, by CRA's T4127 formulas.",
        intro: "The payroll deductions on one biweekly pay in Ontario: the federal and Ontario income tax, CPP, CPP2 and EI, the net pay, what the employer pays on top, and every T4127 factor used.",
    },
    FrequencyPage {
        frequency: Frequency::SemiMonthly,
        heading: "Ontario semi-monthly payroll deductions 2026",
        description: "Works out the federal and Ontario income tax, CPP, CPP2 and EI on a semi-monthly pay, the net pay and what the employer pays on top, by CRA's T4127 formulas.",
        intro: "The payroll deductions on one semi-monthly pay in Ontario: the federal and Ontario income tax, CPP, CPP2 and EI, the net pay, what the employer pays on top, and every T4127 factor used.",
    },
    FrequencyPage {
        frequency: Frequency::Monthly,
        heading: "Ontario monthly payroll deductions 2026",
        description: "Works out the federal and Ontario income tax, CPP, CPP2 and EI on a monthly pay, the net pay and what the employer pays on top, by CRA's T4127 formulas.",
        intro: "The payroll deductions on one monthly pay in Ontario: the federal and Ontario income tax, CPP, CPP2 and EI, the net pay, what the employer pays on top, and every T4127 factor used.",
    },
];

impl FrequencyPage {
    /// The page's address: `/tools/payroll-deductions/ontario/<slug>`.
    pub fn path(&self) -> String {
        format!(
            "/tools/payroll-deductions/ontario/{}",
            self.frequency.slug()
        )
    }

    /// The page the visitor asked for, by the last part of its address.
    pub fn find(slug: &str) -> Option<&'static FrequencyPage> {
        PAGES
            .iter()
            .find(|page| page.frequency.slug().eq_ignore_ascii_case(slug))
    }

    /// The layout's part of the page, already titled and addressed.
    pub fn page(&self, state: &AppState) -> Page {
        Page::new(state, seo::title(self.heading), self.description)
            .without_vehicle_data()
            .with_markdown(&format!("{}.md", self.path()))
            .in_section("tools")
            .indexed(state, &self.path())
            .under(vec![
                ("Tools".to_owned(), "/tools".to_owned()),
                (super::TITLE.to_owned(), super::PATH.to_owned()),
            ])
    }

    /// What the page shows in place of the calculator's own landing.
    pub fn landing(&self) -> Landing {
        Landing {
            heading: self.heading.to_owned(),
            intro: self.intro.to_owned(),
            form_action: self.path(),
            freq_links: PAGES
                .iter()
                .filter(|page| page.frequency != self.frequency)
                .map(|page| (page.frequency.label().to_owned(), page.path()))
                .collect(),
            example: Sent::of(&[("frequency", self.frequency.slug()), ("gross", "2400.00")]),
        }
    }

    /// The Markdown version of the page, on the public address `base`.
    pub fn doc(&self, base: &str) -> Doc {
        super::doc::doc_for(
            base,
            self.heading,
            self.description,
            &self.path(),
            self.frequency.slug(),
        )
    }
}

/// What a page of the payroll calculator shows in place of the default
/// landing: the heading, the intro, where the form posts, the links to the
/// other frequency pages, and the pay the example is preset to.
pub struct Landing {
    pub heading: String,
    pub intro: String,
    pub form_action: String,
    pub freq_links: Vec<(String, String)>,
    pub example: Sent,
}

impl Landing {
    /// The calculator's own landing: its title, summary and address, with
    /// links to all four frequency pages.
    pub fn main() -> Landing {
        Landing {
            heading: super::TITLE.to_owned(),
            intro: super::SUMMARY.to_owned(),
            form_action: super::PATH.to_owned(),
            freq_links: PAGES
                .iter()
                .map(|page| (page.frequency.label().to_owned(), page.path()))
                .collect(),
            example: super::form::example(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_pages_are_the_four_frequencies_in_order() {
        assert_eq!(
            PAGES
                .iter()
                .map(|page| page.frequency.slug())
                .collect::<Vec<_>>(),
            ["weekly", "biweekly", "semi-monthly", "monthly"]
        );
        for page in &PAGES {
            assert_eq!(
                page.path(),
                format!(
                    "/tools/payroll-deductions/ontario/{}",
                    page.frequency.slug()
                )
            );
            assert!(page.description.ends_with('.'));
            assert!(page.intro.ends_with('.'));
        }
    }

    #[test]
    fn a_page_is_found_by_its_slug_in_any_case() {
        assert_eq!(
            FrequencyPage::find("weekly").unwrap().frequency,
            Frequency::Weekly
        );
        assert_eq!(
            FrequencyPage::find("SEMI-MONTHLY").unwrap().frequency,
            Frequency::SemiMonthly
        );
        assert!(FrequencyPage::find("fortnightly").is_none());
    }

    #[test]
    fn a_landing_presets_its_example_and_links_the_other_three() {
        let biweekly = FrequencyPage::find("biweekly").unwrap();
        let landing = biweekly.landing();
        assert_eq!(landing.heading, "Ontario biweekly payroll deductions 2026");
        assert_eq!(
            landing.form_action,
            "/tools/payroll-deductions/ontario/biweekly"
        );
        assert_eq!(
            landing.freq_links,
            [
                (
                    "Weekly".to_owned(),
                    "/tools/payroll-deductions/ontario/weekly".to_owned()
                ),
                (
                    "Semi-monthly".to_owned(),
                    "/tools/payroll-deductions/ontario/semi-monthly".to_owned()
                ),
                (
                    "Monthly".to_owned(),
                    "/tools/payroll-deductions/ontario/monthly".to_owned()
                ),
            ]
        );
        assert_eq!(landing.example.text("frequency"), "biweekly");
        assert_eq!(landing.example.text("gross"), "2400.00");
    }

    #[test]
    fn the_main_landing_links_all_four() {
        let landing = Landing::main();
        assert_eq!(landing.heading, super::super::TITLE);
        assert_eq!(landing.form_action, super::super::PATH);
        assert_eq!(landing.freq_links.len(), 4);
    }
}
