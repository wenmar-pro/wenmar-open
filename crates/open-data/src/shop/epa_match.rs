//! Matches a fuel-economy row to the catalog: the model, then the engine,
//! then a submodel.
//!
//! EPA writes body and drive into the model name (`F150 Pickup 2WD`), so
//! the name is taken apart first and several forms of it are tried against
//! the shared matcher. NRCan's names are the same kind, so plan H5 uses
//! this module too.

use std::collections::HashMap;

use anyhow::Result;
use rusqlite::Connection;
use wenmar_vehicles::text::normalize;

use crate::shop::matcher::{MatchKind, Matcher, Resolved};

/// Words that say which wheels are driven, and the form they are kept in.
/// Matched as whole words, in any letter case, anywhere but first.
pub const DRIVE_WORDS: [(&str, &str); 7] = [
    ("2wd", "2WD"),
    ("4wd", "4WD"),
    ("awd", "AWD"),
    ("fwd", "FWD"),
    ("rwd", "RWD"),
    ("4x4", "4WD"),
    ("4x2", "2WD"),
];

/// Words that name a body. Taken off the end of a name, one at a time,
/// in any letter case, never the first word.
pub const BODY_WORDS: [&str; 15] = [
    "pickup",
    "wagon",
    "convertible",
    "coupe",
    "sedan",
    "hatchback",
    "cabriolet",
    "roadster",
    "van",
    "spyder",
    "spider",
    "hardtop",
    "limousine",
    "cab",
    "chassis",
];

/// Makes EPA and NRCan name another way than the catalog. Left: their
/// text. Right: the catalog make's name.
pub const MAKE_NAMES: [(&str, &str); 10] = [
    ("McLaren Automotive", "McLaren"),
    ("INEOS Automotive", "INEOS"),
    ("Bugatti Rimac", "Bugatti"),
    ("RUF Automobile", "RUF"),
    ("Ruf Automobile Gmbh", "RUF"),
    ("Saleen Performance", "Saleen"),
    ("American Motors Corporation", "American Motors"),
    ("CODA Automotive", "Coda"),
    ("Panoz Auto-Development", "Panoz"),
    ("Avanti Motor Corporation", "Avanti"),
];

/// The make text to give the matcher.
pub fn catalog_make(make_text: &str) -> &str {
    let make_text = make_text.trim();
    MAKE_NAMES
        .iter()
        .find(|(theirs, _)| theirs.eq_ignore_ascii_case(make_text))
        .map_or(make_text, |(_, ours)| *ours)
}

/// A model name taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpaName {
    /// What is left: `F150`, `Silverado 15 Hybrid`, `328i xDrive`.
    pub name: String,
    /// `2WD`, `4WD`, `AWD`, `FWD` or `RWD`, when the name had a drive word.
    pub drive: Option<&'static str>,
    /// The body words taken off the end, as written: `Pickup`,
    /// `Cab Chassis`, `Sports Wagon` gives `Wagon`.
    pub body: Option<String>,
    /// The text in parentheses, joined by `; `: `cargo`, `20 inch wheels`.
    pub note: Option<String>,
}

/// Takes a model name apart. Nothing is guessed: a part is recorded only
/// when the name had the word.
pub fn parse_name(model: &str) -> EpaName {
    let mut notes: Vec<String> = Vec::new();
    let mut outside = String::with_capacity(model.len());
    let mut inside = String::new();
    let mut depth = 0usize;
    for character in model.chars() {
        match character {
            '(' => {
                depth += 1;
                if depth > 1 {
                    inside.push(character);
                }
            }
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let note = inside.trim().to_owned();
                    if !note.is_empty() {
                        notes.push(note);
                    }
                    inside.clear();
                    outside.push(' ');
                } else {
                    inside.push(character);
                }
            }
            _ if depth > 0 => inside.push(character),
            _ => outside.push(character),
        }
    }
    // An opening parenthesis that never closes: its text is a note too.
    let unclosed = inside.trim();
    if !unclosed.is_empty() {
        notes.push(unclosed.to_owned());
    }

    let mut drive = None;
    let mut words: Vec<&str> = Vec::new();
    for (index, word) in outside.split_whitespace().enumerate() {
        let lower = word.to_ascii_lowercase();
        match DRIVE_WORDS.iter().find(|(theirs, _)| *theirs == lower) {
            Some((_, kept)) if index > 0 => drive = Some(*kept),
            _ => words.push(word),
        }
    }
    let mut body: Vec<&str> = Vec::new();
    while words.len() > 1
        && BODY_WORDS.contains(&words[words.len() - 1].to_ascii_lowercase().as_str())
    {
        body.insert(0, words.pop().expect("more than one word"));
    }
    EpaName {
        name: words.join(" "),
        drive,
        body: (!body.is_empty()).then(|| body.join(" ")),
        note: (!notes.is_empty()).then(|| notes.join("; ")),
    }
}

