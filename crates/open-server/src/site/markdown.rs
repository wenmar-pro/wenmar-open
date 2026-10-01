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

#[derive(Debug, Clone, Default)]
pub struct Section {
    pub heading: String,
    pub paragraphs: Vec<String>,
    /// Something to copy and paste, shown in a fixed-width block.
    pub code: Option<String>,
    /// Links, as text and address.
    pub links: Vec<(String, String)>,
}

/// A [`Doc`] as Markdown. Its text is this project's own, so it is not
/// escaped.
pub fn doc(doc: &Doc) -> String {
    let mut text = format!("# {}\n\n{}\n", doc.title, doc.intro);
    for section in &doc.sections {
        text.push_str(&format!("\n## {}\n\n", section.heading));
        for paragraph in &section.paragraphs {
            text.push_str(paragraph);
            text.push_str("\n\n");
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
}
