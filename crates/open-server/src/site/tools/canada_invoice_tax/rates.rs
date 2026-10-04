//! The rates of `data/rates/canada.toml`, as the build compiled them in.
//!
//! The build script reads the file, checks it and writes it out as the
//! list [`PROVINCES`] (see `rates_build.rs` beside this crate's
//! `build.rs`). Nothing is read when the server runs, and nothing here can
//! fail.

use shop_math::invoice_tax::{Invoice, InvoiceTax, InvoiceTaxError, Tax, Tires, invoice_tax};
use shop_math::{Money, Percent};

/// A page a figure was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// Whose page it is, and what it is.
    pub title: &'static str,
    pub url: &'static str,
    /// The date of the Internet Archive's copy, when that was what was read.
    pub archived: Option<&'static str>,
}

/// One tax of a province, with where its rate and its rules are stated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxRule {
    pub name: &'static str,
    pub tax: Tax,
    pub sources: &'static [Source],
}

/// One class of new tire, as the province's own body names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TireClass {
    /// The class in the name of its field: `tires_<key>`.
    pub key: &'static str,
    /// What a shop would call the class, to follow "New tires:".
    pub label: &'static str,
    /// The fee on one tire.
    pub fee: Money,
    /// The day the fee took effect, when a source gives it.
    pub effective: Option<&'static str>,
    pub sources: &'static [Source],
}

impl TireClass {
    /// The name of the class's field in the form and in the address.
    pub fn field(&self) -> String {
        format!("tires_{}", self.key)
    }
}

/// That a province sets no fee on a new tire: a fact with a source, not a
/// gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoTireFee {
    pub statement: &'static str,
    pub sources: &'static [Source],
}

/// One province or territory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Province {
    pub code: &'static str,
    pub name: &'static str,
    /// The day the entry was checked against its sources.
    pub as_of: &'static str,
    /// Whether every rule the calculator uses for the province has a
    /// source. Nothing is worked out for a province that is not confirmed.
    pub confirmed: bool,
    /// What is not yet known, for a province that is not confirmed.
    pub missing: &'static [&'static str],
    /// Sentences the page shows with the result.
    pub notes: &'static [&'static str],
    pub taxes: &'static [TaxRule],
    pub tire_classes: &'static [TireClass],
    pub no_tire_fee: Option<NoTireFee>,
}

include!(concat!(env!("OUT_DIR"), "/canada_rates.rs"));

impl Province {
    /// Every page this province's figures were read from, each once, in
    /// the order they are first used.
    pub fn sources(&self) -> Vec<&'static Source> {
        let of_taxes = self.taxes.iter().flat_map(|tax| tax.sources);
        let of_classes = self.tire_classes.iter().flat_map(|class| class.sources);
        let of_none = self.no_tire_fee.iter().flat_map(|none| none.sources);
        let mut sources: Vec<&'static Source> = Vec::new();
        for source in of_taxes.chain(of_classes).chain(of_none) {
            if !sources.iter().any(|seen| seen.url == source.url) {
                sources.push(source);
            }
        }
        sources
    }

    /// The fees and the taxes on an invoice in this province. `counts` is
    /// the number of new tires in each class, in the order of
    /// [`Province::tire_classes`]; a class with no count has no tires.
    /// `None` for a province that is not confirmed: nothing is worked out
    /// from a rule without a source.
    pub fn invoice(
        &self,
        labour: Money,
        parts: Money,
        supplies: Money,
        counts: &[u32],
    ) -> Option<Result<InvoiceTax, InvoiceTaxError>> {
        if !self.confirmed {
            return None;
        }
        let taxes: Vec<Tax> = self.taxes.iter().map(|rule| rule.tax).collect();
        let tires: Vec<Tires> = self
            .tire_classes
            .iter()
            .enumerate()
            .map(|(index, class)| Tires {
                fee: class.fee,
                count: counts.get(index).copied().unwrap_or(0),
            })
            .collect();
        let invoice = Invoice {
            labour,
            parts,
            supplies,
            tires: &tires,
        };
        Some(invoice_tax(&taxes, &invoice))
    }
}

