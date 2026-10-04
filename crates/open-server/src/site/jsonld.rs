//! Structured data: what a page is about, in schema.org's words, as JSON-LD
//! in the page's head.
//!
//! Names from the data file go into it, so how it is written into a
//! `<script>` element matters: see [`script_safe`].

use serde_json::{Value, json};

use crate::api::types::Entry;

/// Wenmar Pro, wherever it is named.
const ORGANIZATION_ID: &str = "https://wenmarpro.com/#organization";

/// A JSON value as the text of a `<script type="application/ld+json">`
/// element.
///
/// The text of a script element is not HTML. It ends at the first
/// `</script`, and character references such as `&quot;` are not decoded
/// in it. So the five characters that could end the element, start a
/// comment or be mistaken for a line break are written as JSON's own `\u`
/// escapes. They can only occur inside a string, where the escape means
/// the same character, so a JSON reader gets back exactly what went in.
pub fn script_safe(value: &Value) -> String {
    let json = value.to_string();
    let mut safe = String::with_capacity(json.len());
    for character in json.chars() {
        match character {
            '<' => safe.push_str("\\u003c"),
            '>' => safe.push_str("\\u003e"),
            '&' => safe.push_str("\\u0026"),
            '\u{2028}' => safe.push_str("\\u2028"),
            '\u{2029}' => safe.push_str("\\u2029"),
            other => safe.push(other),
        }
    }
    safe
}

/// Several things about one page, as the text of one script element.
pub fn graph(things: Vec<Value>) -> String {
    script_safe(&json!({ "@context": "https://schema.org", "@graph": things }))
}

/// The site, and the one thing its box does. Search engines read the name;
/// none shows a search box for it any more.
pub fn website(base: &str) -> Value {
    json!({
        "@type": "WebSite",
        "@id": format!("{base}/#website"),
        "name": "Wenmar Open",
        "alternateName": "Wenmar Open VIN decoder",
        "url": format!("{base}/"),
        "description": "Free VIN decoding and vehicle data for auto repair shops.",
        "publisher": { "@id": ORGANIZATION_ID },
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{base}/vin?vin={{vin}}")
            },
            "query-input": "required name=vin"
        }
    })
}

/// Who runs the site.
pub fn organization(base: &str) -> Value {
    json!({
        "@type": "Organization",
        "@id": ORGANIZATION_ID,
        "name": "Wenmar Pro",
        "url": "https://wenmarpro.com/",
        "logo": format!("{base}/assets/apple-touch-icon.png")
    })
}

/// The way from the top of the site down to a page. `crumbs` are the pages
/// above it, as name and address on this site, outermost first; `name` is
/// the page itself, which needs no address.
pub fn breadcrumbs(base: &str, crumbs: &[(String, String)], name: &str) -> Value {
    let mut items: Vec<Value> = crumbs
        .iter()
        .zip(1u32..)
        .map(|((label, path), position)| {
            json!({
                "@type": "ListItem",
                "position": position,
                "name": label,
                "item": format!("{base}{path}")
            })
        })
        .collect();
    items.push(json!({
        "@type": "ListItem",
        "position": items.len() + 1,
        "name": name
    }));
    json!({ "@type": "BreadcrumbList", "itemListElement": items })
}

/// A model year. It is a `Car` and nothing else: with no price it is not a
/// product listing, and it must not be read as one.
pub fn car(url: &str, entry: &Entry, engines: &[String]) -> Value {
    let mut car = json!({
        "@type": "Car",
        "name": format!("{} {} {}", entry.year, entry.make, entry.model),
        "url": url,
        "vehicleModelDate": entry.year.to_string(),
        "brand": { "@type": "Brand", "name": entry.make },
        "model": entry.model
    });
    if let Some(body) = &entry.body {
        car["bodyType"] = json!(body);
    }
    if let Some(drive) = &entry.drive {
        car["driveWheelConfiguration"] = json!(drive);
    }
    if let Some(transmission) = &entry.transmission {
        car["vehicleTransmission"] = json!(transmission);
    }
    if !engines.is_empty() {
        let engines: Vec<Value> = engines
            .iter()
            .map(|name| json!({ "@type": "EngineSpecification", "name": name }))
            .collect();
        car["vehicleEngine"] = Value::Array(engines);
    }
    car
}