/// How a row's model was found. `step` is the rule that found it, 1 to 6
/// as numbered on [`match_model`]; 0 when nothing matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelMatch {
    pub resolved: Resolved,
    pub step: u8,
    pub name: EpaName,
}

fn exact(matcher: &Matcher, year: i32, make: &str, text: &str) -> Option<Resolved> {
    let resolved = matcher.resolve(Some(year), make, text);
    (resolved.kind == Some(MatchKind::Exact)).then_some(resolved)
}

/// Matches one row's year, make and model names to catalog models.
///
/// The first rule that finds a model wins:
///
/// 1. The model text as written is a catalog model that year: `exact`.
/// 2. The name left by [`parse_name`] is: `exact`.
/// 3. The base model is, or one of its parts between `/` is, or the make
///    followed by one of those is (`Mazda` + `3` is `Mazda3`): `base`
///    (`exact` when the base model is the whole model text).
/// 4. The name with words taken off its end, longest first, is, alone or
///    after the make: `base`.
/// 5. The shared matcher's base and curated steps on the name.
/// 6. The same on the base model and on each of its parts.
///
/// `base_model` may repeat `model_text` when a source has no base model.
pub fn match_model(
    matcher: &Matcher,
    year: i32,
    make_text: &str,
    model_text: &str,
    base_model: &str,
) -> ModelMatch {
    let make = catalog_make(make_text);
    let name = parse_name(model_text);
    let found = |resolved: Resolved, step: u8, name: EpaName| ModelMatch {
        resolved,
        step,
        name,
    };
    let as_base = |mut resolved: Resolved| {
        resolved.kind = Some(MatchKind::Base);
        resolved
    };

    if let Some(resolved) = exact(matcher, year, make, model_text) {
        return found(resolved, 1, name);
    }
    if let Some(resolved) = exact(matcher, year, make, &name.name) {
        return found(resolved, 2, name);
    }

    let mut bases: Vec<String> = vec![base_model.trim().to_owned()];
    if base_model.contains('/') {
        bases.extend(base_model.split('/').map(|part| part.trim().to_owned()));
    }
    bases.retain(|base| !normalize(base).is_empty());
    for base in &bases {
        let with_make = format!("{make} {base}");
        if let Some(resolved) =
            exact(matcher, year, make, base).or_else(|| exact(matcher, year, make, &with_make))
        {
            let whole = normalize(base) == normalize(model_text);
            return found(if whole { resolved } else { as_base(resolved) }, 3, name);
        }
    }

    let words: Vec<&str> = name.name.split(' ').collect();
    for length in (1..words.len()).rev() {
        let shorter = words[..length].join(" ");
        let with_make = format!("{make} {shorter}");
        if let Some(resolved) =
            exact(matcher, year, make, &shorter).or_else(|| exact(matcher, year, make, &with_make))
        {
            return found(as_base(resolved), 4, name);
        }
    }

    let resolved = matcher.resolve(Some(year), make, &name.name);
    if resolved.kind.is_some() {
        return found(resolved, 5, name);
    }
    for base in &bases {
        let resolved = matcher.resolve(Some(year), make, base);
        if resolved.kind.is_some() {
            return found(resolved, 6, name);
        }
    }
    found(matcher.resolve(Some(year), make, ""), 0, name)
}

