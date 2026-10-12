//! "The manufacturer's VIN guide filed with NHTSA": the newest letters a
//! make's manufacturers, or the manufacturer of one code, have filed under
//! 49 CFR Part 565 to explain how their VINs are built.
//!
//! The letters are the manufacturers' own documents. NHTSA keeps them, and
//! this site holds NHTSA's list of them and never a letter: every link
//! here opens NHTSA's copy.

use wenmar_shopdata::Dataset;
use wenmar_shopdata::vin_guides::{VinGuide, is_document_url};

use crate::site::markdown::escape;
use crate::state::AppState;

/// Filings shown on a page. The count of all of them is said beside.
pub const SHOWN: usize = 10;

/// The heading of the section, on a page and in its Markdown version.
pub const HEADING: &str = "The manufacturer's VIN guide filed with NHTSA";

/// What the documents are, said wherever they are listed.
pub const WHAT: &str = "A manufacturer files letters with NHTSA under 49 CFR Part 565 that explain how its VINs are built: what each position of the VIN stands for. The letters are the manufacturers' own documents. NHTSA keeps them, and each link here opens NHTSA's copy. This site keeps the list, not the letters.";

/// What a page lists filings for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Of {
    /// A catalog make, by `catalog_make.id`.
    Make(i64),
    /// A manufacturer code, such as `KM8`.
    Wmi(String),
}

/// One filing, as a page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideLine {
    /// The date on the letter, as `2026-06-01`.
    pub date: String,
    /// Who filed it, as NHTSA writes the name.
    pub manufacturer: String,
    /// The model years it covers, in words. Empty when NHTSA gives none.
    pub years: String,
    /// NHTSA's name for the document.
    pub name: String,
    /// NHTSA's copy of the document.
    pub url: String,
}

/// The section, when there is something to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guides {
    pub what: &'static str,
    /// The newest filings, at most [`SHOWN`].
    pub lines: Vec<GuideLine>,
    /// How many filings there are in all.
    pub total: usize,
    /// The day the list was current, as `2026-10-12`.
    pub as_of: String,
}

/// The model years a filing covers, in words.
pub fn years(from: Option<u16>, to: Option<u16>) -> String {
    match (from, to) {
        (Some(from), Some(to)) if from == to => format!("model year {from}"),
        (Some(from), Some(to)) => format!("model years {from} to {to}"),
        (Some(from), None) => format!("from model year {from}"),
        (None, Some(to)) => format!("up to model year {to}"),
        (None, None) => String::new(),
    }
}

/// A filing as a line. `None` when its address is not NHTSA's copy of a
/// filing: the shop file is data, and only that address is ever linked.
fn line(guide: VinGuide) -> Option<GuideLine> {
    is_document_url(&guide.url).then(|| GuideLine {
        date: guide.letter_date,
        manufacturer: guide.manufacturer,
        years: years(guide.year_from, guide.year_to),
        name: guide.name,
        url: guide.url,
    })
}

/// The section for a make or a manufacturer code.
///
/// `None` on a server that does not hold the dataset, when no filing
/// reaches the make or the code, and when the list could not be read. The
/// page that asked is then exactly as it is without the section.
pub async fn guides(state: &AppState, of: Of) -> Option<Guides> {
    let shop = state.dataset(Dataset::VinGuides).ok()?;
    let read = shop
        .run(move |reader| match &of {
            Of::Make(make_id) => reader.vin_guides_for_make(*make_id, SHOWN),
            Of::Wmi(code) => reader.vin_guides_for_wmi(code, SHOWN),
        })
        .await;
    let list = match read {
        Ok(Ok(list)) => list,
        Ok(Err(error)) => {
            tracing::warn!(%error, "VIN guides left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "VIN guides left off a page");
            return None;
        }
    };
    let read = list.guides.len();
    let lines: Vec<GuideLine> = list.guides.into_iter().filter_map(line).collect();
    if lines.is_empty() {
        return None;
    }
    // A filing that was left out is not counted either.
    let total = usize::try_from(list.total)
        .unwrap_or(usize::MAX)
        .saturating_sub(read - lines.len());
    Some(Guides {
        what: WHAT,
        lines,
        total,
        as_of: shop
            .datasets()
            .get(Dataset::VinGuides)
            .map(|entry| entry.as_of.clone())
            .unwrap_or_default(),
    })
}

/// The section in Markdown, to go at the end of a page's.
pub fn markdown(guides: &Guides) -> String {
    let mut text = format!("\n## {HEADING}\n\n{WHAT}\n\n");
    for line in &guides.lines {
        text.push_str(&format!("- [{}, {}", line.date, escape(&line.manufacturer)));
        if !line.years.is_empty() {
            text.push_str(&format!(", {}", line.years));
        }
        text.push_str(&format!(
            "]({}). Part 565 letter {}.\n",
            line.url,
            escape(&line.name)
        ));
    }
    text.push('\n');
    if guides.total > guides.lines.len() {
        text.push_str(&format!(
            "The newest {} of {} filings are shown. ",
            guides.lines.len(),
            guides.total
        ));
    }
    text.push_str(&format!("The list is NHTSA's, as of {}.\n", guides.as_of));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_years_are_said_in_words() {
        assert_eq!(years(Some(2027), Some(2027)), "model year 2027");
        assert_eq!(years(Some(2025), Some(2030)), "model years 2025 to 2030");
        assert_eq!(years(Some(2026), None), "from model year 2026");
        assert_eq!(years(None, Some(2017)), "up to model year 2017");
        assert_eq!(years(None, None), "");
    }

    #[test]
    fn a_filing_with_an_address_that_is_not_nhtsas_copy_is_not_a_line() {
        let guide = |url: &str| VinGuide {
            name: "ORG26610".to_owned(),
            manufacturer: "SUBARU CORPORATION".to_owned(),
            kind: "565".to_owned(),
            letter_date: "2026-06-01".to_owned(),
            year_from: Some(2027),
            year_to: Some(2027),
            url: url.to_owned(),
        };
        let found = line(guide(
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b",
        ))
        .unwrap();
        assert_eq!(found.years, "model year 2027");
        assert_eq!(found.date, "2026-06-01");
        assert!(line(guide("javascript:alert(1)")).is_none());
        assert!(line(guide("https://example.com/x")).is_none());
    }
}