/// The province with this code, whatever the case of its letters.
pub fn province(code: &str) -> Option<&'static Province> {
    let code = code.trim();
    PROVINCES
        .iter()
        .find(|province| province.code.eq_ignore_ascii_case(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cents(cents: i64) -> Money {
        Money::from_cents(cents)
    }

    /// Spec section 10: every tax and fee in the file has a source address.
    #[test]
    fn every_tax_and_every_fee_has_a_source_with_an_address() {
        let mut checked = 0;
        for province in &PROVINCES {
            assert!(!province.taxes.is_empty(), "{}", province.code);
            let of_taxes = province.taxes.iter().map(|tax| (tax.name, tax.sources));
            let of_classes = province
                .tire_classes
                .iter()
                .map(|class| (class.key, class.sources));
            let of_none = province
                .no_tire_fee
                .iter()
                .map(|none| ("no tire fee", none.sources));
            for (name, sources) in of_taxes.chain(of_classes).chain(of_none) {
                assert!(!sources.is_empty(), "{} {name}", province.code);
                for source in sources {
                    assert!(
                        source.url.starts_with("https://"),
                        "{} {name}: {}",
                        province.code,
                        source.url
                    );
                    assert!(!source.title.is_empty(), "{} {name}", province.code);
                }
                checked += 1;
            }
            assert!(!province.sources().is_empty());
        }
        assert!(checked >= PROVINCES.len());
        assert_eq!(CURRENCY, "CAD");
    }

    #[test]
    fn a_confirmed_province_has_its_tire_fees_or_says_it_has_none() {
        for province in &PROVINCES {
            let settled = !province.tire_classes.is_empty() || province.no_tire_fee.is_some();
            if province.confirmed {
                assert!(settled, "{}", province.code);
                assert!(province.missing.is_empty(), "{}", province.code);
            } else {
                assert!(!province.missing.is_empty(), "{}", province.code);
            }
            assert!(!(province.no_tire_fee.is_some() && !province.tire_classes.is_empty()));
            // The fields of a province's classes have names of their own.
            for (index, class) in province.tire_classes.iter().enumerate() {
                assert!(class.field().starts_with("tires_"));
                for other in &province.tire_classes[index + 1..] {
                    assert_ne!(class.key, other.key, "{}", province.code);
                }
            }
        }
        // A list in which nothing is confirmed would be a page that
        // works nothing out.
        assert!(PROVINCES.iter().any(|province| province.confirmed));
    }

    #[test]
    fn a_province_is_found_by_its_code_in_either_case() {
        for province in &PROVINCES {
            let found = super::province(province.code).unwrap();
            assert_eq!(found.name, province.name);
            let small = province.code.to_ascii_lowercase();
            assert_eq!(
                super::province(&format!(" {small} ")).unwrap().code,
                province.code
            );
        }
        for unknown in ["", "ZZ", "B", "BCX", "British Columbia", "<b>"] {
            assert!(super::province(unknown).is_none(), "{unknown}");
        }
    }

    /// A province's code, its tire fee lines, each tax's base and amount,
    /// and the total, in cents.
    type Worked = (&'static str, &'static [i64], &'static [(i64, i64)], i64);

    /// One invoice, worked by hand for every confirmed province: labour
    /// 180.00, parts 640.00, shop supplies 14.37, four new tires of the
    /// province's first class and one of its second. Before fees and taxes
    /// that is 834.37. The figures on the right of each line below were
    /// worked with a calculator from the sources' rates, not by this code.
    ///
    /// | Province | Tire fees | Each tax, on its base | Total |
    /// |---|---|---|---|
    /// | BC | 26.00 + 14.00 | 43.72 and 61.21 on 874.37 | 979.30 |
    /// | AB | 20.00 + 14.00 | 43.42 on 868.37 | 911.79 |
    /// | SK | 26.00 + 14.00 | 43.72 on 874.37; 50.06 on 834.37 | 968.15 |
    /// | MB | 20.00 + 14.00 | 43.42 and 60.79 on 868.37 | 972.58 |
    /// | ON | none | 108.47 on 834.37 | 942.84 |
    /// | QC | 18.00 + 6.00 | 42.92 and 85.62 on 858.37 | 986.91 |
    /// | NB | 18.00 + 13.50 | 129.88 on 865.87 | 995.75 |
    /// | NS | 18.00 + 13.50 | 121.22 on 865.87 | 987.09 |
    /// | PE | 16.00 + 11.25 | 129.24 on 861.62 | 990.86 |
    /// | NL | 12.00 + 9.00 | 128.31 on 855.37 | 983.68 |
    /// | YT | 28.00 + 9.00 | 43.57 on 871.37 | 914.94 |
    const WORKED: [Worked; 11] = [
        (
            "BC",
            &[2_600, 1_400],
            &[(87_437, 4_372), (87_437, 6_121)],
            97_930,
        ),
        ("AB", &[2_000, 1_400], &[(86_837, 4_342)], 91_179),
        (
            "SK",
            &[2_600, 1_400],
            &[(87_437, 4_372), (83_437, 5_006)],
            96_815,
        ),
        (
            "MB",
            &[2_000, 1_400],
            &[(86_837, 4_342), (86_837, 6_079)],
            97_258,
        ),
        ("ON", &[], &[(83_437, 10_847)], 94_284),
        (
            "QC",
            &[1_800, 600],
            &[(85_837, 4_292), (85_837, 8_562)],
            98_691,
        ),
        ("NB", &[1_800, 1_350], &[(86_587, 12_988)], 99_575),
        ("NS", &[1_800, 1_350], &[(86_587, 12_122)], 98_709),
        ("PE", &[1_600, 1_125], &[(86_162, 12_924)], 99_086),
        ("NL", &[1_200, 900], &[(85_537, 12_831)], 98_368),
        ("YT", &[2_800, 900], &[(87_137, 4_357)], 91_494),
    ];

    #[test]
    fn a_worked_invoice_for_each_confirmed_province() {
        for (code, fees, taxes, total) in WORKED {
            let province = super::province(code).unwrap();
            assert!(province.confirmed, "{code}");
            let result = province
                .invoice(cents(18_000), cents(64_000), cents(1_437), &[4, 1])
                .unwrap()
                .unwrap();
            assert_eq!(result.subtotal, cents(83_437), "{code}");
            let amounts: Vec<i64> = result.fees.iter().map(|fee| fee.amount.cents()).collect();
            assert_eq!(amounts, fees, "{code}: the tire fees");
            let lines: Vec<(i64, i64)> = result
                .taxes
                .iter()
                .map(|tax| (tax.base.cents(), tax.amount.cents()))
                .collect();
            assert_eq!(lines, taxes, "{code}: each tax's base and amount");
            assert_eq!(result.total, cents(total), "{code}: the total");
        }
        // Every confirmed province has its worked invoice: one that is
        // confirmed later needs a line above.
        for province in PROVINCES.iter().filter(|province| province.confirmed) {
            assert!(
                WORKED.iter().any(|(code, ..)| *code == province.code),
                "{} is confirmed and has no worked invoice",
                province.code
            );
        }
    }

    /// The example in British Columbia's own small business guide: a sale
    /// of 100.00 carries 7.00 of PST and 5.00 of GST.
    #[test]
    fn the_example_british_columbia_gives_comes_out_as_it_gives_it() {
        let province = super::province("BC").unwrap();
        let result = province
            .invoice(Money::ZERO, cents(10_000), Money::ZERO, &[])
            .unwrap()
            .unwrap();
        let by_name: Vec<(&str, i64)> = province
            .taxes
            .iter()
            .zip(&result.taxes)
            .map(|(rule, line)| (rule.name, line.amount.cents()))
            .collect();
        assert_eq!(by_name, [("GST", 500), ("PST", 700)]);
        assert_eq!(result.total, cents(11_200));
    }

    #[test]
    fn a_province_that_is_not_confirmed_works_nothing_out() {
        let mut unconfirmed = 0;
        for province in PROVINCES.iter().filter(|province| !province.confirmed) {
            assert!(
                province
                    .invoice(cents(18_000), cents(64_000), cents(1_437), &[4, 1])
                    .is_none(),
                "{}",
                province.code
            );
            unconfirmed += 1;
        }
        // The same rule holds for a province made up here, whatever the
        // file holds today.
        let made_up = Province {
            confirmed: false,
            missing: &["Whether a fee exists."],
            ..PROVINCES[0]
        };
        assert!(
            made_up
                .invoice(cents(100), Money::ZERO, Money::ZERO, &[])
                .is_none()
        );
        assert!(PROVINCES[0].confirmed || unconfirmed > 0);
    }

    #[test]
    fn a_count_for_a_class_the_province_does_not_have_is_left_out() {
        // More counts than classes, and fewer: neither can reach past the
        // classes the province has.
        let province = PROVINCES
            .iter()
            .find(|province| province.confirmed && !province.tire_classes.is_empty())
            .unwrap();
        let many = province
            .invoice(cents(100), Money::ZERO, Money::ZERO, &[1, 1, 1, 1, 1, 1])
            .unwrap()
            .unwrap();
        assert_eq!(many.fees.len(), province.tire_classes.len());
        let none = province
            .invoice(cents(100), Money::ZERO, Money::ZERO, &[])
            .unwrap()
            .unwrap();
        assert!(none.fees.iter().all(|fee| fee.count == 0));
        assert_eq!(none.fees.len(), province.tire_classes.len());
    }
}