/// What a source says of a row's engine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineFacts {
    /// Litres. None for an electric vehicle.
    pub displacement_l: Option<f64>,
    pub cylinders: Option<u8>,
    pub diesel: bool,
    /// None when the source does not say (NRCan).
    pub turbo: Option<bool>,
    /// Battery-electric or fuel cell: no engine displacement.
    pub electric: bool,
    /// An ordinary hybrid. None when the source does not say (NRCan,
    /// unless the model name has the word).
    pub hybrid: Option<bool>,
    pub plug_in: bool,
}

/// A catalog engine label read back into its parts. See
/// `wenmar_vin::engine::label` for how a label is written.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Label {
    /// `3.5` of `3.5L`. None for `Electric` and for a label in cubic
    /// centimetres.
    pub litres: Option<String>,
    pub turbo: bool,
    /// The `6` of `V6`.
    pub v: Option<u8>,
    pub diesel: bool,
    pub hybrid: bool,
    pub plug_in: bool,
    pub electric: bool,
}

/// Reads a catalog engine label: `3.5L Turbo V6`, `2.0L Hybrid`,
/// `3.0L V6 Diesel`, `Electric`.
pub fn parse_label(label: &str) -> Label {
    let mut parsed = Label::default();
    for (index, word) in label.split(' ').enumerate() {
        match word {
            "Electric" if index == 0 => parsed.electric = true,
            "Turbo" => parsed.turbo = true,
            "Diesel" => parsed.diesel = true,
            "Hybrid" => parsed.hybrid = true,
            "PHEV" => parsed.plug_in = true,
            _ => {
                if index == 0 {
                    if let Some(litres) = word.strip_suffix('L')
                        && litres.parse::<f64>().is_ok()
                    {
                        parsed.litres = Some(litres.to_owned());
                    }
                } else if let Some(count) = word.strip_prefix('V') {
                    parsed.v = count.parse().ok();
                }
            }
        }
    }
    parsed
}

/// How far a row was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// The model year only.
    Model,
    /// One engine of that model year.
    Engine,
    /// One engine and one trim or series.
    Submodel,
}

impl Confidence {
    /// The value of the `confidence` column.
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Model => "model",
            Confidence::Engine => "engine",
            Confidence::Submodel => "submodel",
        }
    }
}

/// The catalog's engines and submodels by model year, read once.
pub struct CatalogEngines {
    /// `(year, model id)` to `catalog_vehicle.detail_id`.
    details: HashMap<(i32, i64), i64>,
    /// Detail id to its engines, by id.
    engines: HashMap<i64, Vec<(i64, Label)>>,
    /// Detail id to its listed submodels' ids and matching forms.
    submodels: HashMap<i64, Vec<(i64, String)>>,
    /// Model id to its matching form.
    models: HashMap<i64, String>,
}

