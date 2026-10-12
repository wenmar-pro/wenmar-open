//! Recall answers as the hosted API words them, built from a shop file and
//! the catalog of the core data file it was matched against.
//!
//! The command-line tool and the offline engine answer from files with
//! this module, so that what they print is what `/v1` sends for the same
//! question. The hosted server has its own copy of the same steps in
//! `open-server/src/api/recalls.rs`, where the types also describe
//! themselves to OpenAPI; a test of that crate asks both the same questions
//! and compares the JSON.
//!
//! Nothing here says a recall applies to a vehicle. Every answer has
//! `may_apply: true` and the official lookups, where a VIN can be checked.
//!
//! One thing differs from the hosted API: `lookup.maker` is always `null`,
//! and so is `links.maker` of an overview. The makers' own links are
//! compiled into the server from `data/catalog/makes.yaml` and are in
//! neither data file.
//!
//! [`Overviews`] builds the answers of `open-server/src/api/overview.rs`
//! that need no shop file: the overview of a model year and of a VIN, and
//! the model years of a model. With no shop file, or one without recalls,
//! their recall parts are `null` and they still answer.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value as Json;
use wenmar_vehicles::text::{is_slug, normalize};
use wenmar_vehicles::{Catalog, CatalogError, Scope, Source, Value, VehicleId};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::recalls::{
    MOST_RECALLS, RecallCursor, RecallQuery, RecallSource, RecallSummary, VehicleRecall,
};
use crate::shop::ShopData;

/// The hosted site. A campaign's `url` is its page there.
pub const SITE: &str = "https://open.wenmarpro.com";

/// NHTSA's recall lookup by VIN, for the United States.
pub const NHTSA_LOOKUP: &str = "https://www.nhtsa.gov/recalls";

/// Transport Canada's recall search, for Canada.
pub const TC_LOOKUP: &str =
    "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng";

/// What a VIN's recall answer says when the VIN names no model year.
pub const NO_CATALOG_VEHICLE: &str = "This VIN does not decode to a year, make and model in the catalog, so no recalls are listed for it. Check the VIN with the official lookups.";

/// Campaigns in a list when `limit` is not given.
pub const LIST_DEFAULT: usize = 25;

/// Longest campaign number read.
pub const LONGEST_CAMPAIGN: usize = 20;

/// `?1` make id, `?2` the model's id form, `?3` year.
pub const MODEL_YEAR_SQL: &str = "
SELECT md.id, v.light
FROM catalog_model md
JOIN catalog_vehicle v ON v.model_id = md.id
WHERE md.make_id = ?1 AND md.slug = ?2 AND v.year = ?3";

/// `?1` make id, `?2` the model as an id form, `?3` its matching form.
pub const MODEL_ID_SQL: &str = "
SELECT id FROM catalog_model
WHERE make_id = ?1 AND (slug = ?2 OR norm = ?3)
ORDER BY (slug = ?2) DESC, id
LIMIT 1";

/// `?1` make id, `?2` the model as an id form, `?3` its matching form.
pub const MODEL_SQL: &str = "
SELECT id, slug, name FROM catalog_model
WHERE make_id = ?1 AND (slug = ?2 OR norm = ?3)
ORDER BY (slug = ?2) DESC, id
LIMIT 1";

/// `?1` model id.
pub const MODEL_YEARS_SQL: &str = "
SELECT DISTINCT year FROM catalog_vehicle
WHERE model_id = ?1
ORDER BY year DESC";

/// Campaigns in the `newest` list of an overview's recall block.
pub const NEWEST: usize = 3;

/// Most vehicles given with one campaign of the recent list.
pub const MOST_VEHICLES_EACH: usize = 200;

/// Why a question could not be answered. Each is an error of the hosted
/// API: `validation_failed`, `not_found`, `dataset_unavailable`, or a file
/// that could not be read.
#[derive(Debug, thiserror::Error)]
pub enum AnswerError {
    /// 400 `validation_failed`, with `details.field`.
    #[error("{message}")]
    Validation {
        field: &'static str,
        message: &'static str,
    },
    /// 404 `not_found`.
    #[error("{0}")]
    NotFound(&'static str),
    /// [`ShopError::DatasetUnavailable`] is 404 `dataset_unavailable`.
    /// Anything else is a shop file that could not be read.
    #[error(transparent)]
    Shop(#[from] ShopError),
    /// The core data file could not be read.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

/// Where to check one vehicle for open recalls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lookup {
    pub nhtsa: String,
    pub tc: String,
    /// Always `null` here: see the module's own words.
    pub maker: Option<String>,
}

/// The day each country's recall data was current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsOf {
    pub us: Option<String>,
    pub ca: Option<String>,
}

/// The model year an answer is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Vehicle {
    pub id: String,
    pub year: u16,
    pub make: String,
    pub model: String,
}

/// One recall campaign in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recall {
    pub source: String,
    pub country: String,
    pub campaign: String,
    pub date: Option<String>,
    pub manufacturer: Option<String>,
    pub component: Option<String>,
    pub summary: Option<String>,
    pub mfr_campaign: Option<String>,
    pub units: Option<u32>,
    pub do_not_drive: Option<bool>,
    pub park_outside: Option<bool>,
    pub notification: Option<String>,
    #[serde(rename = "match")]
    pub matched: Option<String>,
    pub models: Vec<String>,
    pub built_from: Option<String>,
    pub built_to: Option<String>,
    pub url: String,
}