/// A page of prose. It carries no date: a date written here would be wrong
/// the first time the page is reworded.
pub fn article(base: &str, path: &str, headline: &str, description: &str) -> Value {
    json!({
        "@type": "Article",
        "headline": headline,
        "description": description,
        "url": format!("{base}{path}"),
        "mainEntityOfPage": format!("{base}{path}"),
        "inLanguage": "en",
        "image": format!("{base}/assets/og.png"),
        "author": { "@id": ORGANIZATION_ID },
        "publisher": { "@id": ORGANIZATION_ID }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_can_end_the_script_element_or_change_the_json() {
        let hostile = [
            "</script><script>alert(1)</script>",
            "</SCRIPT >",
            "<!-- <script>",
            "]]>",
            "\"quoted\" and 'single'",
            "back\\slash \\u003c",
            "B&B &quot; &#60;",
            "line\u{2028}separator\u{2029}paragraph",
            "a\nb\tc\r\u{0}",
            "naïve ñ 日本",
        ];
        for text in hostile {
            let value = json!({ "name": text, "list": [text, { "inner": text }] });
            let safe = script_safe(&value);
            for forbidden in ['<', '>', '&', '\u{2028}', '\u{2029}'] {
                assert!(!safe.contains(forbidden), "{forbidden:?} in {safe}");
            }
            assert!(!safe.to_lowercase().contains("</script"));
            // A JSON reader gets back exactly what went in.
            let read: Value = serde_json::from_str(&safe).unwrap();
            assert_eq!(read, value, "{text}");
        }
    }

    #[test]
    fn a_graph_is_one_object_with_a_context() {
        let text = graph(vec![json!({ "@type": "Thing", "name": "a < b" })]);
        assert_eq!(
            text,
            r#"{"@context":"https://schema.org","@graph":[{"@type":"Thing","name":"a \u003c b"}]}"#
        );
    }

    #[test]
    fn breadcrumbs_number_the_way_down_and_end_at_the_page() {
        let crumbs = vec![
            ("Makes".to_owned(), "/makes".to_owned()),
            ("Honda".to_owned(), "/makes/honda".to_owned()),
        ];
        let trail = breadcrumbs("https://open.example", &crumbs, "2019 Honda Civic");
        assert_eq!(
            trail["itemListElement"],
            json!([
                { "@type": "ListItem", "position": 1, "name": "Makes", "item": "https://open.example/makes" },
                { "@type": "ListItem", "position": 2, "name": "Honda", "item": "https://open.example/makes/honda" },
                { "@type": "ListItem", "position": 3, "name": "2019 Honda Civic" }
            ])
        );
        let alone = breadcrumbs("https://open.example", &[], "Makes");
        assert_eq!(alone["itemListElement"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn the_templates_use_safe_once_and_only_for_structured_data() {
        let templates = [
            include_str!("../../templates/base.html"),
            include_str!("../../templates/doc.html"),
            include_str!("../../templates/home.html"),
            include_str!("../../templates/make.html"),
            include_str!("../../templates/makes.html"),
            include_str!("../../templates/model_year.html"),
            include_str!("../../templates/problem.html"),
            include_str!("../../templates/vin.html"),
            include_str!("../../templates/vin_form.html"),
            include_str!("../../templates/wmi.html"),
            include_str!("../../templates/tool.html"),
            include_str!("../../templates/parts_matrix.html"),
            include_str!("../../templates/canada_invoice_tax.html"),
            include_str!("../../templates/gross_profit.html"),
            include_str!("../../templates/labor_rate.html"),
        ];
        let uses: usize = templates
            .iter()
            .map(|template| template.matches("|safe").count())
            .sum();
        assert_eq!(uses, 1);
        assert!(
            templates[0]
                .contains(r#"<script type="application/ld+json">{{ json_ld|safe }}</script>"#)
        );
        // And only an indexed page shows it.
        let before = templates[0].split("application/ld+json").next().unwrap();
        assert!(before.trim_end().ends_with(
            "{% if page.index -%}\n{% if let Some(json_ld) = page.json_ld -%}\n<script type=\""
        ));
    }
}