impl CatalogEngines {
    pub fn load(catalog: &Connection) -> Result<CatalogEngines> {
        let mut details = HashMap::new();
        let mut statement = catalog.prepare(
            "SELECT year, model_id, detail_id FROM catalog_vehicle WHERE detail_id IS NOT NULL",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            details.insert((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?), row.get(2)?);
        }
        let mut engines: HashMap<i64, Vec<(i64, Label)>> = HashMap::new();
        let mut statement =
            catalog.prepare("SELECT detail_id, id, label FROM catalog_engine ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let label: String = row.get(2)?;
            engines
                .entry(row.get(0)?)
                .or_default()
                .push((row.get(1)?, parse_label(&label)));
        }
        let mut submodels: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
        let mut statement = catalog.prepare(
            "SELECT detail_id, id, norm FROM catalog_submodel WHERE listed = 1 ORDER BY id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            submodels
                .entry(row.get(0)?)
                .or_default()
                .push((row.get(1)?, row.get(2)?));
        }
        let mut models = HashMap::new();
        let mut statement = catalog.prepare("SELECT id, norm FROM catalog_model")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            models.insert(row.get(0)?, row.get(1)?);
        }
        Ok(CatalogEngines {
            details,
            engines,
            submodels,
            models,
        })
    }

    /// The one catalog engine of the model year that the facts describe.
    /// None when none does, or when more than one still could.
    ///
    /// 1. An electric row: the engines labelled `Electric`.
    /// 2. Otherwise the engines with the same litres to one decimal. An
    ///    engine labelled `V6` is dropped when the row has another number
    ///    of cylinders. An engine is dropped when it is a diesel and the
    ///    row is not, or the other way round.
    /// 3. If several are left and some agree with the row on the
    ///    turbocharger, only those are kept.
    /// 4. If several are left: a plug-in row keeps the `PHEV` engines if
    ///    there are any; a hybrid row keeps the `Hybrid` engines if there
    ///    are any, and otherwise drops the `PHEV` ones if that leaves any;
    ///    a row known to be neither keeps the engines that are neither,
    ///    if there are any. A row whose source does not say whether it is
    ///    a hybrid skips this step.
    /// 5. Exactly one left: that engine.
    pub fn engine(&self, year: i32, model_id: i64, facts: &EngineFacts) -> Option<i64> {
        let detail = self.details.get(&(year, model_id))?;
        let engines = self.engines.get(detail)?;
        let mut left: Vec<&(i64, Label)> = if facts.electric {
            engines.iter().filter(|(_, label)| label.electric).collect()
        } else {
            let litres = format!("{:.1}", facts.displacement_l?);
            engines
                .iter()
                .filter(|(_, label)| label.litres.as_deref() == Some(litres.as_str()))
                .filter(|(_, label)| match (label.v, facts.cylinders) {
                    (Some(v), Some(cylinders)) => v == cylinders,
                    _ => true,
                })
                .filter(|(_, label)| label.diesel == facts.diesel)
                .collect()
        };
        let prefer = |left: &mut Vec<&(i64, Label)>, keep: &dyn Fn(&Label) -> bool| {
            if left.len() > 1 && left.iter().any(|(_, label)| keep(label)) {
                left.retain(|(_, label)| keep(label));
            }
        };
        if let Some(turbo) = facts.turbo {
            prefer(&mut left, &|label| label.turbo == turbo);
        }
        if facts.plug_in {
            prefer(&mut left, &|label| label.plug_in);
        } else {
            match facts.hybrid {
                Some(true) if left.iter().any(|(_, label)| label.hybrid) => {
                    prefer(&mut left, &|label| label.hybrid);
                }
                Some(true) => prefer(&mut left, &|label| !label.plug_in),
                Some(false) if !facts.electric => {
                    prefer(&mut left, &|label| !label.hybrid && !label.plug_in);
                }
                _ => {}
            }
        }
        match left.as_slice() {
            [(id, _)] => Some(*id),
            _ => None,
        }
    }

    /// The one listed submodel of the model year that the name spells.
    ///
    /// Every run of whole words of `name` is tried, the longest first and
    /// from the left. A run counts when its matching form is at least two
    /// characters, is not the model's own, and is the form of exactly one
    /// listed submodel.
    pub fn submodel(&self, year: i32, model_id: i64, name: &str) -> Option<i64> {
        let detail = self.details.get(&(year, model_id))?;
        let submodels = self.submodels.get(detail)?;
        let model = self.models.get(&model_id).map(String::as_str).unwrap_or("");
        let words: Vec<&str> = name.split_whitespace().collect();
        for length in (1..=words.len()).rev() {
            for start in 0..=words.len() - length {
                let form = normalize(&words[start..start + length].join(" "));
                if form.len() < 2 || form == model {
                    continue;
                }
                let mut same = submodels.iter().filter(|(_, norm)| *norm == form);
                if let (Some((id, _)), None) = (same.next(), same.next()) {
                    return Some(*id);
                }
            }
        }
        None
    }
}

/// One row matched as far as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleMatch {
    pub model: ModelMatch,
    /// `catalog_engine.id`. Tried only when the row matched one model.
    pub engine_id: Option<i64>,
    /// `catalog_submodel.id`. Tried only when the row matched one model.
    pub submodel_id: Option<i64>,
    /// None exactly when no model matched. `Submodel` needs both an
    /// engine and a submodel; a submodel without an engine is `Model`.
    pub confidence: Option<Confidence>,
}

