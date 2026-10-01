//! Every statement the catalog runs. Plain SQLite only: no extensions, no
//! virtual tables, nothing engine-specific.
//!
//! Scope parameters come in pairs: a flag that is 1 when only cars, MPVs
//! and trucks count, and a vehicle-type bit that must be set (0 for none).

/// The first and last model year.
pub const YEAR_RANGE: &str = "SELECT MIN(year), MAX(year) FROM catalog_vehicle";

pub const MAKES: &str = "SELECT id, slug, name, norm, rank, types, light FROM catalog_make";

pub const ALIASES: &str = "SELECT norm, make_id FROM catalog_alias";

pub const TYPES: &str = "SELECT id, name FROM catalog_type ORDER BY id";

/// `?1` light flag, `?2` type bit, `?3` leading digits of the year.
pub const YEARS: &str = "
SELECT DISTINCT year FROM catalog_vehicle
WHERE (?1 = 0 OR light = 1) AND (?2 = 0 OR (types & ?2) <> 0)
  AND CAST(year AS TEXT) LIKE ?3 || '%'
ORDER BY year DESC";

/// `?1` year, `?2` light flag, `?3` type bit.
pub const MAKES_FOR_YEAR: &str = "
SELECT DISTINCT make_id FROM catalog_vehicle
WHERE year = ?1 AND (?2 = 0 OR light = 1) AND (?3 = 0 OR (types & ?3) <> 0)";

/// `?1` make id, `?2` year, `?3` light flag, `?4` type bit, `?5` prefix of
/// the matching form, `?6` limit.
pub const MODELS_FOR_YEAR: &str = "
SELECT d.id, d.slug, d.name, d.year_from, d.year_to
FROM catalog_vehicle v JOIN catalog_model d ON d.id = v.model_id
WHERE v.year = ?2 AND v.make_id = ?1
  AND (?3 = 0 OR v.light = 1) AND (?4 = 0 OR (v.types & ?4) <> 0)
  AND d.norm LIKE ?5 || '%'
ORDER BY d.name COLLATE NOCASE, d.id
LIMIT ?6";

/// `?1` make id, `?2` light flag, `?3` type bit, `?4` prefix, `?5` limit.
pub const MODELS_FOR_MAKE: &str = "
SELECT d.id, d.slug, d.name, d.year_from, d.year_to
FROM catalog_model d
WHERE d.make_id = ?1
  AND (?2 = 0 OR d.light = 1) AND (?3 = 0 OR (d.types & ?3) <> 0)
  AND d.norm LIKE ?4 || '%'
ORDER BY d.name COLLATE NOCASE, d.id
LIMIT ?5";

/// The model a person means. `?1` make id, `?2` name as given, `?3` its id
/// form, `?4` its matching form. An exact name wins, then an exact id form.
pub const MODEL: &str = "
SELECT id, slug, name, year_from, year_to FROM catalog_model
WHERE make_id = ?1 AND (name = ?2 COLLATE NOCASE OR slug = ?3 OR norm = ?4)
ORDER BY (name = ?2 COLLATE NOCASE) DESC, (slug = ?3) DESC, light DESC, year_to DESC, id
LIMIT 1";

/// `?1` make id, `?2` id form.
pub const MODEL_BY_SLUG: &str = "
SELECT id, slug, name, year_from, year_to FROM catalog_model
WHERE make_id = ?1 AND slug = ?2";

/// `?1` year, `?2` make id, `?3` model id.
pub const VEHICLE: &str = "
SELECT v.types, v.detail_id, t.body, t.drive, t.transmission
FROM catalog_vehicle v LEFT JOIN catalog_detail t ON t.id = v.detail_id
WHERE v.year = ?1 AND v.make_id = ?2 AND v.model_id = ?3";

/// `?1` detail id.
pub const SUBMODELS: &str = "
SELECT id, name, kind, listed, body, drive, transmission
FROM catalog_submodel WHERE detail_id = ?1 ORDER BY id";