/// Recalls that may apply to one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleRecalls {
    pub vehicle: Vehicle,
    /// Always `true`.
    pub may_apply: bool,
    pub as_of: AsOf,
    pub recalls: Vec<Recall>,
    pub lookup: Lookup,
    pub attribution: Vec<String>,
}

/// What the VIN decoded to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decode {
    pub vin: String,
    pub valid: bool,
    pub year: Option<u16>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub trim: Option<String>,
}

/// Recalls that may apply to the model year a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VinRecalls {
    pub decode: Decode,
    pub vehicle: Option<Vehicle>,
    /// Always `true`.
    pub may_apply: bool,
    pub as_of: AsOf,
    pub recalls: Vec<Recall>,
    pub lookup: Lookup,
    pub attribution: Vec<String>,
    /// The decode's warnings as they are, then `no_catalog_vehicle`.
    pub warnings: Vec<Json>,
}

/// One vehicle a campaign lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampaignVehicle {
    pub year: Option<u16>,
    pub make: String,
    pub model: String,
    pub vehicle_id: Option<String>,
    #[serde(rename = "match")]
    pub matched: Option<String>,
    pub built_from: Option<String>,
    pub built_to: Option<String>,
}

/// Transport Canada's French text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct French {
    pub component: Option<String>,
    pub notification: Option<String>,
    pub summary: Option<String>,
}

/// The newest report of remedy progress a manufacturer filed with NHTSA
/// for a campaign: the API's `completion`. Totals for the whole campaign at
/// the end of the period named, never a statement about one vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Completion {
    pub kind: String,
    pub report_number: u32,
    pub period_from: String,
    pub period_to: String,
    pub submitted: Option<String>,
    pub involved: u32,
    pub remedied: u32,
    pub unreachable: u32,
    pub removed: u32,
    pub remedied_percent: Option<u8>,
    pub as_of: Option<String>,
}

/// One recall campaign with every vehicle it lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Campaign {
    pub source: String,
    pub country: String,
    pub campaign: String,
    pub date: Option<String>,
    pub owner_notified: Option<String>,
    pub manufacturer: Option<String>,
    pub component: Option<String>,
    pub summary: Option<String>,
    pub consequence: Option<String>,
    pub remedy: Option<String>,
    pub notes: Option<String>,
    pub mfr_campaign: Option<String>,
    pub part_number: Option<String>,
    pub units: Option<u32>,
    pub completion: Option<Completion>,
    pub do_not_drive: Option<bool>,
    pub park_outside: Option<bool>,
    pub notification: Option<String>,
    pub french: Option<French>,
    pub vehicles: Vec<CampaignVehicle>,
    /// Always `true`.
    pub may_apply: bool,
    pub as_of: Option<String>,
    pub lookup: Lookup,
    pub attribution: Vec<String>,
    pub url: String,
}

/// One page of recall campaigns, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct List {
    /// Always `true`.
    pub may_apply: bool,
    pub as_of: AsOf,
    pub recalls: Vec<Recall>,
    pub next_cursor: Option<String>,
    pub lookup: Lookup,
    pub attribution: Vec<String>,
}

/// What a list of campaigns is narrowed by, as the API's query string
/// names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<u16>,
    pub component: Option<String>,
    pub q: Option<String>,
    pub country: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// How many recall campaigns list a model year, for each country. `null`
/// for a country the shop file holds no recalls for; `0` when it holds
/// them and none lists the model year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Count {
    pub us: Option<u32>,
    pub ca: Option<u32>,
}

/// The recalls of a model year, in brief: the `recalls` of an overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewRecalls {
    /// Always `true`.
    pub may_apply: bool,
    pub count: Count,
    pub as_of: AsOf,
    pub do_not_drive: bool,
    pub park_outside: bool,
    /// The three newest campaigns.
    pub newest: Vec<Recall>,
    pub lookup: Lookup,
    /// The model year's recall page on the hosted site.
    pub url: String,
}

/// What the VIN of an overview decoded to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewDecode {
    pub vin: String,
    pub valid: bool,
    pub year: Option<u16>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub trim: Option<String>,
    pub engine: Option<String>,
    pub engine_settled: bool,
    pub entry_id: Option<String>,
}

/// Where to read more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Links {
    /// The model year's page on the hosted site.
    pub page: Option<String>,
    /// Always `null` here: see the module's own words.
    pub maker: Option<Json>,
}

/// Everything held for one model year, in one answer. A block with no
/// data is `null`. `specs`, `fuel`, `fluids` and `safety` are always
/// `null` here: the hosted API fills them and the files do not yet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Overview {
    pub vehicle: Option<Vehicle>,
    pub decode: Option<OverviewDecode>,
    pub specs: Option<Json>,
    pub fuel: Option<Json>,
    pub fluids: Option<Json>,
    pub tires: Option<Json>,
    pub recalls: Option<OverviewRecalls>,
    pub safety: Option<Json>,
    pub labor: Option<Json>,
    pub bulletins: Option<Json>,
    pub links: Links,
    pub attribution: Vec<String>,
    /// The decode's warnings as they are, then `no_catalog_vehicle`.
    pub warnings: Vec<Json>,
}

/// A make or a model: its id form and its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamedId {
    pub id: String,
    pub name: String,
}

