//! The recall pages: `/recalls`, a model year's recalls, and one campaign.
//!
//! Every page here says a recall *may apply*. A campaign lists a year, a
//! make and a model; neither government publishes recalls by VIN as data,
//! so only the official lookups, which every page links, can say whether
//! one vehicle is in a campaign.
//!
//! The pages are built from the same answers the API gives, so the two
//! cannot disagree. A server with no recall data has none of these pages.

use askama::Template;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use serde::Deserialize;
use wenmar_shopdata::recalls::{RecallSource, systems};
use wenmar_vehicles::text::is_slug;
use wenmar_vehicles::{Scope, VehicleId};

use crate::api::recalls::{
    MAY_APPLY, RecallSearchQuery, campaign_and_reach, model_year, recall_search_op, recall_shop,
    recalls_of,
};
use crate::api::types::{Make, Recall, RecallCampaign, RecallLookup, VehicleRecalls};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page, jsonld, report, seo};
use crate::state::AppState;

/// Campaigns on the "Check recalls" page.
const NEWEST: usize = 10;

/// Most vehicles listed on a campaign's page. The JSON has them all.
const MOST_VEHICLES: usize = 200;

/// Most vehicles named in a campaign's structured data.
const MOST_SUBJECTS: usize = 20;

/// Who publishes a source's campaigns, in words.
fn publisher(source: &str) -> &'static str {
    match source {
        "tc" => "Transport Canada",
        _ => "NHTSA",
    }
}

/// What NHTSA's advisories on a campaign say, as a sentence. Empty when it
/// has none.
pub fn advisory(recall_do_not_drive: Option<bool>, recall_park_outside: Option<bool>) -> String {
    match (
        recall_do_not_drive == Some(true),
        recall_park_outside == Some(true),
    ) {
        (true, true) => {
            "NHTSA advises: do not drive, and park outside away from buildings, until the repair is done."
        }
        (true, false) => "NHTSA advises: do not drive until the repair is done.",
        (false, true) => {
            "NHTSA advises: park outside, away from buildings, until the repair is done."
        }
        (false, false) => "",
    }
    .to_owned()
}

/// One campaign in a list on a page.
pub struct Line {
    pub campaign: String,
    /// The campaign's page, as a path on this site.
    pub href: String,
    pub date: String,
    pub component: String,
    pub summary: String,
    /// NHTSA's advisory, when the campaign carries one. Empty otherwise.
    pub advisory: String,
    /// The build dates the campaign gives. Empty when it gives none.
    pub built: String,
}

/// The build dates of a campaign, in words.
fn built(from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => format!("Built {from} to {to}."),
        (Some(from), None) => format!("Built from {from}."),
        (None, Some(to)) => format!("Built to {to}."),
        (None, None) => String::new(),
    }
}

