//! The bulletin pages: `/bulletins`, a model year's bulletins, and one
//! bulletin.
//!
//! A page shows NHTSA's index of a manufacturer's communication and
//! NHTSA's summary of it. The document is the manufacturer's: every page
//! links NHTSA's copy and none holds or quotes it. A bulletin is not a
//! recall, and no page says one applies to a vehicle.
//!
//! The pages are built from the same answers the API gives. A server with
//! no bulletin data has none of them.

use askama::Template;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::bulletins::{BulletinType, MOST_RELATED};
use wenmar_vehicles::Scope;
use wenmar_vehicles::text::is_slug;

use crate::api::bulletins::{
    BulletinFilter, BulletinSearchQuery, NOTE, bulletin_and_reach, bulletin_search_op,
    bulletins_of, listed, narrowed,
};
use crate::api::recalls::model_year;
use crate::api::types::{Bulletin, BulletinCount, BulletinDetail, Make, VehicleBulletins};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::recalls::{self, ComponentQuery};
use crate::site::{self, Page, jsonld, report, seo};
use crate::state::AppState;

/// Bulletins on a page of a list.
pub const PER_PAGE: usize = 25;

/// Bulletins on the entry page.
const NEWEST: usize = 10;

/// Most model years one bulletin's page links.
const MOST_VEHICLES: usize = 200;

/// Most vehicles named in a bulletin's structured data.
const MOST_SUBJECTS: usize = 20;

/// One bulletin in a list on a page.
pub struct Line {
    pub nhtsa_id: i64,
    /// The bulletin's page, as a path on this site.
    pub href: String,
    /// The manufacturer's number, or NHTSA's id when it gave none.
    pub label: String,
    pub date: String,
    pub kind: String,
    pub component: String,
    pub summary: String,
}

pub fn line(bulletin: &Bulletin) -> Line {
    Line {
        nhtsa_id: bulletin.nhtsa_id,
        href: format!("/bulletins/{}", bulletin.nhtsa_id),
        label: bulletin
            .document_id
            .clone()
            .unwrap_or_else(|| format!("NHTSA {}", bulletin.nhtsa_id)),
        date: bulletin.date_added.clone(),
        kind: bulletin.type_name.clone(),
        component: bulletin.component.clone().unwrap_or_default(),
        summary: bulletin.summary.clone().unwrap_or_default(),
    }
}

fn markdown_line(line: &Line) -> String {
    let mut text = format!(
        "- [{}]({}.md), {}, {}",
        escape(&line.label),
        line.href,
        line.date,
        escape(&line.kind)
    );
    if !line.component.is_empty() {
        text.push_str(&format!(": {}", escape(&line.component)));
    }
    if !line.summary.is_empty() {
        text.push_str(&format!(". {}", escape(&line.summary)));
    }
    text.push('\n');
    text
}

/// "As of" for the bulletins.
fn dated(day: Option<&String>) -> String {
    match day {
        Some(day) => format!("as of {day}"),
        None => "date not on file".to_owned(),
    }
}

// ----- /bulletins -----

#[derive(Template)]
#[template(path = "bulletins.html")]
struct IndexPage {
    page: Page,
    years: Vec<u16>,
    makes: Vec<Make>,
    note: &'static str,
    newest: Vec<Line>,
    dated: String,
}