/// One model year of a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelYearSummary {
    pub id: String,
    pub year: u16,
    /// `null` when the files hold no recalls.
    pub recalls: Option<Count>,
    /// Always `null`: bulletins are not in the files.
    pub bulletins: Option<u32>,
    pub url: String,
}

/// Every model year of one model, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelYears {
    pub make: NamedId,
    pub model: NamedId,
    /// Always `true`.
    pub may_apply: bool,
    pub as_of: AsOf,
    pub years: Vec<ModelYearSummary>,
    pub lookup: Lookup,
    pub attribution: Vec<String>,
}

/// One campaign of the recent list, with the vehicles it lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentRecall {
    pub recall: Recall,
    /// At most [`MOST_VEHICLES_EACH`] of them.
    pub vehicles: Vec<CampaignVehicle>,
    pub vehicles_listed: u32,
}

/// Campaigns reported on or after a day, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentRecalls {
    /// Always `true`.
    pub may_apply: bool,
    pub since: String,
    pub as_of: AsOf,
    pub recalls: Vec<RecentRecall>,
    pub next_cursor: Option<String>,
    pub lookup: Lookup,
    pub attribution: Vec<String>,
}

/// What the recent list is asked for, as the API's query string names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recent {
    pub since: Option<String>,
    pub country: Option<String>,
    pub make: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// Reads `country`: `us`, `ca` (either case), or nothing for both.
pub fn country(text: Option<&str>) -> Result<Option<RecallSource>, AnswerError> {
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(None),
        Some(text) => RecallSource::from_country(&text.to_ascii_lowercase())
            .map(Some)
            .ok_or(AnswerError::Validation {
                field: "country",
                message: "country must be us or ca",
            }),
    }
}

/// The recall dataset a question needs first, for saying which is missing
/// when there is no shop file at all: the country or the publisher asked
/// for, and the United States' when neither is.
pub fn dataset_asked(country: Option<&str>, source: Option<&str>) -> Dataset {
    let asked = |text: Option<&str>| text.map(|text| text.trim().to_ascii_lowercase());
    match (asked(country).as_deref(), asked(source).as_deref()) {
        (Some("ca"), _) | (_, Some("tc")) => Dataset::RecallsCa,
        _ => Dataset::RecallsUs,
    }
}

/// The official lookups.
pub fn lookup() -> Lookup {
    Lookup {
        nhtsa: NHTSA_LOOKUP.to_owned(),
        tc: TC_LOOKUP.to_owned(),
        maker: None,
    }
}

/// Text a caller gave, or `None` when it is blank.
fn given(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// A model year of the catalog, with the ids the shop file knows it by.
struct ModelYear {
    vehicle: Vehicle,
    make_id: i64,
    model_id: i64,
    make_slug: String,
    model_slug: String,
}

/// The id forms of a catalog model and its make.
struct ModelRef {
    make: String,
    model: String,
}

/// A shop file, the catalog of the core data file it was matched against,
/// and that file itself for the two lookups the catalog has no method for.
pub struct Answers<'a, S, C> {
    pub shop: &'a ShopData<S>,
    pub catalog: &'a Catalog<C>,
    pub core: &'a C,
    /// The site a campaign's `url` points at: [`SITE`].
    pub base: &'a str,
}

