//! The one adapter between Wenmar Open and the `payroll-ca` engine: it
//! reads a pay from text and fills the documented defaults. It writes no
//! rate and no amount: every figure is the engine's. Ontario only.

pub mod factors;
mod read;
pub use read::{Claim, FIELDS, Input, Query, Refusal, Worked, deductions, money, read, work};

use chrono::NaiveDate;
pub use payroll_ca;
use payroll_ca::rates;

/// How often an employee is paid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Frequency {
    Weekly,
    Biweekly,
    SemiMonthly,
    Monthly,
}

impl Frequency {
    pub const ALL: [Frequency; 4] = [
        Frequency::Weekly,
        Frequency::Biweekly,
        Frequency::SemiMonthly,
        Frequency::Monthly,
    ];

    /// The name in a query string and in an address: `weekly`, `biweekly`,
    /// `semi-monthly`, `monthly`.
    pub fn slug(self) -> &'static str {
        match self {
            Frequency::Weekly => "weekly",
            Frequency::Biweekly => "biweekly",
            Frequency::SemiMonthly => "semi-monthly",
            Frequency::Monthly => "monthly",
        }
    }

    /// The name in a list: `Weekly`, `Biweekly`, `Semi-monthly`, `Monthly`.
    pub fn label(self) -> &'static str {
        match self {
            Frequency::Weekly => "Weekly",
            Frequency::Biweekly => "Biweekly",
            Frequency::SemiMonthly => "Semi-monthly",
            Frequency::Monthly => "Monthly",
        }
    }

    /// Pay periods in a year (T4127's P): 52, 26, 24, 12.
    pub fn periods(self) -> u32 {
        match self {
            Frequency::Weekly => 52,
            Frequency::Biweekly => 26,
            Frequency::SemiMonthly => 24,
            Frequency::Monthly => 12,
        }
    }

    /// Reads a slug, in any case, with spaces around it.
    pub fn parse(text: &str) -> Option<Frequency> {
        let text = text.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|frequency| frequency.slug().eq_ignore_ascii_case(text))
    }
}

/// What every answer says. The page links the middle part to [`PDOC_URL`].
pub const NOTE: &str = "An estimate. The employer remains liable to CRA for the amounts it deducts and remits. Check a result with CRA's Payroll Deductions Online Calculator (PDOC) before relying on it.";
/// CRA's page for its Payroll Deductions Online Calculator.
pub const PDOC_URL: &str = "https://www.canada.ca/en/revenue-agency/services/e-services/digital-services-businesses/payroll-deductions-online-calculator.html";
/// Where `payroll-ca` shows the PDOC cases its results were compared with, and how to compare
/// one yourself (its README section "Check a result yourself").
pub const CHECK_URL: &str = "https://github.com/wenmar-pro/payroll-ca#check-a-result-yourself";
/// The words of NOTE that are the link.
const PDOC_LINK: &str = "Payroll Deductions Online Calculator (PDOC)";

/// NOTE as the text before the link, the link's text and the text after it.
pub fn note_parts() -> (&'static str, &'static str, &'static str) {
    let (before, after) = NOTE.split_once(PDOC_LINK).expect("NOTE holds its link");
    (before, PDOC_LINK, after)
}

/// Where CRA publishes each edition of T4127. An edition missing here fails a test.
const T4127_URLS: [(u32, &str); 1] = [(
    123,
    "https://www.canada.ca/content/dam/cra-arc/formspubs/pub/t4127-jul/t4127-jul-26e.pdf",
)];

/// One edition of T4127 the engine holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edition {
    pub number: u32,
    pub effective: NaiveDate,
    pub valid_until: NaiveDate,
    pub url: Option<&'static str>,
}

impl Edition {
    pub fn of(set: &rates::RateSet) -> Edition {
        Edition {
            number: set.edition,
            effective: set.effective,
            valid_until: set.valid_until,
            url: T4127_URLS
                .iter()
                .find(|(number, _)| *number == set.edition)
                .map(|(_, url)| *url),
        }
    }

    /// "T4127 Payroll Deductions Formulas, 123rd edition"
    pub fn title(&self) -> String {
        format!(
            "T4127 Payroll Deductions Formulas, {} edition",
            ordinal(self.number)
        )
    }

    /// "for pay dates from 2026-07-01 to 2026-12-31"
    pub fn span(&self) -> String {
        format!(
            "for pay dates from {} to {}",
            self.effective, self.valid_until
        )
    }
}

/// Every edition held, oldest first.
pub fn editions() -> Vec<Edition> {
    rates::editions().iter().map(Edition::of).collect()
}

/// 1st, 2nd, 3rd, 4th, 11th, 12th, 13th, 21st, 123rd.
pub fn ordinal(number: u32) -> String {
    let suffix = match number % 100 {
        11..=13 => "th",
        _ => match number % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{number}{suffix}")
}

/// Today in Toronto (`America/Toronto`): Ontario's date, not UTC's, so an
/// evening visit does not get tomorrow. Only the default pay date reads the
/// clock.
pub fn today() -> NaiveDate {
    chrono::Utc::now()
        .with_timezone(&chrono_tz::America::Toronto)
        .date_naive()
}

/// The pay date used when none is given: `today` when an edition covers
/// it; otherwise the nearest day an edition covers (the first edition's
/// first day before them all, else the last day of the newest edition
/// that ended before `today`). `true` when the day was moved.
pub fn default_pay_date(today: NaiveDate) -> (NaiveDate, bool) {
    let held = editions();
    if held
        .iter()
        .any(|edition| edition.effective <= today && today <= edition.valid_until)
    {
        return (today, false);
    }
    if today < held[0].effective {
        (held[0].effective, true)
    } else {
        (held[held.len() - 1].valid_until, true)
    }
}

#[cfg(test)]
mod tests {
    /// A rate, a maximum or a basic amount is the engine's, written in its
    /// tables. No file of the adapter writes one of its own.
    #[test]
    fn no_adapter_code_writes_a_payroll_figure_of_its_own() {
        let files = [
            ("lib.rs", include_str!("lib.rs")),
            ("read.rs", include_str!("read.rs")),
            ("factors.rs", include_str!("factors.rs")),
        ];
        for (name, text) in files {
            let code = text.split("#[cfg(test)]").next().unwrap();
            // The T4127 address names its edition in a file name.
            let code = code
                .lines()
                .filter(|line| !line.contains("t4127-jul"))
                .collect::<Vec<_>>()
                .join("\n");
            let bytes = code.as_bytes();
            for (index, pair) in bytes.windows(2).enumerate() {
                let rate = pair[0].is_ascii_digit() && pair[1] == b'%';
                let from = index.saturating_sub(30);
                assert!(
                    !rate,
                    "{name} writes a percentage: {}",
                    String::from_utf8_lossy(&bytes[from..index + 2])
                );
            }
            for word in [
                "16452", "12989", "16,452", "12,989", "0.0595", "0.0163", "1.4", "3500", "68900",
            ] {
                assert!(!code.contains(word), "{name} writes {word}");
            }
        }
    }
}