/// The entry page: what a bulletin is, the VIN box, the picker and the
/// newest bulletins.
pub async fn index(State(state): State<AppState>) -> Response {
    if state.dataset(Dataset::Bulletins).is_err() {
        return site::not_found(&state);
    }
    let makes = state
        .db()
        .run(|worker| worker.catalog.makes(None, Scope::Light, "", 500))
        .await;
    let makes = match makes {
        Ok(Ok(makes)) => makes,
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let asked = BulletinSearchQuery {
        limit: Some(NEWEST),
        ..BulletinSearchQuery::default()
    };
    let list = match bulletin_search_op(&state, asked).await {
        Ok(list) => list,
        Err(error) => return site::failed(&state, &error),
    };
    let page = Page::new(
        &state,
        seo::title("Service bulletins by vehicle"),
        "Manufacturer service bulletins, warranty extensions and over-the-air updates that NHTSA publishes, by VIN or by year, make and model, each with a link to NHTSA's copy of the document.",
    )
    .indexed(&state, "/bulletins")
    .with_mono();
    site::html(
        StatusCode::OK,
        &IndexPage {
            page,
            years: state.years(Scope::Light).unwrap_or_default().to_vec(),
            makes: makes.into_iter().map(Make::from).collect(),
            note: NOTE,
            newest: list.bulletins.iter().map(line).collect(),
            dated: dated(list.as_of.as_ref()),
        },
    )
}

// ----- a model year's bulletins -----

/// One choice of a filter: a link.
pub struct Choice {
    pub name: String,
    pub count: u64,
    pub href: String,
    pub current: bool,
}

/// The choices of one filter, under a heading.
pub struct Choices {
    pub heading: &'static str,
    pub id: &'static str,
    pub choices: Vec<Choice>,
}

#[derive(Template)]
#[template(path = "bulletin_model_year.html")]
struct ModelYearBulletinsPage {
    page: Page,
    /// `2023 Hyundai Kona`.
    name: String,
    model_year_path: String,
    /// This page without any narrowing.
    path: String,
    note: &'static str,
    total: u64,
    /// The warranty extensions, said before anything else. Empty when the
    /// list is narrowed or is not at its first page.
    warranty: Vec<Line>,
    filters: Vec<Choices>,
    /// What the list is narrowed to, in words. Empty when it is not.
    narrowed_to: Vec<String>,
    lines: Vec<Line>,
    next: Option<String>,
    dated: String,
    recalls_path: Option<String>,
    report: report::ReportLinks,
}

/// The query string of a filter link: `key=value`, with the text search
/// kept and the cursor dropped.
fn filter_href(path: &str, key: &str, value: &str, q: Option<&str>) -> String {
    let mut href = format!("{path}?{key}={}", report::encode(value));
    if let Some(q) = q {
        href.push_str(&format!("&q={}", report::encode(q)));
    }
    href
}

fn choices(
    heading: &'static str,
    id: &'static str,
    key: &str,
    counts: &[BulletinCount],
    chosen: Option<&str>,
    path: &str,
    q: Option<&str>,
) -> Option<Choices> {
    if counts.is_empty() {
        return None;
    }
    Some(Choices {
        heading,
        id,
        choices: counts
            .iter()
            .map(|entry| Choice {
                name: entry.name.clone(),
                count: entry.count,
                href: filter_href(path, key, &entry.id, q),
                current: chosen == Some(entry.id.as_str()),
            })
            .collect(),
    })
}

fn model_year_markdown(
    name: &str,
    answer: &VehicleBulletins,
    report: &report::ReportLinks,
) -> String {
    let mut text = format!("# Service bulletins for the {}\n\n", escape(name));
    text.push_str(NOTE);
    text.push_str(&format!(
        "\n\n{} from NHTSA, {}.\n\n",
        seo::count(answer.counts.count as usize, "bulletin"),
        dated(answer.as_of.as_ref())
    ));
    if answer.bulletins.is_empty() {
        text.push_str("None found.\n");
    }
    for bulletin in &answer.bulletins {
        text.push_str(&markdown_line(&line(bulletin)));
    }
    if let Some(cursor) = &answer.next_cursor {
        text.push_str(&format!(
            "\nMore: [the next page](/v1/vehicles/{}/bulletins?cursor={cursor}) (JSON)\n",
            answer.vehicle.id
        ));
    }
    text.push_str(&format!(
        "\nJSON: [/v1/vehicles/{id}/bulletins](/v1/vehicles/{id}/bulletins)\n",
        id = answer.vehicle.id
    ));
    text.push_str(&format!(
        "\n[Report an error on this page]({})\n",
        report.issue
    ));
    text
}

/// What is under a model year: `bulletins` here, and whatever
/// `recalls::model_year_section` serves.
pub async fn model_year_section(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String, String)>, PathRejection>,
    recall_query: Result<Query<ComponentQuery>, QueryRejection>,
    bulletin_query: Result<Query<BulletinFilter>, QueryRejection>,
    uri: Uri,
) -> Response {
    let section = path
        .as_ref()
        .ok()
        .map(|Path((_, _, _, segment))| markdown::split(segment).0.to_owned());
    if section.as_deref() != Some("bulletins") {
        return recalls::model_year_section(State(state), path, recall_query, uri).await;
    }
    let Ok(Path((make, model, year, segment))) = path else {
        return site::not_found(&state);
    };
    let (_, format) = markdown::split(&segment);
    let Some(year) = site::year_in(&year) else {
        return site::not_found(&state);
    };
    if !is_slug(&make) || !is_slug(&model) {
        return site::not_found(&state);
    }
    let Ok(shop) = state.dataset(Dataset::Bulletins) else {
        return site::not_found(&state);
    };
    // A filter that cannot be read narrows nothing: the page is a list of
    // links, and a mistyped address should still show the vehicle.
    let filter = bulletin_query
        .map(|Query(filter)| filter)
        .unwrap_or_default();
    let q = filter
        .q
        .clone()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    let mut asked = match narrowed(BulletinFilter {
        limit: Some(PER_PAGE),
        ..filter.clone()
    }) {
        Ok(asked) => asked,
        Err(_) => match narrowed(BulletinFilter {
            limit: Some(PER_PAGE),
            ..BulletinFilter::default()
        }) {
            Ok(asked) => asked,
            Err(error) => return site::failed(&state, &error),
        },
    };
    asked.limit = PER_PAGE;
    let first_page = asked.cursor.is_none();
    let unfiltered = asked.kind.is_none()
        && asked.component.is_none()
        && asked.code.is_none()
        && asked.topic.is_none()
        && asked.q.is_none();
    let chosen_type = asked.kind.map(BulletinType::id);
    let chosen = (
        asked.component.clone(),
        asked.code.clone(),
        asked.topic.clone(),
    );

    let id = format!("{year:04}_{make}_{model}");
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(Some(found))) => found,
        Ok(Ok(None)) => return site::not_found(&state),
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let answer = match bulletins_of(&state, shop, &found, asked).await {
        Ok(answer) => answer,
        Err(error) => return site::failed(&state, &error),
    };
    let model_year_path = format!("/makes/{}/{}/{year}", found.make_slug, found.model_slug);
    let path = format!("{model_year_path}/bulletins");
    let name = format!(
        "{} {} {}",
        answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
    );
    let report = report::links(state.config(), &path, &name);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(model_year_markdown(&name, &answer, &report), &canonical);
    }

    // Warranty extensions close jobs, so they are said first: on the
    // first page of the list as it is before any narrowing.
    let warranty: Vec<Line> = if unfiltered && first_page {
        let only = match narrowed(BulletinFilter {
            kind: Some(BulletinType::WarrantyExtension.id().to_owned()),
            limit: Some(PER_PAGE),
            ..BulletinFilter::default()
        }) {
            Ok(only) => only,
            Err(error) => return site::failed(&state, &error),
        };
        match bulletins_of(&state, shop, &found, only).await {
            Ok(extensions) => extensions.bulletins.iter().map(line).collect(),
            Err(error) => return site::failed(&state, &error),
        }
    } else {
        Vec::new()
    };

    let counts = &answer.counts;
    let q_ref = q.as_deref();
    let filters: Vec<Choices> = [
        choices(
            "By type",
            "types",
            "type",
            &counts.by_type,
            chosen_type,
            &path,
            q_ref,
        ),
        choices(
            "By component",
            "components",
            "component",
            &counts.components,
            chosen.0.as_deref(),
            &path,
            q_ref,
        ),
        choices(
            "By trouble code",
            "codes",
            "code",
            &counts.codes,
            chosen.1.as_deref(),
            &path,
            q_ref,
        ),
        choices(
            "By topic",
            "topics",
            "topic",
            &counts.topics,
            chosen.2.as_deref(),
            &path,
            q_ref,
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let named = |list: &[BulletinCount], id: &str| {
        list.iter()
            .find(|entry| entry.id == id)
            .map_or_else(|| id.to_owned(), |entry| entry.name.clone())
    };
    let mut narrowed_to: Vec<String> = Vec::new();
    if let Some(kind) = chosen_type {
        narrowed_to.push(format!("type: {}", named(&counts.by_type, kind)));
    }
    if let Some(component) = &chosen.0 {
        narrowed_to.push(format!(
            "component: {}",
            named(&counts.components, component)
        ));
    }
    if let Some(code) = &chosen.1 {
        narrowed_to.push(format!("trouble code: {code}"));
    }
    if let Some(topic) = &chosen.2 {
        narrowed_to.push(format!("topic: {topic}"));
    }
    if let Some(text) = &q {
        narrowed_to.push(format!("text: {text}"));
    }
    // The next page keeps every narrowing and adds the cursor.
    let next = answer.next_cursor.as_ref().map(|cursor| {
        let mut href = format!("{path}?cursor={cursor}");
        for (key, value) in [
            ("type", chosen_type.map(str::to_owned)),
            ("component", chosen.0.clone()),
            ("code", chosen.1.clone()),
            ("topic", chosen.2.clone()),
            ("q", q.clone()),
        ] {
            if let Some(value) = value {
                href.push_str(&format!("&{key}={}", report::encode(&value)));
            }
        }
        href
    });

    let total = counts.count;
    let description = if total == 0 {
        format!("No manufacturer service bulletin for the {name} was found in the NHTSA data.")
    } else {
        format!(
            "{} from the manufacturer for the {name}, as NHTSA publishes them: service bulletins, warranty extensions and software updates, with NHTSA's summaries.",
            seo::count(total as usize, "communication")
        )
    };
    let page = Page::new(
        &state,
        seo::title(&format!("{name} service bulletins")),
        description,
    )
    .with_markdown(&format!("{path}.md"))
    .in_section("makes")
    .under(vec![
        ("Makes".to_owned(), "/makes".to_owned()),
        (
            answer.vehicle.make.clone(),
            format!("/makes/{}", found.make_slug),
        ),
        (name.clone(), model_year_path.clone()),
    ]);
    let trail = page.trail("Service bulletins");
    let page = page.describing(vec![trail]);
    // As the recall list: indexed for a car, an MPV or a truck that has
    // something to show; a narrowed or later page is a view of the first.
    let listed_page = found.light && total > 0;
    let page = match (listed_page, uri.query().is_some()) {
        (true, false) => page.indexed(&state, &path),
        (true, true) => page.indexed(&state, &path).with_query(),
        (false, _) => page,
    };
    let indexed = page.index;
    let recalls_path = crate::api::recalls::recall_shop(&state, None)
        .is_ok()
        .then(|| format!("{model_year_path}/recalls"));
    let mut response = site::html(
        StatusCode::OK,
        &ModelYearBulletinsPage {
            page,
            name,
            model_year_path,
            path,
            note: NOTE,
            total,
            warranty,
            filters,
            narrowed_to,
            lines: answer.bulletins.iter().map(line).collect(),
            next,
            dated: dated(answer.as_of.as_ref()),
            recalls_path,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}

// ----- one bulletin -----

/// The model years of one name a bulletin lists, each a link where the
/// catalog has it.
pub struct VehicleLine {
    /// `HYUNDAI KONA`, as the source words it.
    pub name: String,
    /// `2022 to 2023`, `2023`, or empty when no model year was given.
    pub years: String,
    /// The bulletin list of each model year, as `(year, path)`.
    pub links: Vec<(String, String)>,
    pub matched: String,
}

/// A recall beside a bulletin.
pub struct RelatedRecall {
    pub campaign: String,
    pub href: String,
    pub date: String,
    pub component: String,
}

#[derive(Template)]
#[template(path = "bulletin.html")]
struct BulletinPage {
    page: Page,
    heading: String,
    note: &'static str,
    facts: Vec<(&'static str, String)>,
    summary: Vec<String>,
    document: Option<(String, String)>,
    components: Vec<String>,
    codes: Vec<String>,
    vehicles: Vec<VehicleLine>,
    more_vehicles: usize,
    related: Vec<Line>,
    recalls: Vec<RelatedRecall>,
    dated: String,
    report: report::ReportLinks,
}

fn facts(bulletin: &BulletinDetail) -> Vec<(&'static str, String)> {
    let mut facts = vec![
        ("Type", bulletin.type_name.clone()),
        ("NHTSA id", bulletin.nhtsa_id.to_string()),
    ];
    for (name, value) in [
        ("Manufacturer's number", &bulletin.document_id),
        ("Manufacturer's date", &bulletin.date),
        ("Campaign id or software version", &bulletin.mfr_campaign),
        ("Replaces", &bulletin.replaces),
        ("NHTSA component", &bulletin.component),
        ("Manufacturer's system", &bulletin.mfr_system),
    ] {
        if let Some(value) = value {
            facts.push((name, value.clone()));
        }
    }
    facts.insert(2, ("Published by NHTSA", bulletin.date_added.clone()));
    facts
}

/// The lines of the vehicle list, and how many were left out.
fn vehicle_lines(bulletin: &BulletinDetail) -> (Vec<VehicleLine>, usize) {
    let mut lines = Vec::new();
    for vehicle in bulletin.vehicles.iter().take(MOST_VEHICLES) {
        let years = match (vehicle.year_from, vehicle.year_to) {
            (Some(from), Some(to)) if from == to => from.to_string(),
            (Some(from), Some(to)) => format!("{from} to {to}"),
            _ => String::new(),
        };
        // A vehicle id is `year_make_model` in id forms, checked when the
        // answer was built.
        let links = vehicle
            .vehicle_ids
            .iter()
            .filter_map(|id| {
                let mut parts = id.splitn(3, '_');
                let (year, make, model) = (parts.next()?, parts.next()?, parts.next()?);
                (is_slug(make) && is_slug(model) && site::year_in(year).is_some()).then(|| {
                    (
                        year.to_owned(),
                        format!("/makes/{make}/{model}/{year}/bulletins"),
                    )
                })
            })
            .collect();
        lines.push(VehicleLine {
            name: format!("{} {}", vehicle.make, vehicle.model),
            years,
            links,
            matched: vehicle.matched.clone().unwrap_or_default(),
        });
    }
    (lines, bulletin.vehicles.len().saturating_sub(MOST_VEHICLES))
}

fn bulletin_markdown(
    heading: &str,
    bulletin: &BulletinDetail,
    facts: &[(&'static str, String)],
    related: &[Line],
    recalls: &[RelatedRecall],
    report: &report::ReportLinks,
) -> String {
    let mut text = format!("# {}\n\n{NOTE}\n\n", escape(heading));
    for (name, value) in facts {
        text.push_str(&format!("- {name}: {}\n", escape(value)));
    }
    if let Some(summary) = &bulletin.summary {
        text.push_str(&format!("\n## NHTSA's summary\n\n{}\n", escape(summary)));
    }
    text.push_str("\n## The document\n\n");
    match &bulletin.document {
        Some(document) => text.push_str(&format!(
            "- [NHTSA's copy of the document (PDF)]({})\n- [If that address does not open, try this one]({})\n\nThe document is the manufacturer's. This site does not hold it.\n",
            document.url, document.alternate_url
        )),
        None => text.push_str(
            "No address for NHTSA's copy can be built for this communication. Ask the dealer for it by the manufacturer's number.\n",
        ),
    }
    if !bulletin.components.is_empty() {
        let names: Vec<String> = bulletin
            .components
            .iter()
            .map(|component| escape(&component.name))
            .collect();
        text.push_str(&format!("\n## Components\n\n{}\n", names.join(", ")));
    }
    if !bulletin.codes.is_empty() {
        text.push_str(&format!(
            "\n## Trouble codes named\n\n{}\n",
            bulletin.codes.join(", ")
        ));
    }
    text.push_str("\n## Vehicles listed\n\n");
    for vehicle in bulletin.vehicles.iter().take(MOST_VEHICLES) {
        let years = match (vehicle.year_from, vehicle.year_to) {
            (Some(from), Some(to)) if from == to => format!("{from} "),
            (Some(from), Some(to)) => format!("{from} to {to} "),
            _ => String::new(),
        };
        text.push_str(&format!(
            "- {years}{} {}\n",
            escape(&vehicle.make),
            escape(&vehicle.model)
        ));
    }
    if !recalls.is_empty() {
        text.push_str("\n## Recalls for the same vehicles and component\n\n");
        for recall in recalls {
            text.push_str(&format!(
                "- [{}]({}.md), {}: {}\n",
                escape(&recall.campaign),
                recall.href,
                recall.date,
                escape(&recall.component)
            ));
        }
    }
    if !related.is_empty() {
        text.push_str("\n## Other bulletins for the same vehicles and component\n\n");
        for line in related {
            text.push_str(&markdown_line(line));
        }
    }
    text.push_str(&format!(
        "\nJSON: [/v1/bulletins/{id}](/v1/bulletins/{id})\n",
        id = bulletin.nhtsa_id
    ));
    text.push_str(&format!(
        "\n[Report an error on this page]({})\n",
        report.issue
    ));
    text
}

pub async fn bulletin(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Response {
    let Ok(Path(segment)) = path else {
        return site::not_found(&state);
    };
    let (number, format) = markdown::split(&segment);
    // One bulletin, one address: its id with no leading zero.
    if number.starts_with('0') {
        return site::not_found(&state);
    }
    let (found, _light) = match bulletin_and_reach(&state, number).await {
        Ok(found) => found,
        Err(ApiError::NotFound(_) | ApiError::DatasetUnavailable(_)) => {
            return site::not_found(&state);
        }
        Err(error) => return site::failed(&state, &error),
    };
    let Ok(shop) = state.dataset(Dataset::Bulletins) else {
        return site::not_found(&state);
    };
    let id = found.nhtsa_id;
    let related = shop
        .run(move |reader| {
            let bulletins = reader.related_bulletins(id, MOST_RELATED)?;
            let recalls = reader.related_recalls(id, MOST_RELATED)?;
            Ok::<_, wenmar_shopdata::ShopError>((bulletins, recalls))
        })
        .await;
    let (related_bulletins, related_recalls) = match related {
        Ok(Ok(found)) => found,
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let base = &state.config().base_url;
    let related: Vec<Line> = related_bulletins
        .into_iter()
        .map(|bulletin| line(&listed(base, bulletin)))
        .collect();
    let recalls: Vec<RelatedRecall> = related_recalls
        .into_iter()
        .filter(|recall| {
            !recall.campaign.is_empty()
                && recall
                    .campaign
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric())
        })
        .map(|recall| RelatedRecall {
            href: format!("/recalls/{}/{}", recall.source, recall.campaign),
            campaign: recall.campaign,
            date: recall.date.unwrap_or_default(),
            component: recall.component.unwrap_or_default(),
        })
        .collect();

    let path = format!("/bulletins/{id}");
    let label = found
        .document_id
        .clone()
        .unwrap_or_else(|| format!("NHTSA {id}"));
    let heading = format!("{} {label}", found.type_name);
    let facts = facts(&found);
    let report = report::links(state.config(), &path, &heading);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(
            bulletin_markdown(&heading, &found, &facts, &related, &recalls, &report),
            &canonical,
        );
    }
    let (vehicles, more_vehicles) = vehicle_lines(&found);
    let subjects: Vec<String> = vehicles
        .iter()
        .take(MOST_SUBJECTS)
        .map(|vehicle| {
            if vehicle.years.is_empty() {
                vehicle.name.clone()
            } else {
                format!("{} {}", vehicle.years, vehicle.name)
            }
        })
        .collect();
    let what = found
        .components
        .last()
        .map(|component| component.name.to_lowercase())
        .unwrap_or_else(|| "the vehicle".to_owned());
    let description = seo::description(
        &[
            &[
                format!(
                    "{heading} from the manufacturer, about {what}, for {}.",
                    seo::count(found.vehicles.len(), "listed vehicle")
                ),
                format!("{heading}: a manufacturer communication NHTSA publishes."),
            ],
            &["With NHTSA's summary and a link to its copy of the document.".to_owned()],
        ],
        160,
    );
    let page = Page::new(&state, seo::title(&heading), description.clone())
        .with_markdown(&format!("{path}.md"))
        .with_mono()
        .as_article()
        .under(vec![("Bulletins".to_owned(), "/bulletins".to_owned())]);
    let things = vec![
        page.trail(&heading),
        jsonld::recall(
            &state.config().base_url,
            &path,
            &heading,
            &description,
            Some(found.date_added.as_str()),
            &subjects,
        ),
    ];
    // Every bulletin's page is offered to search engines (master plan Q8).
    let page = page.describing(things).indexed(&state, &path);
    let summary: Vec<String> = found
        .summary
        .iter()
        .flat_map(|text| text.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    site::html(
        StatusCode::OK,
        &BulletinPage {
            page,
            heading,
            note: NOTE,
            facts,
            summary,
            document: found
                .document
                .as_ref()
                .map(|document| (document.url.clone(), document.alternate_url.clone())),
            components: found
                .components
                .iter()
                .map(|component| component.name.clone())
                .collect(),
            codes: found.codes.clone(),
            vehicles,
            more_vehicles,
            related,
            recalls,
            dated: dated(found.as_of.as_ref()),
            report,
        },
    )
}