impl<S: Source, C: Source> Answers<'_, S, C> {
    fn as_of(&self) -> AsOf {
        AsOf {
            us: self
                .shop
                .recalls_as_of(RecallSource::Nhtsa)
                .map(str::to_owned),
            ca: self.shop.recalls_as_of(RecallSource::Tc).map(str::to_owned),
        }
    }

    /// The credit lines of the recall datasets an answer drew on.
    fn attribution(&self, source: Option<RecallSource>) -> Vec<String> {
        RecallSource::ALL
            .into_iter()
            .filter(|held| source.is_none_or(|asked| asked == *held))
            .filter_map(|held| self.shop.datasets().get(held.dataset()))
            .map(|entry| entry.attribution.clone())
            .filter(|credit| !credit.is_empty())
            .collect()
    }

    /// The newest report of remedy progress for an NHTSA campaign. `None`
    /// for a Canadian campaign, for a campaign with no report, and when
    /// the shop file holds no reports.
    fn completion(
        &self,
        source: RecallSource,
        campaign: &str,
    ) -> Result<Option<Completion>, AnswerError> {
        if source != RecallSource::Nhtsa || self.shop.require(Dataset::RecallReports).is_err() {
            return Ok(None);
        }
        let as_of = self.shop.recall_reports_as_of().map(str::to_owned);
        Ok(self.shop.recall_report(campaign)?.map(|report| Completion {
            remedied_percent: report.remedied_percent(),
            kind: report.kind,
            report_number: report.report_number,
            period_from: report.period_from,
            period_to: report.period_to,
            submitted: report.submitted,
            involved: report.involved,
            remedied: report.remedied,
            unreachable: report.unreachable,
            removed: report.removed,
            as_of,
        }))
    }

    fn listed(&self, recall: RecallSummary) -> Recall {
        Recall {
            url: format!(
                "{}/recalls/{}/{}",
                self.base, recall.source, recall.campaign
            ),
            source: recall.source,
            country: recall.country,
            campaign: recall.campaign,
            date: recall.date,
            manufacturer: recall.manufacturer,
            component: recall.component,
            summary: recall.summary,
            mfr_campaign: recall.mfr_campaign,
            units: recall.units,
            do_not_drive: recall.do_not_drive,
            park_outside: recall.park_outside,
            notification: recall.notification,
            matched: None,
            models: Vec::new(),
            built_from: None,
            built_to: None,
        }
    }

    fn for_vehicle(&self, found: VehicleRecall) -> Recall {
        let mut recall = self.listed(found.recall);
        recall.matched = Some(found.matched);
        recall.models = found.models;
        recall.built_from = found.built_from;
        recall.built_to = found.built_to;
        recall
    }

    /// The model year a vehicle id names. A submodel or an engine in the
    /// id must exist, and is then set aside.
    fn model_year(&self, id: &str) -> Result<Option<ModelYear>, AnswerError> {
        model_year_in(self.catalog, self.core, id)
    }

    /// Catalog models by id. Ids the catalog lacks are left out.
    fn model_slugs(&self, model_ids: &[i64]) -> Result<HashMap<i64, ModelRef>, AnswerError> {
        let mut found = HashMap::new();
        for chunk in model_ids.chunks(400) {
            // The ids are numbers from the shop file, so they are written
            // into the statement as they are.
            let list: Vec<String> = chunk.iter().map(i64::to_string).collect();
            let sql = format!(
                "SELECT md.id, mk.slug, md.slug, md.light FROM catalog_model md JOIN catalog_make mk ON mk.id = md.make_id WHERE md.id IN ({})",
                list.join(",")
            );
            for row in self.core.query(&sql, &[]).map_err(CatalogError::Source)? {
                if let (Some(id), Some(make), Some(model)) = (
                    row.first().and_then(Value::integer),
                    row.get(1).and_then(Value::text),
                    row.get(2).and_then(Value::text),
                ) && is_slug(make)
                    && is_slug(model)
                {
                    found.insert(
                        id,
                        ModelRef {
                            make: make.to_owned(),
                            model: model.to_owned(),
                        },
                    );
                }
            }
        }
        Ok(found)
    }

    fn recalls_of(
        &self,
        found: &ModelYear,
        source: Option<RecallSource>,
    ) -> Result<VehicleRecalls, AnswerError> {
        let recalls = self.shop.recalls_for_vehicle(
            found.make_id,
            found.model_id,
            found.vehicle.year,
            source,
        )?;
        Ok(VehicleRecalls {
            vehicle: found.vehicle.clone(),
            may_apply: true,
            as_of: self.as_of(),
            recalls: recalls
                .into_iter()
                .map(|recall| self.for_vehicle(recall))
                .collect(),
            lookup: lookup(),
            attribution: self.attribution(source),
        })
    }

    /// `GET /v1/vehicles/{id}/recalls`.
    pub fn vehicle(
        &self,
        id: &str,
        country_text: Option<&str>,
    ) -> Result<VehicleRecalls, AnswerError> {
        let source = country(country_text)?;
        self.shop.require_recalls(source)?;
        let Some(found) = self.model_year(id)? else {
            return Err(AnswerError::NotFound("No vehicle has that id."));
        };
        self.recalls_of(&found, source)
    }

    /// `GET /v1/vin/{vin}/recalls`. `decode` is the JSON of the decode, as
    /// `GET /v1/vin/{vin}` gives it, with its `catalog` when it has one.
    pub fn vin(
        &self,
        decode: &Json,
        country_text: Option<&str>,
    ) -> Result<VinRecalls, AnswerError> {
        let source = country(country_text)?;
        self.shop.require_recalls(source)?;
        let text = |key: &str| decode.get(key).and_then(Json::as_str).map(str::to_owned);
        let summary = Decode {
            vin: text("vin").unwrap_or_default(),
            valid: decode.get("valid").and_then(Json::as_bool).unwrap_or(false),
            year: decode
                .get("year")
                .and_then(Json::as_u64)
                .and_then(|year| u16::try_from(year).ok()),
            make: text("make"),
            model: text("model"),
            trim: text("trim"),
        };
        let mut warnings = decode
            .get("warnings")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        let found = match decode
            .get("catalog")
            .and_then(|catalog| catalog.get("vehicle_id"))
            .and_then(Json::as_str)
        {
            Some(id) => self.model_year(id)?,
            None => None,
        };
        let Some(found) = found else {
            // Not an error: the VIN decoded, there is only no model year
            // to list recalls for.
            warnings.push(serde_json::json!({
                "code": "no_catalog_vehicle",
                "message": NO_CATALOG_VEHICLE,
            }));
            return Ok(VinRecalls {
                decode: summary,
                vehicle: None,
                may_apply: true,
                as_of: self.as_of(),
                recalls: Vec::new(),
                lookup: lookup(),
                attribution: self.attribution(source),
                warnings,
            });
        };
        let answer = self.recalls_of(&found, source)?;
        Ok(VinRecalls {
            decode: summary,
            vehicle: Some(answer.vehicle),
            may_apply: true,
            as_of: answer.as_of,
            recalls: answer.recalls,
            lookup: answer.lookup,
            attribution: answer.attribution,
            warnings,
        })
    }

    /// `GET /v1/recalls/{source}/{campaign}`.
    pub fn campaign(&self, source: &str, campaign: &str) -> Result<Campaign, AnswerError> {
        let missing = || AnswerError::NotFound("No recall campaign has that number.");
        let Some(source) = RecallSource::parse(source) else {
            return Err(missing());
        };
        self.shop.require(source.dataset())?;
        let number = campaign.trim();
        if number.is_empty()
            || number.len() > LONGEST_CAMPAIGN
            || !number.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(missing());
        }
        let Some(detail) = self.shop.recall(source, number)? else {
            return Err(missing());
        };
        let mut model_ids: Vec<i64> = detail
            .vehicles
            .iter()
            .filter_map(|vehicle| vehicle.model_id)
            .collect();
        model_ids.sort_unstable();
        model_ids.dedup();
        let slugs = self.model_slugs(&model_ids)?;
        let vehicles = detail
            .vehicles
            .into_iter()
            .map(|vehicle| campaign_vehicle(vehicle, &slugs))
            .collect();
        let has_french = detail.component_fr.is_some()
            || detail.notification_fr.is_some()
            || detail.summary_fr.is_some();
        let french = has_french.then_some(French {
            component: detail.component_fr,
            notification: detail.notification_fr,
            summary: detail.summary_fr,
        });
        let recall = detail.recall;
        let completion = self.completion(source, &recall.campaign)?;
        Ok(Campaign {
            url: format!(
                "{}/recalls/{}/{}",
                self.base, recall.source, recall.campaign
            ),
            source: recall.source,
            country: recall.country,
            campaign: recall.campaign,
            date: recall.date,
            owner_notified: detail.owner_notified,
            manufacturer: recall.manufacturer,
            component: recall.component,
            summary: recall.summary,
            consequence: detail.consequence,
            remedy: detail.remedy,
            notes: detail.notes,
            mfr_campaign: recall.mfr_campaign,
            part_number: detail.part_number,
            units: recall.units,
            completion,
            do_not_drive: recall.do_not_drive,
            park_outside: recall.park_outside,
            notification: recall.notification,
            french,
            vehicles,
            may_apply: true,
            as_of: self.shop.recalls_as_of(source).map(str::to_owned),
            lookup: lookup(),
            attribution: self.attribution(Some(source)),
        })
    }

    /// `GET /v1/recalls`.
    pub fn search(&self, query: &Search) -> Result<List, AnswerError> {
        let source = country(query.country.as_deref())?;
        self.shop.require_recalls(source)?;
        let (make, model) = (given(query.make.as_ref()), given(query.model.as_ref()));
        if model.is_some() && make.is_none() {
            return Err(AnswerError::Validation {
                field: "make",
                message: "model needs make",
            });
        }
        let cursor = match given(query.cursor.as_ref()) {
            None => None,
            Some(text) => Some(RecallCursor::parse(&text).ok_or(AnswerError::Validation {
                field: "cursor",
                message: "cursor is not one this API gave out",
            })?),
        };
        let empty = List {
            may_apply: true,
            as_of: self.as_of(),
            recalls: Vec::new(),
            next_cursor: None,
            lookup: lookup(),
            attribution: self.attribution(source),
        };
        // A make or a model the catalog does not have lists nothing.
        let (make_id, model_id) = match make {
            None => (None, None),
            Some(make) => {
                let Some(found) = self.catalog.index().resolve(&make, Scope::All) else {
                    return Ok(empty);
                };
                match model {
                    None => (Some(found.id), None),
                    Some(model) => {
                        let rows = self
                            .core
                            .query(
                                MODEL_ID_SQL,
                                &[
                                    found.id.into(),
                                    model.trim().into(),
                                    normalize(&model).as_str().into(),
                                ],
                            )
                            .map_err(CatalogError::Source)?;
                        let Some(model_id) = rows
                            .first()
                            .and_then(|row| row.first())
                            .and_then(Value::integer)
                        else {
                            return Ok(empty);
                        };
                        (Some(found.id), Some(model_id))
                    }
                }
            }
        };
        let page = self.shop.recalls(&RecallQuery {
            source,
            make_id,
            model_id,
            year: query.year,
            component: given(query.component.as_ref()),
            q: given(query.q.as_ref()),
            since: None,
            limit: query.limit.unwrap_or(LIST_DEFAULT).clamp(1, MOST_RECALLS),
            cursor,
        })?;
        Ok(List {
            recalls: page
                .recalls
                .into_iter()
                .map(|recall| self.listed(recall))
                .collect(),
            next_cursor: page.next_cursor.map(|cursor| cursor.to_string()),
            ..empty
        })
    }
}

