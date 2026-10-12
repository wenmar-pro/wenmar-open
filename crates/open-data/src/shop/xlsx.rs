//! Reads the cells of the first sheet of an `.xlsx` workbook.
//!
//! An `.xlsx` file is a zip of XML files. EPA's guide files need only the
//! text and numbers of one sheet, so this reads those three files with a
//! plain scan rather than through a spreadsheet library. It knows nothing
//! of styles, dates or formulas: a cell is the text the file stores for it.

use std::io::{Cursor, Read};

use anyhow::{Context, Result, bail};

/// The elements named `name` in `xml`, each as its attribute text and its
/// content. For elements that do not hold another of the same name.
fn elements<'a>(xml: &'a str, name: &str) -> Vec<(&'a str, &'a str)> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        // `<c` must not match `<col` or `<cols`.
        if !after.starts_with([' ', '>', '/', '\t', '\r', '\n']) {
            rest = after;
            continue;
        }
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let attributes = &after[..tag_end];
        let body = &after[tag_end + 1..];
        if let Some(attributes) = attributes.strip_suffix('/') {
            found.push((attributes, ""));
            rest = body;
            continue;
        }
        let Some(end) = body.find(&close) else {
            break;
        };
        found.push((attributes, &body[..end]));
        rest = &body[end + close.len()..];
    }
    found
}

/// The value of an attribute in a tag's attribute text.
fn attribute<'a>(attributes: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = attributes;
    while let Some(start) = rest.find(name) {
        let before_ok = start == 0 || rest[..start].ends_with([' ', '\t', '\r', '\n']);
        let after = &rest[start + name.len()..];
        if before_ok && let Some(value) = after.strip_prefix("=\"") {
            return value.find('"').map(|end| &value[..end]);
        }
        rest = after;
    }
    None
}