/// A campaign number is letters and digits. Anything else from the data
/// file does not go into an address.
fn campaign_path(recall: &Recall) -> Option<String> {
    let number = &recall.campaign;
    (!number.is_empty() && number.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .then(|| format!("/recalls/{}/{number}", recall.source))
}

fn line(recall: &Recall) -> Option<Line> {
    Some(Line {
        href: campaign_path(recall)?,
        campaign: recall.campaign.clone(),
        date: recall.date.clone().unwrap_or_default(),
        component: recall.component.clone().unwrap_or_default(),
        summary: recall.summary.clone().unwrap_or_default(),
        advisory: advisory(recall.do_not_drive, recall.park_outside),
        built: built(recall.built_from.as_deref(), recall.built_to.as_deref()),
    })
}

fn lines(recalls: &[Recall], source: &str) -> Vec<Line> {
    recalls
        .iter()
        .filter(|recall| recall.source == source)
        .filter_map(line)
        .collect()
}

/// "As of" for a country, or that the server has no data for it.
fn dated(day: Option<&String>) -> String {
    match day {
        Some(day) => format!("as of {day}"),
        None => "not on file here".to_owned(),
    }
}

/// A line of a Markdown list for one campaign.
fn markdown_line(line: &Line) -> String {
    let mut text = format!("- [{}]({}.md)", escape(&line.campaign), line.href);
    if !line.date.is_empty() {
        text.push_str(&format!(", {}", line.date));
    }
    if !line.component.is_empty() {
        text.push_str(&format!(": {}", escape(&line.component)));
    }
    if !line.summary.is_empty() {
        text.push_str(&format!(". {}", escape(&line.summary)));
    }
    if !line.built.is_empty() {
        text.push_str(&format!(" {}", escape(&line.built)));
    }
    if !line.advisory.is_empty() {
        text.push_str(&format!(" **{}**", escape(&line.advisory)));
    }
    text.push('\n');
    text
}

/// The end of every recall page in Markdown: where to check a VIN, and
/// the credit.
fn markdown_footer(lookup: &RecallLookup, attribution: &[String]) -> String {
    let mut text = String::from("\n## Check a VIN\n\n");
    text.push_str(&format!(
        "- [NHTSA recall lookup by VIN, United States]({})\n",
        lookup.nhtsa
    ));
    text.push_str(&format!(
        "- [Transport Canada recall search, Canada]({})\n",
        lookup.tc
    ));
    if let Some(maker) = &lookup.maker {
        text.push_str(&format!("- [The maker's own recall lookup]({maker})\n"));
    }
    for credit in attribution {
        text.push_str(&format!("\n{credit}\n"));
    }
    text
}

// ----- a model year's recalls, in brief -----

/// Campaigns shown where a page says of a model year's recalls in brief.
const BRIEF: usize = 3;

/// What another page says of a model year's recalls: how many may apply,
/// the newest, the advisories, and where the whole list is.
pub struct Brief {
    /// `2023 Hyundai Kona`.
    pub name: String,
    /// The model year's recall page.
    pub href: String,
    /// `4 recall campaigns may apply`, or that none was found.
    pub text: String,
    /// The newest campaigns, at most three.
    pub newest: Vec<Line>,
    /// Every campaign with an advisory, whether or not it is among the
    /// newest.
    pub advisories: Vec<Line>,
    /// When each country's data was current, in words.
    pub us: String,
    pub ca: String,
    pub lookup: RecallLookup,
}

/// The recalls of the model year `vehicle_id` names, in brief.
///
/// `None` on a server that holds no recalls, for an id that names no model
/// year, and when the recalls could not be read. The page that asked is
/// then shown exactly as it is without recalls: they are an addition to
/// it, never a reason for it to fail.
pub async fn brief(state: &AppState, vehicle_id: &str) -> Option<Brief> {
    let shop = recall_shop(state, None).ok()?;
    let id = vehicle_id.to_owned();
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(found)) => found?,
        Ok(Err(error)) => {
            tracing::warn!(%error, "recalls left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "recalls left off a page");
            return None;
        }
    };
    // A failure here is logged where it is turned into an `ApiError`.
    let answer = recalls_of(state, shop, &found, None).await.ok()?;
    let total = answer.recalls.len();
    let text = if total == 0 {
        "None found that may apply. Check the VIN to be sure.".to_owned()
    } else {
        format!("{} may apply", seo::count(total, "recall campaign"))
    };
    let all: Vec<Line> = answer.recalls.iter().filter_map(line).collect();
    let advisories: Vec<Line> = answer
        .recalls
        .iter()
        .filter_map(line)
        .filter(|line| !line.advisory.is_empty())
        .collect();
    Some(Brief {
        name: format!(
            "{} {} {}",
            answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
        ),
        href: format!(
            "/makes/{}/{}/{}/recalls",
            found.make_slug, found.model_slug, answer.vehicle.year
        ),
        text,
        newest: all.into_iter().take(BRIEF).collect(),
        advisories,
        us: dated(answer.as_of.us.as_ref()),
        ca: dated(answer.as_of.ca.as_ref()),
        lookup: answer.lookup,
    })
}

// ----- /recalls -----

#[derive(Template)]
#[template(path = "recalls.html")]
struct IndexPage {
    page: Page,
    years: Vec<u16>,
    makes: Vec<Make>,
    may_apply: &'static str,
    newest: Vec<Line>,
    us: String,
    ca: String,
    lookup: RecallLookup,
    attribution: Vec<String>,
}