/// Whether `text` is a day written `2026-09-01`.
fn is_day(text: &str) -> bool {
    text.len() == 10
        && text.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

/// How many campaigns of a model year's list each country has, and whether
/// any carries the advisory not to drive, or to park outside. A country
/// the shop file holds no recalls for is not counted: its count is `None`.
pub fn tally(recalls: &[Recall], held: &AsOf) -> (Count, bool, bool) {
    let of = |country: &str| {
        let count = recalls
            .iter()
            .filter(|recall| recall.country == country)
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    };
    (
        Count {
            us: held.us.as_ref().map(|_| of("us")),
            ca: held.ca.as_ref().map(|_| of("ca")),
        },
        recalls
            .iter()
            .any(|recall| recall.do_not_drive == Some(true)),
        recalls
            .iter()
            .any(|recall| recall.park_outside == Some(true)),
    )
}

/// One vehicle a campaign lists, with its vehicle id when it matched a
/// catalog model and has a year.
fn campaign_vehicle(
    vehicle: crate::recalls::RecallVehicle,
    slugs: &HashMap<i64, ModelRef>,
) -> CampaignVehicle {
    let model = vehicle.model_id.and_then(|id| slugs.get(&id));
    let vehicle_id = match (vehicle.year, model) {
        (Some(year), Some(found)) => Some(format!("{year:04}_{}_{}", found.make, found.model)),
        _ => None,
    };
    CampaignVehicle {
        year: vehicle.year,
        make: vehicle.make_text,
        model: vehicle.model_text,
        vehicle_id,
        matched: vehicle.matched,
        built_from: vehicle.built_from,
        built_to: vehicle.built_to,
    }
}

/// The model year a vehicle id names, from the core data file alone. A
/// submodel or an engine in the id must exist, and is then set aside.
/// `None` when no vehicle has that id.
fn model_year_in<C: Source>(
    catalog: &Catalog<C>,
    core: &C,
    id: &str,
) -> Result<Option<ModelYear>, AnswerError> {
    let Some(entry) = catalog.entry(id)? else {
        return Ok(None);
    };
    let Some(parsed) = VehicleId::parse(&entry.id) else {
        return Ok(None);
    };
    let Some(make) = catalog.index().by_slug(&parsed.make) else {
        return Ok(None);
    };
    let rows = core
        .query(
            MODEL_YEAR_SQL,
            &[
                make.id.into(),
                parsed.model.as_str().into(),
                i64::from(parsed.year).into(),
            ],
        )
        .map_err(CatalogError::Source)?;
    let Some(model_id) = rows
        .first()
        .and_then(|row| row.first())
        .and_then(Value::integer)
    else {
        return Ok(None);
    };
    Ok(Some(ModelYear {
        vehicle: Vehicle {
            id: format!("{:04}_{}_{}", parsed.year, parsed.make, parsed.model),
            year: parsed.year,
            make: entry.make,
            model: entry.model,
        },
        make_id: make.id,
        model_id,
        make_slug: parsed.make,
        model_slug: parsed.model,
    }))
}

impl<S: Source, C: Source> Answers<'_, S, C> {
    /// `GET /v1/recalls/recent`.
    pub fn recent(&self, query: &Recent) -> Result<RecentRecalls, AnswerError> {
        let source = country(query.country.as_deref())?;
        self.shop.require_recalls(source)?;
        let since = match given(query.since.as_ref()) {
            Some(since) if is_day(&since) => since,
            Some(_) => {
                return Err(AnswerError::Validation {
                    field: "since",
                    message: "since must be a day written like 2026-09-01",
                });
            }
            None => {
                return Err(AnswerError::Validation {
                    field: "since",
                    message: "since is required",
                });
            }
        };
        let cursor = match given(query.cursor.as_ref()) {
            None => None,
            Some(text) => Some(RecallCursor::parse(&text).ok_or(AnswerError::Validation {
                field: "cursor",
                message: "cursor is not one this API gave out",
            })?),
        };
        let mut answer = RecentRecalls {
            may_apply: true,
            since: since.clone(),
            as_of: self.as_of(),
            recalls: Vec::new(),
            next_cursor: None,
            lookup: lookup(),
            attribution: self.attribution(source),
        };
        // A make the catalog does not have lists nothing, as in `search`.
        let make_id = match given(query.make.as_ref()) {
            None => None,
            Some(make) => match self.catalog.index().resolve(&make, Scope::All) {
                Some(found) => Some(found.id),
                None => return Ok(answer),
            },
        };
        let page = self.shop.recalls(&RecallQuery {
            source,
            make_id,
            since: Some(since),
            limit: query.limit.unwrap_or(LIST_DEFAULT).clamp(1, MOST_RECALLS),
            cursor,
            ..RecallQuery::default()
        })?;
        let mut vehicles = Vec::with_capacity(page.recalls.len());
        for recall in &page.recalls {
            let listed = match RecallSource::parse(&recall.source) {
                Some(source) => self
                    .shop
                    .recall(source, &recall.campaign)?
                    .map(|detail| detail.vehicles)
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            vehicles.push(listed);
        }
        let mut model_ids: Vec<i64> = vehicles
            .iter()
            .flat_map(|listed| listed.iter().take(MOST_VEHICLES_EACH))
            .filter_map(|vehicle| vehicle.model_id)
            .collect();
        model_ids.sort_unstable();
        model_ids.dedup();
        let slugs = self.model_slugs(&model_ids)?;
        answer.next_cursor = page.next_cursor.map(|cursor| cursor.to_string());
        answer.recalls = page
            .recalls
            .into_iter()
            .zip(vehicles)
            .map(|(recall, vehicles)| RecentRecall {
                recall: self.listed(recall),
                vehicles_listed: u32::try_from(vehicles.len()).unwrap_or(u32::MAX),
                vehicles: vehicles
                    .into_iter()
                    .take(MOST_VEHICLES_EACH)
                    .map(|vehicle| campaign_vehicle(vehicle, &slugs))
                    .collect(),
            })
            .collect();
        Ok(answer)
    }
}

/// The catalog of the core data file, that file itself, and the shop file
/// when there is one: what the overview of a vehicle and the model years
/// of a model are built from. They answer with no shop file at all.
pub struct Overviews<'a, S, C> {
    /// `None` when there is no shop file, or none that can be used.
    pub shop: Option<&'a ShopData<S>>,
    pub catalog: &'a Catalog<C>,
    pub core: &'a C,
    /// The site the addresses in an answer point at: [`SITE`].
    pub base: &'a str,
}