/// Text with XML's character references put back.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = after;
            continue;
        };
        let entity = &after[..end];
        let character = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|number| number.parse().ok())
                })
                .and_then(char::from_u32),
        };
        match character {
            Some(character) => {
                out.push(character);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text of a string item: every `<t>` in it, joined. Phonetic runs
/// (`<rPh>`) are left out.
fn string_item(item: &str) -> String {
    let mut plain = String::with_capacity(item.len());
    let mut rest = item;
    while let Some(start) = rest.find("<rPh") {
        plain.push_str(&rest[..start]);
        rest = rest[start..]
            .find("</rPh>")
            .map_or("", |end| &rest[start + end + "</rPh>".len()..]);
    }
    plain.push_str(rest);
    elements(&plain, "t")
        .into_iter()
        .map(|(_, text)| unescape(text))
        .collect()
}

/// The zero-based column of a cell reference: `A1` is 0, `AB7` is 27.
fn column(reference: &str) -> Option<usize> {
    let mut column = 0usize;
    let mut letters = 0;
    for byte in reference.bytes() {
        if !byte.is_ascii_alphabetic() {
            break;
        }
        column = column * 26 + usize::from(byte.to_ascii_uppercase() - b'A') + 1;
        letters += 1;
    }
    (letters > 0).then(|| column - 1)
}

fn entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String> {
    let mut file = archive
        .by_name(name)
        .with_context(|| format!("the workbook has no {name}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("reading {name}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The rows of a sheet's XML. `strings` is the shared string table.
fn sheet_rows(sheet: &str, strings: &[String]) -> Result<Vec<Vec<String>>> {
    if !sheet.contains("<sheetData") {
        bail!("the sheet has no sheetData; is it written with a namespace prefix?");
    }
    let mut rows = Vec::new();
    for (_, row) in elements(sheet, "row") {
        let mut cells: Vec<String> = Vec::new();
        let mut next = 0usize;
        for (attributes, content) in elements(row, "c") {
            let at = attribute(attributes, "r").and_then(column).unwrap_or(next);
            next = at + 1;
            let value = elements(content, "v")
                .first()
                .map(|(_, value)| unescape(value));
            let text = match attribute(attributes, "t") {
                Some("s") => {
                    let index: usize = value
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .parse()
                        .context("a shared string cell has no number")?;
                    strings
                        .get(index)
                        .with_context(|| format!("no shared string {index}"))?
                        .clone()
                }
                Some("inlineStr") => string_item(content),
                _ => value.unwrap_or_default(),
            };
            if cells.len() <= at {
                cells.resize(at + 1, String::new());
            }
            cells[at] = text;
        }
        rows.push(cells);
    }
    Ok(rows)
}

/// Every row of the first sheet of the workbook in `bytes`, each as its
/// cells from column A on. A cell the file leaves out is an empty string,
/// and rows are as long as their last stored cell.
pub fn first_sheet(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).context("the workbook is not a zip file")?;
    let workbook = entry(&mut archive, "xl/workbook.xml")?;
    let sheets = elements(&workbook, "sheet");
    let (first, _) = sheets.first().context("the workbook lists no sheet")?;
    let id = attribute(first, "r:id").context("the first sheet has no r:id")?;
    let relations = entry(&mut archive, "xl/_rels/workbook.xml.rels")?;
    let target = elements(&relations, "Relationship")
        .into_iter()
        .find(|(attributes, _)| attribute(attributes, "Id") == Some(id))
        .and_then(|(attributes, _)| attribute(attributes, "Target"))
        .with_context(|| format!("no relationship {id} for the first sheet"))?;
    let target = target.trim_start_matches('/');
    let path = if target.starts_with("xl/") {
        target.to_owned()
    } else {
        format!("xl/{target}")
    };
    // A workbook with no text at all has no shared strings file.
    let strings: Vec<String> = match entry(&mut archive, "xl/sharedStrings.xml") {
        Ok(xml) => elements(&xml, "si")
            .into_iter()
            .map(|(_, item)| string_item(item))
            .collect(),
        Err(_) => Vec::new(),
    };
    let sheet = entry(&mut archive, &path)?;
    sheet_rows(&sheet, &strings).with_context(|| format!("reading {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_are_found_by_whole_name() {
        let xml =
            r#"<cols><col min="1"/></cols><c r="A1" t="s"><v>0</v></c><c r="B1"/><c><v>7</v></c>"#;
        let cells = elements(xml, "c");
        assert_eq!(cells.len(), 3);
        assert_eq!(attribute(cells[0].0, "r"), Some("A1"));
        assert_eq!(attribute(cells[0].0, "t"), Some("s"));
        assert_eq!(cells[0].1, "<v>0</v>");
        assert_eq!(cells[1], (r#" r="B1""#, ""));
        assert_eq!(attribute(cells[2].0, "r"), None);
        assert_eq!(elements("<v>1</v><v>2", "v"), [("", "1")]);
    }

    #[test]
    fn an_attribute_is_found_by_its_whole_name() {
        let tag = r#" name="FEguide" sheetId="1" r:id="rId1""#;
        assert_eq!(attribute(tag, "r:id"), Some("rId1"));
        assert_eq!(attribute(tag, "name"), Some("FEguide"));
        // `Id` is not the end of `sheetId`.
        assert_eq!(attribute(tag, "Id"), None);
        assert_eq!(
            attribute(r#" Id="rId3" Target="worksheets/sheet1.xml""#, "Id"),
            Some("rId3")
        );
    }

    #[test]
    fn references_and_columns() {
        assert_eq!(
            unescape("A &amp; B &lt;1&gt; &quot;x&quot; &apos;y&apos;"),
            "A & B <1> \"x\" 'y'"
        );
        assert_eq!(unescape("caf&#233; &#x41;"), "caf\u{e9} A");
        assert_eq!(unescape("AT&T & co; &bogus;"), "AT&T & co; &bogus;");
        assert_eq!(column("A1"), Some(0));
        assert_eq!(column("Z9"), Some(25));
        assert_eq!(column("AA10"), Some(26));
        assert_eq!(column("FK2"), Some(166));
        assert_eq!(column("12"), None);
    }

    #[test]
    fn a_string_item_joins_its_runs_and_drops_phonetics() {
        assert_eq!(string_item("<t>Oil Viscosity</t>"), "Oil Viscosity");
        assert_eq!(
            string_item(r#"<r><t xml:space="preserve">5W-30 </t></r><r><rPr/><t>GF-5</t></r>"#),
            "5W-30 GF-5"
        );
        assert_eq!(
            string_item("<t>A</t><rPh sb=\"0\" eb=\"1\"><t>x</t></rPh><t>B</t>"),
            "AB"
        );
        assert_eq!(string_item("<t/>"), "");
    }

    #[test]
    fn rows_keep_their_columns() {
        let strings = vec!["Model Year".to_owned(), "Ford".to_owned()];
        let sheet = r#"<worksheet><sheetData>
            <row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="inlineStr"><is><t>x &amp; y</t></is></c></row>
            <row r="2"><c r="A2"><v>2018</v></c><c r="B2" t="s"><v>1</v></c><c r="C2" t="str"><v>5W-20</v></c><c r="E2" s="3"/></row>
            <row r="3"/>
            </sheetData></worksheet>"#;
        assert_eq!(
            sheet_rows(sheet, &strings).unwrap(),
            [
                vec!["Model Year".to_owned(), String::new(), "x & y".to_owned()],
                vec![
                    "2018".to_owned(),
                    "Ford".to_owned(),
                    "5W-20".to_owned(),
                    String::new(),
                    String::new()
                ],
                vec![],
            ]
        );
        assert!(sheet_rows("<x:worksheet/>", &strings).is_err());
        let bad = r#"<sheetData><row><c r="A1" t="s"><v>9</v></c></row></sheetData>"#;
        assert!(sheet_rows(bad, &strings).is_err());
    }

    #[test]
    fn a_file_that_is_not_a_workbook_is_refused() {
        let error = first_sheet(b"not a zip").unwrap_err();
        assert!(format!("{error:#}").contains("not a zip file"), "{error:#}");
    }
}