/// "Check recalls": the VIN box, the picker, the official lookups and the
/// newest campaigns.
pub async fn index(State(state): State<AppState>) -> Response {
    if recall_shop(&state, None).is_err() {
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
    let asked = RecallSearchQuery {
        limit: Some(NEWEST),
        ..RecallSearchQuery::default()
    };
    let list = match recall_search_op(&state, asked).await {
        Ok(list) => list,
        Err(error) => return site::failed(&state, &error),
    };
    let newest: Vec<Line> = list.recalls.iter().filter_map(line).collect();
    let page = Page::new(
        &state,
        seo::title("Check a vehicle for recalls"),
        "Recalls from NHTSA and Transport Canada that may apply to a vehicle, by VIN or by year, make and model, with the official lookups to confirm one vehicle.",
    )
    .indexed(&state, "/recalls")
    .with_mono();
    site::html(
        StatusCode::OK,
        &IndexPage {
            page,
            years: state.years(Scope::Light).unwrap_or_default().to_vec(),
            makes: makes.into_iter().map(Make::from).collect(),
            may_apply: MAY_APPLY,
            newest,
            us: dated(list.as_of.us.as_ref()),
            ca: dated(list.as_of.ca.as_ref()),
            lookup: list.lookup,
            attribution: list.attribution,
        },
    )
}

// ----- a model year's recalls -----

/// One choice of the component filter: a link.
pub struct Filter {
    pub name: String,
    pub href: String,
    pub current: bool,
}

#[derive(Template)]
#[template(path = "recall_model_year.html")]
struct ModelYearRecallsPage {
    page: Page,
    /// `2023 Hyundai Kona`.
    name: String,
    /// The model-year page.
    model_year_path: String,
    may_apply: &'static str,
    /// The campaigns with an advisory, said before anything else.
    advisories: Vec<Line>,
    filters: Vec<Filter>,
    /// The component the list is narrowed to.
    chosen: Option<String>,
    us: Vec<Line>,
    ca: Vec<Line>,
    us_dated: String,
    ca_dated: String,
    lookup: RecallLookup,
    attribution: Vec<String>,
    report: report::ReportLinks,
}

#[derive(Debug, Default, Deserialize)]
pub struct ComponentQuery {
    component: Option<String>,
}

/// The systems the recalls of a model year name, each once, by name.
fn components(recalls: &[Recall]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for recall in recalls {
        for system in systems(recall.component.as_deref().unwrap_or_default()) {
            if !found
                .iter()
                .any(|known| known.eq_ignore_ascii_case(&system))
            {
                found.push(system);
            }
        }
    }
    found.sort_by_key(|system| system.to_ascii_lowercase());
    found
}

/// Whether a recall names `system` among its components.
fn names(recall: &Recall, system: &str) -> bool {
    systems(recall.component.as_deref().unwrap_or_default())
        .iter()
        .any(|found| found.eq_ignore_ascii_case(system))
}

fn model_year_markdown(
    name: &str,
    answer: &VehicleRecalls,
    report: &report::ReportLinks,
) -> String {
    let mut text = format!("# Recalls that may apply to the {}\n\n", escape(name));
    text.push_str(MAY_APPLY);
    text.push('\n');
    for (heading, source, day) in [
        ("United States (NHTSA)", "nhtsa", answer.as_of.us.as_ref()),
        ("Canada (Transport Canada)", "tc", answer.as_of.ca.as_ref()),
    ] {
        text.push_str(&format!("\n## {heading}, {}\n\n", dated(day)));
        let found = lines(&answer.recalls, source);
        if found.is_empty() {
            text.push_str(if day.is_some() {
                "None found.\n"
            } else {
                "This server holds no recalls for this country.\n"
            });
        }
        for line in &found {
            text.push_str(&markdown_line(line));
        }
    }
    text.push_str(&markdown_footer(&answer.lookup, &answer.attribution));
    text.push_str(&format!(
        "\nJSON: [/v1/vehicles/{id}/recalls](/v1/vehicles/{id}/recalls)\n",
        id = answer.vehicle.id
    ));
    text.push_str(&format!(
        "\n[Report an error on this page]({})\n",
        report.issue
    ));
    text
}

/// What is under a model year: `recalls` today.
pub async fn model_year_section(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String, String)>, PathRejection>,
    query: Result<Query<ComponentQuery>, QueryRejection>,
    uri: Uri,
) -> Response {
    let Ok(Path((make, model, year, segment))) = path else {
        return site::not_found(&state);
    };
    let (section, format) = markdown::split(&segment);
    let Some(year) = site::year_in(&year) else {
        return site::not_found(&state);
    };
    if section != "recalls" || !is_slug(&make) || !is_slug(&model) {
        return site::not_found(&state);
    }
    let Ok(shop) = recall_shop(&state, None) else {
        return site::not_found(&state);
    };
    let id = format!("{year:04}_{make}_{model}");
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(Some(found))) => found,
        Ok(Ok(None)) => return site::not_found(&state),
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let answer = match recalls_of(&state, shop, &found, None).await {
        Ok(answer) => answer,
        Err(error) => return site::failed(&state, &error),
    };
    let model_year_path = format!("/makes/{}/{}/{year}", found.make_slug, found.model_slug);
    let path = format!("{model_year_path}/recalls");
    let name = format!(
        "{} {} {}",
        answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
    );
    let report = report::links(state.config(), &path, &name);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(model_year_markdown(&name, &answer, &report), &canonical);
    }

    let all = components(&answer.recalls);
    // The filter is a link, so its value is one of the names on the page.
    // Anything else narrows nothing.
    let chosen = query
        .ok()
        .and_then(|Query(query)| query.component)
        .and_then(|asked| {
            all.iter()
                .find(|system| system.eq_ignore_ascii_case(asked.trim()))
                .cloned()
        });
    let mut filters = vec![Filter {
        name: "All components".to_owned(),
        href: path.clone(),
        current: chosen.is_none(),
    }];
    filters.extend(all.iter().map(|system| Filter {
        name: system.clone(),
        href: format!("{path}?component={}", report::encode(system)),
        current: chosen.as_deref() == Some(system.as_str()),
    }));
    let shown: Vec<Recall> = answer
        .recalls
        .iter()
        .filter(|recall| chosen.as_deref().is_none_or(|system| names(recall, system)))
        .cloned()
        .collect();
    let advisories: Vec<Line> = shown
        .iter()
        .filter_map(line)
        .filter(|line| !line.advisory.is_empty())
        .collect();

    let total = answer.recalls.len();
    let description = if total == 0 {
        format!(
            "No recall campaign for the {name} was found in the NHTSA or Transport Canada data. Check the VIN with the manufacturer to be sure."
        )
    } else {
        format!(
            "{} from NHTSA and Transport Canada may apply to the {name}. Check the VIN with the manufacturer to confirm.",
            seo::count(total, "recall campaign")
        )
    };
    let page = Page::new(&state, seo::title(&format!("{name} recalls")), description)
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
    let trail = page.trail("Recalls");
    let page = page.describing(vec![trail]);
    // The same rule as the model-year page, and one more: a page with no
    // campaign on it is there for whoever asks, and is in no sitemap. With
    // a query string the page is a filter of the one without.
    let listed = found.light && total > 0;
    let page = match (listed, uri.query().is_some()) {
        (true, false) => page.indexed(&state, &path),
        (true, true) => page.indexed(&state, &path).with_query(),
        (false, _) => page,
    };
    let indexed = page.index;
    let mut response = site::html(
        StatusCode::OK,
        &ModelYearRecallsPage {
            page,
            name,
            model_year_path,
            may_apply: MAY_APPLY,
            advisories,
            filters,
            chosen,
            us: lines(&shown, "nhtsa"),
            ca: lines(&shown, "tc"),
            us_dated: dated(answer.as_of.us.as_ref()),
            ca_dated: dated(answer.as_of.ca.as_ref()),
            lookup: answer.lookup,
            attribution: answer.attribution,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}

// ----- one campaign -----

/// One vehicle a campaign lists, as its page shows it.
pub struct VehicleLine {
    /// `2019 CHEVROLET SILVERADO 1500`, in the source's words.
    pub name: String,
    /// The model-year page, when the model is in the catalog.
    pub href: Option<String>,
    /// Build dates, and how the name reached the catalog's model.
    pub details: String,
}

#[derive(Template)]
#[template(path = "recall_campaign.html")]
struct CampaignPage {
    page: Page,
    heading: String,
    may_apply: &'static str,
    advisory: String,
    /// The facts of the campaign, as label and value.
    facts: Vec<(&'static str, String)>,
    /// The sections of text, as heading and paragraphs.
    sections: Vec<(&'static str, Vec<String>)>,
    vehicles: Vec<VehicleLine>,
    /// How many vehicles the campaign lists; `vehicles` holds at most 200.
    vehicles_total: usize,
    dated: String,
    lookup: RecallLookup,
    attribution: Vec<String>,
    json_path: String,
    report: report::ReportLinks,
}

/// Text as paragraphs: Transport Canada's comments are several lines.
fn paragraphs(text: Option<&String>) -> Vec<String> {
    text.map(|text| {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default()
}

fn facts(campaign: &RecallCampaign) -> Vec<(&'static str, String)> {
    let published = match campaign.source.as_str() {
        "tc" => "Transport Canada (Canada)",
        _ => "NHTSA (United States)",
    };
    let mut facts = vec![
        ("Published by", published.to_owned()),
        ("Campaign number", campaign.campaign.clone()),
    ];
    let mut push = |label: &'static str, value: Option<String>| {
        if let Some(value) = value {
            facts.push((label, value));
        }
    };
    push("Manufacturer's number", campaign.mfr_campaign.clone());
    push("Manufacturer", campaign.manufacturer.clone());
    push("Reported", campaign.date.clone());
    push("Owners notified", campaign.owner_notified.clone());
    push("Component", campaign.component.clone());
    push("Notification type", campaign.notification.clone());
    push(
        "Units potentially affected",
        campaign.units.map(|units| units.to_string()),
    );
    push("Part numbers", campaign.part_number.clone());
    facts
}

fn vehicle_lines(campaign: &RecallCampaign) -> Vec<VehicleLine> {
    campaign
        .vehicles
        .iter()
        .take(MOST_VEHICLES)
        .map(|vehicle| {
            let name = match vehicle.year {
                Some(year) => format!("{year} {} {}", vehicle.make, vehicle.model),
                None => format!("{} {}", vehicle.make, vehicle.model),
            };
            // The id is made of id forms, and is read back before any part
            // of it goes into an address.
            let href = vehicle
                .vehicle_id
                .as_deref()
                .and_then(VehicleId::parse)
                .map(|id| format!("/makes/{}/{}/{}", id.make, id.model, id.year));
            let mut details = built(vehicle.built_from.as_deref(), vehicle.built_to.as_deref());
            let note = match vehicle.matched.as_deref() {
                Some("base") => "Listed as a series or trim of this model.",
                Some("curated") => "Listed under a family name that covers this model.",
                _ => "",
            };
            if !note.is_empty() {
                if !details.is_empty() {
                    details.push(' ');
                }
                details.push_str(note);
            }
            VehicleLine {
                name,
                href,
                details,
            }
        })
        .collect()
}

fn campaign_markdown(
    heading: &str,
    campaign: &RecallCampaign,
    facts: &[(&'static str, String)],
    sections: &[(&'static str, Vec<String>)],
    report: &report::ReportLinks,
) -> String {
    let mut text = format!("# {}\n\n", escape(heading));
    text.push_str(MAY_APPLY);
    text.push_str("\n\n");
    let warned = advisory(campaign.do_not_drive, campaign.park_outside);
    if !warned.is_empty() {
        text.push_str(&format!("**{warned}**\n\n"));
    }
    for (label, value) in facts {
        text.push_str(&format!("- {label}: {}\n", escape(value)));
    }
    for (title, body) in sections {
        text.push_str(&format!("\n## {title}\n\n"));
        for paragraph in body {
            text.push_str(&format!("{}\n\n", escape(paragraph)));
        }
    }
    text.push_str("\n## Vehicles this campaign lists\n\n");
    for vehicle in vehicle_lines(campaign) {
        match &vehicle.href {
            Some(href) => {
                text.push_str(&format!("- [{}]({href}/recalls.md)", escape(&vehicle.name)))
            }
            None => text.push_str(&format!("- {}", escape(&vehicle.name))),
        }
        if !vehicle.details.is_empty() {
            text.push_str(&format!(". {}", escape(&vehicle.details)));
        }
        text.push('\n');
    }
    if campaign.vehicles.len() > MOST_VEHICLES {
        text.push_str(&format!(
            "\nThe first {MOST_VEHICLES} of {} vehicles are shown. The JSON has them all.\n",
            campaign.vehicles.len()
        ));
    }
    text.push_str(&markdown_footer(&campaign.lookup, &campaign.attribution));
    text.push_str(&format!(
        "\nJSON: [/v1/recalls/{source}/{number}](/v1/recalls/{source}/{number})\n",
        source = campaign.source,
        number = campaign.campaign
    ));
    text.push_str(&format!(
        "\n[Report an error on this page]({})\n",
        report.issue
    ));
    text
}

pub async fn campaign(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Response {
    let Ok(Path((source, segment))) = path else {
        return site::not_found(&state);
    };
    let (number, format) = markdown::split(&segment);
    // One campaign, one address: its number as the publisher writes it.
    if RecallSource::parse(&source).is_none() || number != number.to_ascii_uppercase() {
        return site::not_found(&state);
    }
    let (found, light) = match campaign_and_reach(&state, &source, number).await {
        Ok(found) => found,
        Err(ApiError::NotFound(_) | ApiError::DatasetUnavailable(_)) => {
            return site::not_found(&state);
        }
        Err(error) => return site::failed(&state, &error),
    };
    let path = format!("/recalls/{}/{}", found.source, found.campaign);
    let heading = format!("{} recall {}", publisher(&found.source), found.campaign);
    let facts = facts(&found);
    let sections: Vec<(&'static str, Vec<String>)> = [
        ("What is wrong", paragraphs(found.summary.as_ref())),
        ("What can happen", paragraphs(found.consequence.as_ref())),
        ("The remedy", paragraphs(found.remedy.as_ref())),
        ("Notes", paragraphs(found.notes.as_ref())),
    ]
    .into_iter()
    .filter(|(_, body)| !body.is_empty())
    .collect();
    let report = report::links(state.config(), &path, &heading);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(
            campaign_markdown(&heading, &found, &facts, &sections, &report),
            &canonical,
        );
    }

    let vehicles = vehicle_lines(&found);
    let named: Vec<String> = vehicles
        .iter()
        .take(MOST_SUBJECTS)
        .map(|vehicle| vehicle.name.clone())
        .collect();
    let what = found
        .component
        .clone()
        .unwrap_or_else(|| "a safety defect".to_owned());
    let description = seo::description(
        &[
            &[
                format!(
                    "{heading}, about {what}, may apply to {}.",
                    seo::count(found.vehicles.len(), "listed vehicle")
                ),
                format!("{heading} may apply to the vehicles it lists."),
            ],
            &["Check the VIN with the manufacturer to confirm.".to_owned()],
        ],
        160,
    );
    let page = Page::new(&state, seo::title(&heading), description.clone())
        .with_markdown(&format!("{path}.md"))
        .with_mono()
        .as_article()
        .under(vec![("Recalls".to_owned(), "/recalls".to_owned())]);
    let things = vec![
        page.trail(&heading),
        jsonld::recall(
            &state.config().base_url,
            &path,
            &heading,
            &description,
            found.date.as_deref(),
            &named,
        ),
    ];
    let page = page.describing(things);
    // A campaign that lists no car, MPV or truck of the catalog has a page
    // for whoever asks for it, and is not offered to search engines.
    let page = if light {
        page.indexed(&state, &path)
    } else {
        page
    };
    let indexed = page.index;
    let mut response = site::html(
        StatusCode::OK,
        &CampaignPage {
            page,
            heading,
            may_apply: MAY_APPLY,
            advisory: advisory(found.do_not_drive, found.park_outside),
            facts,
            sections,
            vehicles,
            vehicles_total: found.vehicles.len(),
            dated: dated(found.as_of.as_ref()),
            json_path: format!("/v1/recalls/{}/{}", found.source, found.campaign),
            lookup: found.lookup,
            attribution: found.attribution,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_advisory_is_said_in_nhtsas_name_and_only_when_set() {
        assert_eq!(advisory(Some(false), Some(false)), "");
        assert_eq!(advisory(None, None), "");
        assert_eq!(
            advisory(Some(true), Some(false)),
            "NHTSA advises: do not drive until the repair is done."
        );
        assert_eq!(
            advisory(Some(false), Some(true)),
            "NHTSA advises: park outside, away from buildings, until the repair is done."
        );
        assert!(advisory(Some(true), Some(true)).contains("do not drive, and park outside"));
    }

    #[test]
    fn build_dates_are_said_in_words() {
        assert_eq!(
            built(Some("2018-07-11"), Some("2019-11-22")),
            "Built 2018-07-11 to 2019-11-22."
        );
        assert_eq!(built(Some("2018-07-11"), None), "Built from 2018-07-11.");
        assert_eq!(built(None, Some("2019-11-22")), "Built to 2019-11-22.");
        assert_eq!(built(None, None), "");
    }

    #[test]
    fn text_of_several_lines_is_several_paragraphs() {
        let text = "Issue:\nThe nut may be loose.\n\n  Corrective Actions:  \nNone.".to_owned();
        assert_eq!(
            paragraphs(Some(&text)),
            [
                "Issue:",
                "The nut may be loose.",
                "Corrective Actions:",
                "None."
            ]
        );
        assert!(paragraphs(None).is_empty());
    }
}