/// `?1` detail id, `?2` submodel id or NULL. With a submodel that narrows
/// the engines, only its engines.
pub const ENGINES: &str = "
SELECT e.id, e.label, e.vin8, e.source FROM catalog_engine e
WHERE e.detail_id = ?1
  AND (?2 IS NULL
       OR NOT EXISTS (SELECT 1 FROM catalog_submodel_engine x WHERE x.submodel_id = ?2)
       OR e.id IN (SELECT engine_id FROM catalog_submodel_engine WHERE submodel_id = ?2))
ORDER BY e.label, e.id";

/// Models by matching form, with the newest year each exists in. `?1` form,
/// `?2` 1 to also match it as a prefix, `?3` make id or NULL, `?4` year or
/// NULL, `?5` light flag, `?6` type bit, `?7` limit. Columns: make id, model
/// id form, newest year, whether the form matched exactly, model id.
pub const SEARCH_MODELS: &str = "
SELECT d.make_id, d.slug, MAX(v.year), d.norm = ?1, d.id
FROM catalog_model d
JOIN catalog_vehicle v ON v.model_id = d.id
JOIN catalog_make m ON m.id = d.make_id
WHERE (d.norm = ?1 OR (?2 = 1 AND d.norm LIKE ?1 || '%'))
  AND (?3 IS NULL OR d.make_id = ?3)
  AND (?4 IS NULL OR v.year = ?4)
  AND (?5 = 0 OR v.light = 1) AND (?6 = 0 OR (v.types & ?6) <> 0)
GROUP BY d.id
ORDER BY (d.norm = ?1) DESC, m.rank IS NULL, m.rank, d.name COLLATE NOCASE, d.id
LIMIT ?7";

/// The models of a make, with the newest year each exists in. `?1` make id,
/// `?2` year or NULL, `?3` light flag, `?4` type bit, `?5` limit.
pub const SEARCH_MAKE_MODELS: &str = "
SELECT d.make_id, d.slug, MAX(v.year), 0, d.id
FROM catalog_model d JOIN catalog_vehicle v ON v.model_id = d.id
WHERE d.make_id = ?1
  AND (?2 IS NULL OR v.year = ?2)
  AND (?3 = 0 OR v.light = 1) AND (?4 = 0 OR (v.types & ?4) <> 0)
GROUP BY d.id
ORDER BY d.name COLLATE NOCASE, d.id
LIMIT ?5";

/// Submodels of one model by matching form, with the newest year each
/// exists in. `?1` model id, `?2` form, `?3` year or NULL, `?4` light flag,
/// `?5` type bit, `?6` limit. Columns: name, newest year, exact match.
pub const SEARCH_SUBMODELS: &str = "
SELECT s.name, MAX(v.year), s.norm = ?2
FROM catalog_vehicle v JOIN catalog_submodel s ON s.detail_id = v.detail_id
WHERE v.model_id = ?1
  AND (s.norm = ?2 OR s.norm LIKE ?2 || '%')
  AND (?3 IS NULL OR v.year = ?3)
  AND (?4 = 0 OR v.light = 1) AND (?5 = 0 OR (v.types & ?5) <> 0)
GROUP BY s.name
ORDER BY (s.norm = ?2) DESC, s.name COLLATE NOCASE
LIMIT ?6";

/// Submodels across a make's models, for text such as `chevy 1500` where
/// the words name no model. `?1` make id, `?2` form, `?3` year or NULL,
/// `?4` light flag, `?5` type bit, `?6` limit. Columns: model id form,
/// submodel name, newest year, exact match.
pub const SEARCH_MAKE_SUBMODELS: &str = "
SELECT d.slug, s.name, MAX(v.year), s.norm = ?2
FROM catalog_model d
JOIN catalog_vehicle v ON v.model_id = d.id
JOIN catalog_submodel s ON s.detail_id = v.detail_id
WHERE d.make_id = ?1
  AND (s.norm = ?2 OR s.norm LIKE ?2 || '%')
  AND (?3 IS NULL OR v.year = ?3)
  AND (?4 = 0 OR v.light = 1) AND (?5 = 0 OR (v.types & ?5) <> 0)
GROUP BY d.id, s.name
ORDER BY (s.norm = ?2) DESC, MAX(v.year) DESC, d.name COLLATE NOCASE, s.name COLLATE NOCASE
LIMIT ?6";
