//! Links to one model year's owner's manual and maintenance schedule on
//! the maker's own site.
//!
//! Two kinds, and a page says which is which. A link a person found and
//! opened for this model year comes from the shop file (`manual_link`).
//! A link made from the maker's address pattern comes from
//! `data/catalog/makes.yaml`: a person checked the pattern for the make,
//! not this address. Where both exist for one kind of document, only the
//! one a person opened is shown.
//!
//! Nothing here fetches, copies or keeps a manual.

use wenmar_shopdata::Dataset;

use crate::api::recalls::model_year;
use crate::site::links;
use crate::site::markdown::escape;
use crate::state::AppState;

/// One link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// `Owner's manual for the 2001 Testmake Sampler`.
    pub words: String,
    pub address: String,
    /// `Checked 2026-10-12.`, or what says the address came from a pattern.
    pub note: String,
}

/// What a page shows of a model year's documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub items: Vec<Item>,
}

/// The two kinds of document, with the words for each.
const KINDS: [(&str, &str); 2] = [
    ("owner_manual", "Owner's manual"),
    ("maintenance_schedule", "Maintenance schedule"),
];

/// The items made from a make's address patterns, for the kinds that
/// `found` does not already hold.
fn from_patterns(
    maker: &links::MakeLinks,
    make: &str,
    name: &str,
    year: u16,
    model_slug: &str,
    found: &[String],
) -> Vec<Item> {
    [
        ("owner_manual", "Owner's manuals", maker.manual_url_template),
        (
            "maintenance_schedule",
            "Maintenance schedule",
            maker.maintenance_url_template,
        ),
    ]
    .into_iter()
    .filter(|(kind, _, _)| !found.iter().any(|known| known == kind))
    .filter_map(|(_, words, template)| {
        let template = template?;
        Some(Item {
            words: format!("{words} for the {name}, at {make}"),
            address: links::fill(template, year, model_slug),
            note: format!(
                "This address was made from the pattern of {make}'s site, checked {}. Nobody opened this one; it may not exist for every model.",
                maker.checked
            ),
        })
    })
    .collect()
}

/// The links of a model year, by its vehicle id. `make` is the make as
/// the page writes it and `model_slug` the model's id form.
///
/// `None` when there is nothing to show. A shop file that cannot be read
/// costs the page its hand-found links, never the page.
pub async fn section(
    state: &AppState,
    vehicle_id: &str,
    make: &str,
    model_slug: &str,
) -> Option<Section> {
    let maker = links::for_make(make).filter(|maker| {
        maker.manual_url_template.is_some() || maker.maintenance_url_template.is_some()
    });
    let shop = state.dataset(Dataset::Manuals).ok();
    if maker.is_none() && shop.is_none() {
        return None;
    }
    let id = vehicle_id.to_owned();
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(found)) => found?,
        Ok(Err(error)) => {
            tracing::warn!(%error, "manual links left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "manual links left off a page");
            return None;
        }
    };
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let recorded = match shop {
        Some(shop) => match shop
            .run(move |reader| reader.manuals_for_vehicle(make_id, model_id, year))
            .await
        {
            Ok(Ok(recorded)) => recorded,
            Ok(Err(error)) => {
                tracing::warn!(%error, "manual links left off a page");
                Vec::new()
            }
            Err(error) => {
                tracing::warn!(%error, "manual links left off a page");
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let name = format!(
        "{} {} {}",
        found.vehicle.year, found.vehicle.make, found.vehicle.model
    );
    let mut items: Vec<Item> = recorded
        .iter()
        .map(|link| {
            let words = KINDS
                .iter()
                .find(|(kind, _)| *kind == link.kind)
                .map_or("Document", |(_, words)| words);
            let market = match link.market.as_deref() {
                Some("ca") => " (Canada)",
                Some("us") => " (United States)",
                _ => "",
            };
            Item {
                words: format!("{words} for the {name}{market}"),
                address: link.url.clone(),
                note: format!("Checked {}.", link.checked),
            }
        })
        .collect();
    if let Some(maker) = maker {
        let kinds: Vec<String> = recorded.iter().map(|link| link.kind.clone()).collect();
        items.extend(from_patterns(maker, make, &name, year, model_slug, &kinds));
    }
    if items.is_empty() {
        return None;
    }
    Some(Section { items })
}

/// What the page says under the list.
pub const NOTE: &str = "These lead to the maker's own site, which this site does not run. No manual is copied or kept here.";

/// The section as Markdown, for the page's `.md` twin.
pub fn markdown(section: &Section) -> String {
    let mut text = String::from("\n## Owner's manual and maintenance schedule\n\n");
    for item in &section.items {
        text.push_str(&format!(
            "- [{}]({}) {}\n",
            escape(&item.words),
            item.address,
            escape(&item.note)
        ));
    }
    text.push_str(&format!("\n{NOTE}\n"));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAKER: links::MakeLinks = links::MakeLinks {
        norm: "testmake",
        checked: "2026-10-12",
        recall_lookup_url: None,
        manual_url: None,
        maintenance_url: None,
        service_info_url: None,
        parts_url: None,
        customer_line: None,
        manual_url_template: Some("https://example.org/owners/{year}/{model}/manuals"),
        maintenance_url_template: Some("https://example.org/schedule/{model}"),
    };

    #[test]
    fn a_pattern_gives_a_link_that_says_nobody_opened_it() {
        let items = from_patterns(
            &MAKER,
            "Testmake",
            "2001 Testmake Sampler",
            2001,
            "sampler",
            &[],
        );
        assert_eq!(
            items,
            [
                Item {
                    words: "Owner's manuals for the 2001 Testmake Sampler, at Testmake".to_owned(),
                    address: "https://example.org/owners/2001/sampler/manuals".to_owned(),
                    note: "This address was made from the pattern of Testmake's site, checked 2026-10-12. Nobody opened this one; it may not exist for every model.".to_owned(),
                },
                Item {
                    words: "Maintenance schedule for the 2001 Testmake Sampler, at Testmake"
                        .to_owned(),
                    address: "https://example.org/schedule/sampler".to_owned(),
                    note: "This address was made from the pattern of Testmake's site, checked 2026-10-12. Nobody opened this one; it may not exist for every model.".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn a_link_a_person_opened_replaces_the_pattern_for_that_kind() {
        let items = from_patterns(
            &MAKER,
            "Testmake",
            "2001 Testmake Sampler",
            2001,
            "sampler",
            &["owner_manual".to_owned()],
        );
        assert_eq!(items.len(), 1);
        assert!(items[0].words.starts_with("Maintenance schedule"));
        let text = markdown(&Section { items });
        assert!(text.starts_with("\n## Owner's manual and maintenance schedule\n\n- [Maintenance schedule for the 2001 Testmake Sampler, at Testmake](https://example.org/schedule/sampler) This address was made from the pattern"), "{text}");
        assert!(text.ends_with(&format!("\n{NOTE}\n")));
    }
}
