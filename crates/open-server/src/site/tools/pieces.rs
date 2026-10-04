//! What the shared pieces of a calculator page say, and tests of the
//! template that holds them, `templates/tool.html`.

/// The line every calculator page carries, as HTML and as Markdown.
pub const ADVICE: &str = "This is arithmetic, not accounting or tax advice.";

#[cfg(test)]
mod tests {
    use askama::Template;
    use axum::response::Response;
    use shop_math::Percent;
    use shop_math::targets::{LABOR, PARTS};

    use super::ADVICE;
    use crate::site::Page;
    use crate::site::markdown::{Doc, Table};
    use crate::site::tools::Tool;
    use crate::site::tools::field::{Field, Sent};
    use crate::site::tools::target::TargetView;

    fn no_doc(_base: &str) -> Doc {
        Doc::default()
    }

    fn no_page(_page: Page, _tool: &'static Tool, _sent: &Sent) -> Response {
        Response::default()
    }

    static TOOL: Tool = Tool {
        slug: "sample",
        title: "Sample calculator",
        description: "A calculator that exists only in this test.",
        summary: "Works out a sample, for a test.",
        pro_does: "does this on every estimate.",
        doc: no_doc,
        page: no_page,
    };

    #[derive(Template)]
    #[template(
        source = r#"{% import "tool.html" as piece %}{% call piece::opening(tool) %}{% call piece::field(good) %}{% call piece::field(bad) %}{% call piece::no_result(troubles) %}{% call piece::actions() %}{% call piece::table(table) %}{% call piece::target(parts) %}{% call piece::target(labor) %}{% call piece::how(lines) %}{% call piece::closing(tool) %}"#,
        ext = "html"
    )]
    struct Pieces {
        tool: &'static Tool,
        good: Field,
        bad: Field,
        troubles: Vec<(String, String)>,
        table: Table,
        parts: TargetView,
        labor: TargetView,
        lines: Vec<String>,
    }

    fn rendered() -> String {
        let sent = Sent::of(&[("cost", "42.50"), ("rate", "\"><b>x")]);
        let mut bad = Field::new(&sent, "rate", "Markup");
        bad.required(Percent::parse);
        Pieces {
            tool: &TOOL,
            good: Field::new(&sent, "cost", "Cost"),
            bad,
            troubles: vec![("rate".to_owned(), "Markup: not a number".to_owned())],
            table: Table {
                caption: "Figures".to_owned(),
                head: vec!["Row".to_owned(), "Price".to_owned()],
                rows: vec![vec!["1".to_owned(), "$12.50".to_owned()]],
                prose: true,
            },
            parts: TargetView::new(
                "Blended margin",
                Percent::from_thousandths(43_662),
                "parts gross profit",
                &PARTS,
            ),
            labor: TargetView::new(
                "Labor gross profit",
                Percent::whole(55),
                "labor gross profit",
                &LABOR,
            ),
            lines: vec!["price = cost + profit".to_owned()],
        }
        .render()
        .unwrap()
    }

    #[test]
    fn a_page_opens_with_its_heading_and_closes_with_the_advice_and_one_pro_link() {
        let html = rendered();
        assert!(
            html.contains("<h1>Sample calculator</h1>\n<p>Works out a sample, for a test.</p>")
        );
        assert_eq!(ADVICE, "This is arithmetic, not accounting or tax advice.");
        assert!(html.contains(&format!("<p class=\"note\">{ADVICE}</p>")));
        assert!(html.contains(
            "<p class=\"pro\"><a href=\"https://wenmarpro.com/?utm_source=wenmar-open&#38;utm_medium=referral&#38;utm_campaign=tool-sample\">Wenmar Pro</a> does this on every estimate.</p>"
        ), "{html}");
        assert_eq!(html.matches("wenmarpro.com").count(), 1);
        assert!(
            html.contains(
                "<h2 id=\"how\">How this is worked out</h2>\n<p>price = cost + profit</p>"
            )
        );
        // The order of the parts is the order they were called in.
        let places: Vec<usize> = [
            "<h1>",
            "<div class=\"field\">",
            "<table",
            "id=\"how\"",
            "class=\"note\">This is",
            "class=\"pro\"",
        ]
        .iter()
        .map(|part| html.find(part).unwrap())
        .collect();
        assert!(places.is_sorted(), "{places:?}");
    }

    #[test]
    fn a_field_shows_what_was_typed_and_a_message_only_when_it_could_not_be_read() {
        let html = rendered();
        assert!(html.contains(
            "<label for=\"cost\">Cost</label>\n<input id=\"cost\" name=\"cost\" type=\"text\" inputmode=\"decimal\" autocomplete=\"off\" value=\"42.50\">"
        ), "{html}");
        // What was typed is text, whatever it holds.
        assert!(html.contains(
            "value=\"&#34;&#62;&#60;b&#62;x\" aria-invalid=\"true\" aria-describedby=\"rate-error\">"
        ), "{html}");
        assert!(html.contains(
            "<p class=\"warning\" id=\"rate-error\">This is not a number. Write it like 1,234.50.</p>"
        ));
        assert_eq!(html.matches("class=\"warning\"").count(), 1);
        assert!(html.contains("<li><a href=\"#rate\">Markup: not a number</a></li>"));
        assert!(html.contains("<button type=\"button\" data-print hidden>Print</button>"));
    }

    #[test]
    fn a_table_of_figures_is_marked_up_as_the_guides_tables_are() {
        let html = rendered();
        assert!(html.contains(
            "<div class=\"scroll\">\n<table class=\"prose\">\n<caption>Figures</caption>"
        ));
        assert!(html.contains(
            "<thead><tr><th scope=\"col\">Row</th><th scope=\"col\">Price</th></tr></thead>"
        ));
        assert!(html.contains("<tr><th scope=\"row\">1</th><td>$12.50</td></tr>"));
    }

    #[test]
    fn a_target_is_shown_with_its_range_its_standing_and_a_link_to_each_source() {
        let html = rendered();
        assert!(html.contains(
            "<p class=\"target\"><strong>Blended margin: 43.662%.</strong> That is inside the typical range of 40% to 50% for parts gross profit. The usual target is 50%.</p>"
        ), "{html}");
        assert!(html.contains("That is below the typical range of 60% to 75% for labor gross profit. The usual target is 70%."));
        for target in [&PARTS, &LABOR] {
            for source in target.range_sources.iter().chain(target.usual_sources) {
                assert!(
                    html.contains(&format!("<a href=\"{}\">{}</a>", source.url, source.name)),
                    "{}",
                    source.url
                );
            }
        }
        // Two sources of one figure are joined in words.
        assert!(
            html.contains("</a> and <a href=\"https://partstech.com/"),
            "{html}"
        );
        // What the figures are, and are not.
        assert_eq!(
            html.matches("These figures are for a general repair shop.")
                .count(),
            2
        );
        assert_eq!(
            html.matches("Neither is a survey of what shops earn.")
                .count(),
            2
        );
    }

    /// Spec section 10: each target and each source is written in one place,
    /// the crate `shop-math`. No file of the pages names one.
    #[test]
    fn no_page_or_template_writes_a_target_or_a_source_of_its_own() {
        let files = [
            ("tool.html", include_str!("../../../templates/tool.html")),
            ("target.rs", include_str!("target.rs")),
            ("field.rs", include_str!("field.rs")),
            ("mod.rs", include_str!("mod.rs")),
            (
                "parts_matrix.html",
                include_str!("../../../templates/parts_matrix.html"),
            ),
            ("parts_matrix/mod.rs", include_str!("parts_matrix/mod.rs")),
            ("parts_matrix/form.rs", include_str!("parts_matrix/form.rs")),
            (
                "parts_matrix/presets.rs",
                include_str!("parts_matrix/presets.rs"),
            ),
            (
                "parts_matrix/result.rs",
                include_str!("parts_matrix/result.rs"),
            ),
            ("parts_matrix/doc.rs", include_str!("parts_matrix/doc.rs")),
            (
                "canada_invoice_tax.html",
                include_str!("../../../templates/canada_invoice_tax.html"),
            ),
            (
                "canada_invoice_tax/mod.rs",
                include_str!("canada_invoice_tax/mod.rs"),
            ),
            (
                "canada_invoice_tax/form.rs",
                include_str!("canada_invoice_tax/form.rs"),
            ),
            (
                "canada_invoice_tax/result.rs",
                include_str!("canada_invoice_tax/result.rs"),
            ),
            (
                "canada_invoice_tax/doc.rs",
                include_str!("canada_invoice_tax/doc.rs"),
            ),
            (
                "canada_invoice_tax/rates.rs",
                include_str!("canada_invoice_tax/rates.rs"),
            ),
            (
                "gross_profit.html",
                include_str!("../../../templates/gross_profit.html"),
            ),
            ("gross_profit/mod.rs", include_str!("gross_profit/mod.rs")),
            ("gross_profit/form.rs", include_str!("gross_profit/form.rs")),
            (
                "gross_profit/result.rs",
                include_str!("gross_profit/result.rs"),
            ),
            ("gross_profit/doc.rs", include_str!("gross_profit/doc.rs")),
            (
                "labor_rate.html",
                include_str!("../../../templates/labor_rate.html"),
            ),
            ("labor_rate/mod.rs", include_str!("labor_rate/mod.rs")),
            ("labor_rate/form.rs", include_str!("labor_rate/form.rs")),
            ("labor_rate/result.rs", include_str!("labor_rate/result.rs")),
            ("labor_rate/doc.rs", include_str!("labor_rate/doc.rs")),
        ];
        for (name, text) in files {
            let code = text.split("#[cfg(test)]").next().unwrap();
            for word in [
                "WickedFile",
                "wickedfile",
                "Elite",
                "eliteworldwide",
                "PartsTech",
                "partstech",
            ] {
                assert!(!code.contains(word), "{name} names {word}");
            }
            for figure in ["40%", "50%", "60%", "65%", "70%", "75%"] {
                assert!(!code.contains(figure), "{name} writes {figure}");
            }
        }
    }
}
