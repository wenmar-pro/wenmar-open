//! What the Canadian invoice page shows for an invoice that was worked
//! out, and what it says of a province's rates.
//!
//! Every figure comes from the rates file or from `shop-math` and is
//! written with its `Display`. The same table and sentences are the worked
//! example of the Markdown version, so the two cannot disagree.

use crate::site::markdown::Table;
use crate::site::tools::canada_invoice_tax::form::Worked;
use crate::site::tools::canada_invoice_tax::rates::{CURRENCY, Province, TaxRule};

/// The words of a list, joined as a sentence joins them.
fn listed(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [most @ .., last] => format!("{} and {last}", most.join(", ")),
    }
}

/// One line of the result: what it is, and its amount.
fn line(name: impl Into<String>, amount: impl ToString) -> Vec<String> {
    vec![name.into(), amount.to_string()]
}

/// The invoice, line by line. A tax's line says its rate and the amount it
/// was calculated on. A line that does not apply is not there: a shop
/// supplies charge of nothing, and a tire class with no tires.
pub fn table(worked: &Worked) -> Table {
    let province = worked.province;
    let result = &worked.result;
    let mut rows = vec![line("Labour", worked.labour), line("Parts", worked.parts)];
    if worked.supplies.cents() != 0 {
        rows.push(line("Shop supplies", worked.supplies));
    }
    rows.push(line("Subtotal", result.subtotal));
    for (class, fee) in province.tire_classes.iter().zip(&result.fees) {
        if fee.count > 0 {
            rows.push(line(
                format!("Tire fee, {}: {} at {}", class.label, fee.count, fee.fee),
                fee.amount,
            ));
        }
    }
    for (rule, tax) in province.taxes.iter().zip(&result.taxes) {
        rows.push(line(
            format!("{} {} on {}", rule.name, tax.rate, tax.base),
            tax.amount,
        ));
    }
    rows.push(line(format!("Total ({CURRENCY})"), result.total));
    Table {
        caption: format!(
            "Invoice in {}, in Canadian dollars ({CURRENCY})",
            province.name
        ),
        head: ["Line", "Amount"].map(str::to_owned).to_vec(),
        rows,
        prose: false,
    }
}

/// What one tax is charged on, as a sentence.
fn tax_line(province: &Province, rule: &TaxRule) -> String {
    let tax = rule.tax;
    let fees = !province.tire_classes.is_empty();
    let mut on = Vec::new();
    for (applies, name) in [
        (tax.on_labour, "labour"),
        (tax.on_parts, "parts"),
        (tax.on_supplies, "shop supplies"),
        (tax.on_tire_fee && fees, "the tire fee"),
    ] {
        if applies {
            on.push(name);
        }
    }
    let not = if fees && !tax.on_tire_fee {
        " Not on the tire fee."
    } else {
        ""
    };
    format!("{} {}: on {}.{not}", rule.name, tax.rate, listed(&on))
}

/// The rates held for a province, one sentence for each tax and each tire
/// class. This is what the calculator uses, and all of it.
pub fn rate_lines(province: &Province) -> Vec<String> {
    let mut lines: Vec<String> = province
        .taxes
        .iter()
        .map(|rule| tax_line(province, rule))
        .collect();
    for class in province.tire_classes {
        let since = class
            .effective
            .map_or_else(String::new, |day| format!(", since {day}"));
        lines.push(format!(
            "Tire fee, {}: {} a tire{since}.",
            class.label, class.fee
        ));
    }
    lines.extend(province.no_tire_fee.map(|none| none.statement.to_owned()));
    lines
}

/// When a province's rates were checked.
pub fn checked(province: &Province) -> String {
    format!("Rates for {} as of {}.", province.name, province.as_of)
}