/// Matches one row to the catalog: model, engine and submodel.
pub fn match_vehicle(
    matcher: &Matcher,
    engines: &CatalogEngines,
    year: i32,
    make_text: &str,
    model_text: &str,
    base_model: &str,
    facts: &EngineFacts,
) -> VehicleMatch {
    let model = match_model(matcher, year, make_text, model_text, base_model);
    let (engine_id, submodel_id) = match model.resolved.model_ids.as_slice() {
        [model_id] => (
            engines.engine(year, *model_id, facts),
            engines.submodel(year, *model_id, &model.name.name),
        ),
        _ => (None, None),
    };
    let confidence = model.resolved.kind.map(|_| match (engine_id, submodel_id) {
        (Some(_), Some(_)) => Confidence::Submodel,
        (Some(_), None) => Confidence::Engine,
        (None, _) => Confidence::Model,
    });
    VehicleMatch {
        model,
        engine_id,
        submodel_id,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(model: &str) -> (String, Option<&'static str>, Option<String>, Option<String>) {
        let name = parse_name(model);
        (name.name, name.drive, name.body, name.note)
    }

    #[test]
    fn drive_body_and_notes_come_off_the_name() {
        for (model, name, drive, body, note) in [
            ("F150 Pickup 2WD", "F150", Some("2WD"), Some("Pickup"), None),
            ("4Runner 4WD", "4Runner", Some("4WD"), None, None),
            ("Civic 4Dr", "Civic 4Dr", None, None, None),
            (
                "328d xDrive Sports Wagon",
                "328d xDrive Sports",
                None,
                Some("Wagon"),
                None,
            ),
            (
                "Astro 2WD (cargo)",
                "Astro",
                Some("2WD"),
                None,
                Some("cargo"),
            ),
            (
                "Sierra C15 Cab Chassis 2WD",
                "Sierra C15",
                Some("2WD"),
                Some("Cab Chassis"),
                None,
            ),
            (
                "Q8 Sportback e-tron quattro (21 inch wheels)",
                "Q8 Sportback e-tron quattro",
                None,
                None,
                Some("21 inch wheels"),
            ),
            ("K1500 Sierra 4X4", "K1500 Sierra", Some("4WD"), None, None),
            (
                "F150 Pickup 2WD FFV",
                "F150 Pickup FFV",
                Some("2WD"),
                None,
                None,
            ),
            ("M3 Convertible", "M3", None, Some("Convertible"), None),
            // The first word is never taken for a drive or a body.
            ("Van", "Van", None, None, None),
            ("4WD", "4WD", None, None, None),
            ("Pickup 2WD", "Pickup", Some("2WD"), None, None),
            ("", "", None, None, None),
            ("Prius (a) (b", "Prius", None, None, Some("a; b")),
        ] {
            assert_eq!(
                parts(model),
                (
                    name.to_owned(),
                    drive,
                    body.map(str::to_owned),
                    note.map(str::to_owned)
                ),
                "{model}"
            );
        }
    }

    #[test]
    fn a_make_named_another_way_is_given_the_catalog_name() {
        assert_eq!(catalog_make("McLaren Automotive"), "McLaren");
        assert_eq!(catalog_make(" mclaren automotive "), "McLaren");
        assert_eq!(catalog_make("Ford"), "Ford");
        assert_eq!(catalog_make("Scion"), "Scion");
    }

    #[test]
    fn labels_are_read_back() {
        let label = parse_label("3.5L Turbo V6");
        assert_eq!(label.litres.as_deref(), Some("3.5"));
        assert!(label.turbo);
        assert_eq!(label.v, Some(6));
        assert!(!label.diesel && !label.hybrid && !label.plug_in && !label.electric);
        let label = parse_label("3.0L V6 Diesel");
        assert!(label.diesel);
        assert_eq!(label.v, Some(6));
        assert!(parse_label("2.0L Hybrid").hybrid);
        assert!(parse_label("2.0L Turbo PHEV").plug_in);
        assert!(parse_label("Electric").electric);
        assert_eq!(parse_label("Electric").litres, None);
        assert_eq!(parse_label("900cc Turbo").litres, None);
        assert_eq!(parse_label("10.8L Diesel").litres.as_deref(), Some("10.8"));
    }

    #[test]
    fn confidence_is_ordered_and_named() {
        assert!(Confidence::Model < Confidence::Engine);
        assert!(Confidence::Engine < Confidence::Submodel);
        assert_eq!(Confidence::Model.as_str(), "model");
        assert_eq!(Confidence::Engine.as_str(), "engine");
        assert_eq!(Confidence::Submodel.as_str(), "submodel");
    }
}