impl<'a, S: Source, C: Source> Overviews<'a, S, C> {
    /// The recall answers over the same files, when the shop file holds
    /// the recalls of either country.
    fn recalls(&self) -> Option<Answers<'a, S, C>> {
        let shop = self
            .shop
            .filter(|shop| shop.require_recalls(None).is_ok())?;
        Some(Answers {
            shop,
            catalog: self.catalog,
            core: self.core,
            base: self.base,
        })
    }

    /// The address of a model year's page on the hosted site.
    fn page(&self, found: &ModelYear) -> String {
        format!(
            "{}/makes/{}/{}/{}",
            self.base, found.make_slug, found.model_slug, found.vehicle.year
        )
    }

    /// The overview of a model year, or of nothing when `found` is `None`.
    ///
    /// This is the one place a block is filled. A plan that brings a
    /// dataset the files hold replaces one `None` below, as the server's
    /// `overview_of` does.
    fn overview_of(
        &self,
        found: Option<&ModelYear>,
        decode: Option<OverviewDecode>,
        warnings: Vec<Json>,
    ) -> Result<Overview, AnswerError> {
        let (recalls, attribution) = match (found, self.recalls()) {
            (Some(found), Some(answers)) => {
                let answer = answers.recalls_of(found, None)?;
                let (count, do_not_drive, park_outside) = tally(&answer.recalls, &answer.as_of);
                let mut newest = answer.recalls;
                newest.truncate(NEWEST);
                (
                    Some(OverviewRecalls {
                        may_apply: true,
                        count,
                        as_of: answer.as_of,
                        do_not_drive,
                        park_outside,
                        newest,
                        lookup: answer.lookup,
                        url: format!("{}/recalls", self.page(found)),
                    }),
                    answer.attribution,
                )
            }
            _ => (None, Vec::new()),
        };
        Ok(Overview {
            vehicle: found.map(|found| found.vehicle.clone()),
            decode,
            specs: None,
            fuel: None,
            fluids: None,
            tires: None,
            recalls,
            safety: None,
            labor: None,
            bulletins: None,
            links: Links {
                page: found.map(|found| self.page(found)),
                maker: None,
            },
            attribution,
            warnings,
        })
    }

    /// `GET /v1/vehicles/{id}/overview`.
    pub fn vehicle(&self, id: &str) -> Result<Overview, AnswerError> {
        let Some(found) = model_year_in(self.catalog, self.core, id)? else {
            return Err(AnswerError::NotFound("No vehicle has that id."));
        };
        self.overview_of(Some(&found), None, Vec::new())
    }

    /// `GET /v1/vin/{vin}/overview`. `decode` is the JSON of the decode, as
    /// `GET /v1/vin/{vin}` gives it, with its `catalog` when it has one.
    pub fn vin(&self, decode: &Json) -> Result<Overview, AnswerError> {
        let text = |key: &str| decode.get(key).and_then(Json::as_str).map(str::to_owned);
        let catalog = decode.get("catalog");
        let found = match catalog
            .and_then(|catalog| catalog.get("vehicle_id"))
            .and_then(Json::as_str)
        {
            Some(id) => model_year_in(self.catalog, self.core, id)?,
            None => None,
        };
        let mut warnings = decode
            .get("warnings")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        if found.is_none() {
            warnings.push(serde_json::json!({
                "code": "no_catalog_vehicle",
                "message": NO_CATALOG_VEHICLE,
            }));
        }
        let summary = OverviewDecode {
            vin: text("vin").unwrap_or_default(),
            valid: decode.get("valid").and_then(Json::as_bool).unwrap_or(false),
            year: decode
                .get("year")
                .and_then(Json::as_u64)
                .and_then(|year| u16::try_from(year).ok()),
            make: text("make"),
            model: text("model"),
            trim: text("trim"),
            engine: decode
                .get("engine")
                .and_then(|engine| engine.get("label"))
                .and_then(Json::as_str)
                .map(str::to_owned),
            engine_settled: catalog
                .and_then(|catalog| catalog.get("engine_id"))
                .is_some_and(|id| !id.is_null()),
            entry_id: catalog
                .and_then(|catalog| catalog.get("entry"))
                .and_then(|entry| entry.get("id"))
                .and_then(Json::as_str)
                .map(str::to_owned),
        };
        self.overview_of(found.as_ref(), Some(summary), warnings)
    }

    /// `GET /v1/models/{make}/{model}`.
    pub fn model_years(&self, make: &str, model: &str) -> Result<ModelYears, AnswerError> {
        let missing = || AnswerError::NotFound("No model has that id.");
        let Some(make) = self.catalog.index().resolve(make, Scope::All) else {
            return Err(missing());
        };
        let rows = self
            .core
            .query(
                MODEL_SQL,
                &[
                    make.id.into(),
                    model.trim().into(),
                    normalize(model).as_str().into(),
                ],
            )
            .map_err(CatalogError::Source)?;
        let Some((model_id, slug, name)) = rows.first().and_then(|row| {
            Some((
                row.first().and_then(Value::integer)?,
                row.get(1).and_then(Value::text)?,
                row.get(2).and_then(Value::text)?,
            ))
        }) else {
            return Err(missing());
        };
        let found: Vec<u16> = self
            .core
            .query(MODEL_YEARS_SQL, &[model_id.into()])
            .map_err(CatalogError::Source)?
            .iter()
            .filter_map(|row| row.first().and_then(Value::integer))
            .filter_map(|year| u16::try_from(year).ok())
            .collect();
        let answers = self.recalls();
        let counts = match &answers {
            Some(answers) => Some(
                answers
                    .shop
                    .recall_counts_by_year(make.id, model_id, None)?,
            ),
            None => None,
        };
        let held = answers.as_ref().map(|answers| answers.as_of());
        let years = found
            .iter()
            .map(|year| {
                // A year no campaign lists is zero for each country held.
                let recalls = counts.as_ref().map(|counted| {
                    counted
                        .iter()
                        .find(|entry| entry.year == *year)
                        .map(|entry| Count {
                            us: entry.us,
                            ca: entry.ca,
                        })
                        .unwrap_or(Count {
                            us: held.as_ref().and_then(|held| held.us.as_ref()).map(|_| 0),
                            ca: held.as_ref().and_then(|held| held.ca.as_ref()).map(|_| 0),
                        })
                });
                ModelYearSummary {
                    id: format!("{year:04}_{}_{slug}", make.slug),
                    year: *year,
                    recalls,
                    bulletins: None,
                    url: format!("{}/makes/{}/{slug}/{year}", self.base, make.slug),
                }
            })
            .collect();
        Ok(ModelYears {
            make: NamedId {
                id: make.slug.clone(),
                name: make.name.clone(),
            },
            model: NamedId {
                id: slug.to_owned(),
                name: name.to_owned(),
            },
            may_apply: true,
            as_of: held.unwrap_or(AsOf { us: None, ca: None }),
            years,
            lookup: lookup(),
            attribution: answers
                .map(|answers| answers.attribution(None))
                .unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_country_is_us_or_ca_in_either_case_or_nothing() {
        assert_eq!(country(None).unwrap(), None);
        assert_eq!(country(Some("  ")).unwrap(), None);
        assert_eq!(country(Some("US")).unwrap(), Some(RecallSource::Nhtsa));
        assert_eq!(country(Some(" ca ")).unwrap(), Some(RecallSource::Tc));
        let error = country(Some("mx")).unwrap_err();
        assert!(matches!(
            error,
            AnswerError::Validation {
                field: "country",
                ..
            }
        ));
        assert_eq!(error.to_string(), "country must be us or ca");
    }

    #[test]
    fn the_dataset_named_as_missing_follows_what_was_asked_for() {
        assert_eq!(dataset_asked(None, None), Dataset::RecallsUs);
        assert_eq!(dataset_asked(Some("us"), None), Dataset::RecallsUs);
        assert_eq!(dataset_asked(Some("CA"), None), Dataset::RecallsCa);
        assert_eq!(dataset_asked(None, Some("tc")), Dataset::RecallsCa);
        assert_eq!(dataset_asked(None, Some("nhtsa")), Dataset::RecallsUs);
        assert_eq!(dataset_asked(Some("mx"), Some("epa")), Dataset::RecallsUs);
    }

    fn counted(country: &str, do_not_drive: Option<bool>, park_outside: Option<bool>) -> Recall {
        Recall {
            source: if country == "us" { "nhtsa" } else { "tc" }.to_owned(),
            country: country.to_owned(),
            campaign: "X".to_owned(),
            date: None,
            manufacturer: None,
            component: None,
            summary: None,
            mfr_campaign: None,
            units: None,
            do_not_drive,
            park_outside,
            notification: None,
            matched: None,
            models: Vec::new(),
            built_from: None,
            built_to: None,
            url: String::new(),
        }
    }

    #[test]
    fn a_country_not_held_is_not_counted_and_one_held_may_be_zero() {
        let day = || Some("2026-10-11".to_owned());
        let list = [
            counted("us", Some(false), Some(true)),
            counted("us", Some(false), Some(false)),
            counted("ca", None, None),
        ];
        let both = AsOf {
            us: day(),
            ca: day(),
        };
        assert_eq!(
            tally(&list, &both),
            (
                Count {
                    us: Some(2),
                    ca: Some(1)
                },
                false,
                true
            )
        );
        assert_eq!(
            tally(&[], &both).0,
            Count {
                us: Some(0),
                ca: Some(0)
            }
        );
        let american = AsOf {
            us: day(),
            ca: None,
        };
        assert_eq!(
            tally(&list[..2], &american).0,
            Count {
                us: Some(2),
                ca: None
            }
        );
        assert!(tally(&[counted("us", Some(true), None)], &american).1);
    }

    #[test]
    fn a_day_is_ten_characters_of_digits_and_dashes() {
        for good in ["2026-09-01", "1968-03-19"] {
            assert!(is_day(good), "{good}");
        }
        for bad in ["", "2026-9-1", "2026/09/01", "20260901", "soon"] {
            assert!(!is_day(bad), "{bad}");
        }
    }

    #[test]
    fn the_lookups_are_the_official_ones_and_name_no_maker() {
        let lookup = lookup();
        assert_eq!(lookup.nhtsa, "https://www.nhtsa.gov/recalls");
        assert!(lookup.tc.starts_with("https://wwwapps.tc.gc.ca/"));
        assert_eq!(lookup.maker, None);
        assert!(NO_CATALOG_VEHICLE.contains("official lookups"));
    }
}