/// How the page works its figures out, in words and symbols. The page and
/// its Markdown version both show these lines.
pub fn formulas() -> Vec<String> {
    [
        "Subtotal = labour + parts + shop supplies.",
        "A tire fee line = the fee on one new tire of that class \u{d7} the number of tires. The fee is the one the province's own program sets.",
        "Each tax = its rate \u{d7} the amount it is calculated on. That amount is labour, parts, shop supplies and the tire fees, each only where the province charges that tax on it, and it is shown in the tax's line.",
        "No tax is calculated on another tax. Where a province has two, each is calculated on the price before either.",
        "Each tax is rounded once, on its whole amount, to the nearest cent, and a half cent goes up. Total = subtotal + the tire fee lines + the taxes, as they are shown.",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// What the calculator covers and what it does not.
pub fn scope() -> Vec<String> {
    vec![
        format!(
            "This covers a retail repair invoice to a consumer, in Canadian dollars ({CURRENCY}), from a shop that is registered to collect each tax."
        ),
        "It does not cover a customer who is exempt from a tax, or a vehicle that is being repaired for resale.".to_owned(),
        "It does not cover fees other than the fee on a new tire, such as those on oil, oil filters, antifreeze and batteries.".to_owned(),
        "It does not cover tire classes other than the ones listed for the province, such as agricultural, off-road and other commercial tires.".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tools::canada_invoice_tax::form::{Form, example};
    use crate::site::tools::canada_invoice_tax::rates::{PROVINCES, province};
    use crate::site::tools::field::Sent;

    fn rows_for(pairs: &[(&str, &str)]) -> Vec<String> {
        let (form, worked) = Form::read(&Sent::of(pairs));
        assert!(form.troubles().is_empty(), "{:?}", form.troubles());
        let table = table(&worked.unwrap());
        table.rows.iter().map(|row| row.join(" | ")).collect()
    }

    #[test]
    fn the_example_is_shown_line_by_line_with_its_total_in_cad() {
        let (_, worked) = Form::read(&example());
        let table = table(&worked.unwrap());
        assert_eq!(
            table.caption,
            "Invoice in British Columbia, in Canadian dollars (CAD)"
        );
        assert_eq!(table.head, ["Line", "Amount"]);
        let rows: Vec<String> = table.rows.iter().map(|row| row.join(" | ")).collect();
        assert_eq!(
            rows,
            [
                "Labour | $180.00",
                "Parts | $640.00",
                "Shop supplies | $14.37",
                "Subtotal | $834.37",
                "Tire fee, passenger and light truck: 4 at $6.50 | $26.00",
                "GST 5% on $860.37 | $43.02",
                "PST 7% on $860.37 | $60.23",
                "Total (CAD) | $963.62",
            ]
        );
    }

    #[test]
    fn each_tax_line_shows_its_rate_and_what_it_was_calculated_on() {
        // Saskatchewan: one tax is on the tire fee and the other is not,
        // so the two are calculated on different amounts.
        let rows = rows_for(&[
            ("province", "SK"),
            ("labour", "180"),
            ("parts", "640"),
            ("supplies", "14.37"),
            ("tires_passenger_light_truck", "4"),
            ("tires_medium_truck", "1"),
        ]);
        assert_eq!(
            rows,
            [
                "Labour | $180.00",
                "Parts | $640.00",
                "Shop supplies | $14.37",
                "Subtotal | $834.37",
                "Tire fee, passenger and light truck: 4 at $6.50 | $26.00",
                "Tire fee, medium truck: 1 at $14.00 | $14.00",
                "GST 5% on $874.37 | $43.72",
                "PST 6% on $834.37 | $50.06",
                "Total (CAD) | $968.15",
            ]
        );
        // Quebec: a rate with three decimals is shown whole.
        let rows = rows_for(&[
            ("province", "QC"),
            ("labour", "180"),
            ("parts", "640"),
            ("supplies", "14.37"),
            ("tires_diameter_83_82_cm_or_less", "4"),
            ("tires_diameter_over_83_82_cm_to_123_19_cm", "1"),
        ]);
        assert_eq!(
            rows[4..],
            [
                "Tire fee, overall diameter of 83.82 cm (33 in) or less: 4 at $4.50 | $18.00",
                "Tire fee, overall diameter over 83.82 cm (33 in), up to 123.19 cm (48.5 in): 1 at $6.00 | $6.00",
                "GST 5% on $858.37 | $42.92",
                "QST 9.975% on $858.37 | $85.62",
                "Total (CAD) | $986.91",
            ]
        );
    }

    #[test]
    fn only_the_lines_that_apply_are_shown() {
        // Ontario has no tire fee; no shop supplies were charged.
        let rows = rows_for(&[("province", "ON"), ("labour", "180"), ("parts", "640")]);
        assert_eq!(
            rows,
            [
                "Labour | $180.00",
                "Parts | $640.00",
                "Subtotal | $820.00",
                "HST 13% on $820.00 | $106.60",
                "Total (CAD) | $926.60",
            ]
        );
        // Tires only, in one class of two: the other class has no line.
        let rows = rows_for(&[
            ("province", "NL"),
            ("labour", "0"),
            ("parts", "0"),
            ("tires_rim_over_17_in_to_24_5_in", "2"),
        ]);
        assert_eq!(
            rows,
            [
                "Labour | $0.00",
                "Parts | $0.00",
                "Subtotal | $0.00",
                "Tire fee, rim over 17 inches, up to 24.5 inches: 2 at $9.00 | $18.00",
                "HST 15% on $18.00 | $2.70",
                "Total (CAD) | $20.70",
            ]
        );
    }

    #[test]
    fn the_total_is_the_sum_of_the_amounts_shown_above_it() {
        for province in PROVINCES.iter().filter(|province| province.confirmed) {
            let mut pairs = vec![
                ("province".to_owned(), province.code.to_owned()),
                ("labour".to_owned(), "180.00".to_owned()),
                ("parts".to_owned(), "640.00".to_owned()),
                ("supplies".to_owned(), "14.37".to_owned()),
            ];
            for class in province.tire_classes {
                pairs.push((class.field(), "3".to_owned()));
            }
            let (_, worked) = Form::read(&Sent::new(pairs));
            let table = table(&worked.unwrap());
            let cents = |row: &Vec<String>| shop_math::Money::parse(&row[1]).unwrap().cents();
            let mut sum = 0;
            let mut total = 0;
            for row in &table.rows {
                match row[0].as_str() {
                    // The three amounts above the subtotal are in it.
                    "Labour" | "Parts" | "Shop supplies" => {}
                    "Total (CAD)" => total = cents(row),
                    _ => sum += cents(row),
                }
            }
            assert_eq!(sum, total, "{}", province.code);
            // One line for each tax, each with what it was calculated on.
            let taxes = table
                .rows
                .iter()
                .filter(|row| row[0].contains("% on $"))
                .count();
            assert_eq!(taxes, province.taxes.len(), "{}", province.code);
        }
    }

    #[test]
    fn the_rates_of_a_province_are_stated_with_what_each_tax_is_charged_on() {
        assert_eq!(
            rate_lines(province("SK").unwrap()),
            [
                "GST 5%: on labour, parts, shop supplies and the tire fee.",
                "PST 6%: on labour, parts and shop supplies. Not on the tire fee.",
                "Tire fee, passenger and light truck: $6.50 a tire, since 2025-12-01.",
                "Tire fee, medium truck: $14.00 a tire.",
            ]
        );
        // A province with no fee says so in the words of the rates file.
        let ontario = province("ON").unwrap();
        let lines = rate_lines(ontario);
        assert_eq!(lines[0], "HST 13%: on labour, parts and shop supplies.");
        assert_eq!(lines[1], ontario.no_tire_fee.unwrap().statement);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            checked(ontario),
            format!("Rates for Ontario as of {}.", ontario.as_of)
        );
        for province in &PROVINCES {
            let lines = rate_lines(province);
            assert!(lines.len() >= province.taxes.len() + province.tire_classes.len());
        }
        assert_eq!(listed(&[]), "");
        assert_eq!(listed(&["parts"]), "parts");
        assert_eq!(listed(&["labour", "parts"]), "labour and parts");
    }

    #[test]
    fn the_formulas_and_the_scope_are_stated() {
        let lines = formulas();
        assert_eq!(lines.len(), 5);
        let text = lines.join("\n");
        for formula in [
            "Subtotal = labour + parts + shop supplies.",
            "the fee on one new tire of that class \u{d7} the number of tires",
            "Each tax = its rate \u{d7} the amount it is calculated on.",
            "No tax is calculated on another tax.",
            "a half cent goes up",
            "Total = subtotal + the tire fee lines + the taxes",
        ] {
            assert!(text.contains(formula), "{formula}");
        }
        // Spec section 8.3: what is covered, and each thing that is not.
        let scope = scope().join("\n");
        for limit in [
            "a retail repair invoice to a consumer",
            "Canadian dollars (CAD)",
            "exempt from a tax",
            "resale",
            "oil, oil filters, antifreeze and batteries",
            "commercial tires",
        ] {
            assert!(scope.contains(limit), "{limit}");
        }
    }
}
