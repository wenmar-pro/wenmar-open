//! Markdown versions of the reference pages, for AI agents.
//!
//! A page at `/makes/honda` has its Markdown at `/makes/honda.md`. The
//! Markdown is written from the same view the HTML page is rendered from.

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

/// Which form of a page was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Html,
    Markdown,
}

/// Splits the last part of an address into its name and its form:
/// `honda.md` is `honda` in Markdown.
pub fn split(segment: &str) -> (&str, Format) {
    match segment.strip_suffix(".md") {
        Some(name) => (name, Format::Markdown),
        None => (segment, Format::Html),
    }
}

/// Text from the data file, made safe to put in Markdown: characters that
/// would start markup are escaped, and a line break cannot start a new
/// block.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '#' | '+'
            | '-' | '!' | '|' | '~' | '&' => {
                escaped.push('\\');
                escaped.push(character);
            }
            '\n' | '\r' | '\t' => escaped.push(' '),
            other => escaped.push(other),
        }
    }
    escaped
}

/// A Markdown response. It is not indexed: the HTML page is the one search
/// engines should show, and `canonical` points to it.
pub fn response(body: String, canonical: &str) -> Response {
    let mut response = (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/markdown; charset=utf-8"),
        )],
        body,
    )
        .into_response();
    let headers = response.headers_mut();
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    if let Ok(link) = HeaderValue::from_str(&format!("<{canonical}>; rel=\"canonical\"")) {
        headers.insert(header::LINK, link);
    }
    response
}

/// A page of prose: a title, an opening paragraph and sections. The docs,
/// data and about pages are written once in this form and shown as HTML
/// and as Markdown.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub title: String,
    pub intro: String,
    pub sections: Vec<Section>,
}

/// A small table: what it holds, a header row, and rows of cells. The
/// first cell of a row names the row.
#[derive(Debug, Clone, Default)]
pub struct Table {
    pub caption: String,
    pub head: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Whether the last column holds sentences and the others a word or
    /// two. A page gives such a table's last column the room.
    pub prose: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Section {
    pub heading: String,
    pub paragraphs: Vec<String>,
    pub table: Option<Table>,
    /// Something to copy and paste, shown in a fixed-width block.
    pub code: Option<String>,
    /// Links, as text and address.
    pub links: Vec<(String, String)>,
}

impl Section {
    /// The heading as part of an address: `For AI agents` is
    /// `for-ai-agents`.
    pub fn anchor(&self) -> String {
        wenmar_vehicles::text::slug(&self.heading)
    }
}

fn table(table: &Table) -> String {
    let row = |cells: &[String]| format!("| {} |\n", cells.join(" | "));
    let mut text = format!("{}:\n\n", table.caption);
    text.push_str(&row(&table.head));
    text.push_str(&row(&vec!["---".to_owned(); table.head.len()]));
    for cells in &table.rows {
        text.push_str(&row(cells));
    }
    text.push('\n');
    text
}

/// A [`Doc`] as Markdown. Its text is this project's own, so it is not
/// escaped.
pub fn doc(doc: &Doc) -> String {
    doc_at(doc, 1)
}

/// A [`Doc`] as Markdown with its title at heading `level` and its sections
/// one below, for a document that holds several.
pub fn doc_at(doc: &Doc, level: usize) -> String {
    let marks = "#".repeat(level);
    let mut text = format!("{marks} {}\n\n{}\n", doc.title, doc.intro);
    for section in &doc.sections {
        text.push_str(&format!("\n{marks}# {}\n\n", section.heading));
        for paragraph in &section.paragraphs {
            text.push_str(paragraph);
            text.push_str("\n\n");
        }
        if let Some(found) = &section.table {
            text.push_str(&table(found));
        }
        if let Some(code) = &section.code {
            text.push_str(&format!("```\n{code}\n```\n\n"));
        }
        for (label, address) in &section.links {
            text.push_str(&format!("- [{label}]({address})\n"));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_is_read_from_the_end_of_the_address() {
        assert_eq!(split("honda"), ("honda", Format::Html));
        assert_eq!(split("honda.md"), ("honda", Format::Markdown));
        assert_eq!(split("2019.md"), ("2019", Format::Markdown));
        assert_eq!(split(".md"), ("", Format::Markdown));
        assert_eq!(split("honda.md.md"), ("honda.md", Format::Markdown));
        assert_eq!(split("honda.MD"), ("honda.MD", Format::Html));
    }

    #[test]
    fn data_cannot_start_markup() {
        assert_eq!(escape("F-150"), "F\\-150");
        assert_eq!(escape("CR-V [EX]"), "CR\\-V \\[EX\\]");
        assert_eq!(
            escape("<script>alert(1)</script>"),
            "\\<script\\>alert\\(1\\)\\</script\\>"
        );
        assert_eq!(
            escape("[click](https://evil.example)"),
            "\\[click\\]\\(https://evil.example\\)"
        );
        assert_eq!(escape("a\n# Heading\n- item"), "a \\# Heading \\- item");
        assert_eq!(escape("![x](y)"), "\\!\\[x\\]\\(y\\)");
        assert_eq!(escape("a | b"), "a \\| b");
        assert_eq!(escape("Civic"), "Civic");
    }

    #[test]
    fn a_doc_is_written_at_the_level_asked_for_with_its_tables() {
        let doc = Doc {
            title: "Title".to_owned(),
            intro: "Intro.".to_owned(),
            sections: vec![Section {
                heading: "For AI agents".to_owned(),
                paragraphs: vec!["One.".to_owned()],
                table: Some(Table {
                    caption: "Codes".to_owned(),
                    head: vec!["Code".to_owned(), "Year".to_owned()],
                    rows: vec![vec!["P".to_owned(), "2023".to_owned()]],
                    prose: false,
                }),
                ..Section::default()
            }],
        };
        assert_eq!(
            super::doc(&doc),
            "# Title\n\nIntro.\n\n## For AI agents\n\nOne.\n\nCodes:\n\n| Code | Year |\n| --- | --- |\n| P | 2023 |\n\n"
        );
        assert!(doc_at(&doc, 2).starts_with("## Title\n\nIntro.\n\n### For AI agents\n"));
        assert_eq!(doc.sections[0].anchor(), "for-ai-agents");
    }
}
