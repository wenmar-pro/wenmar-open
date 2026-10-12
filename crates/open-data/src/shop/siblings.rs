//! Models that share recall campaigns and bulletins with a model year.
//!
//! Derived from the vehicle rows the sources stored. It says two models
//! have documents in common; it does not say a part fits both.

use anyhow::Result;
use wenmar_shopdata::schema::{self, SIBLING_FEWEST_SHARED, SIBLING_MOST_MODELS, SIBLING_SHARE};

use crate::shop::build::ShapeContext;

fn has_table(context: &ShapeContext<'_>, name: &str) -> Result<bool> {
    Ok(context.transaction.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )?)
}

/// Fills `vehicle_sibling` from whichever of the recall and bulletin
/// tables the file has. Runs after every source's shape step.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    let mut documents: Vec<&str> = Vec::new();
    if has_table(context, "bulletin_vehicle")? && has_table(context, "bulletin_year")? {
        documents.push(
            "SELECT 'b' || v.bulletin_id AS doc, m.model_id AS model_id, y.year AS year
             FROM bulletin_vehicle v
             JOIN bulletin_model m ON m.id = v.bulletin_model_id
             JOIN bulletin_year y ON y.year BETWEEN v.year_from AND v.year_to
             WHERE m.model_id IS NOT NULL",
        );
    }
    if has_table(context, "recall_vehicle")? {
        documents.push(
            "SELECT 'r' || recall_id AS doc, model_id, year FROM recall_vehicle
             WHERE model_id IS NOT NULL AND year IS NOT NULL",
        );
    }
    let transaction = context.transaction;
    transaction.execute_batch(schema::SIBLINGS)?;
    if documents.is_empty() {
        return Ok(());
    }
    transaction.execute_batch(&format!(
        "CREATE TEMP TABLE sibling_doc AS {documents};
         CREATE INDEX temp.sibling_doc_doc ON sibling_doc (doc);
         CREATE TEMP TABLE sibling_width AS
           SELECT doc, COUNT(DISTINCT model_id) AS models FROM sibling_doc GROUP BY doc;
         CREATE TEMP TABLE sibling_narrow AS
           SELECT d.doc, d.model_id, d.year FROM sibling_doc d
           JOIN sibling_width w ON w.doc = d.doc WHERE w.models BETWEEN 2 AND {SIBLING_MOST_MODELS};
         CREATE INDEX temp.sibling_narrow_doc ON sibling_narrow (doc, year);
         CREATE TEMP TABLE sibling_total AS
           SELECT d.model_id, d.year, COUNT(*) AS total FROM sibling_doc d
           JOIN sibling_width w ON w.doc = d.doc WHERE w.models <= {SIBLING_MOST_MODELS}
           GROUP BY d.model_id, d.year;
         INSERT INTO vehicle_sibling (model_id, year, sibling_model_id, shared, total)
           SELECT a.model_id, a.year, b.model_id, COUNT(*), t.total
           FROM sibling_narrow a
           JOIN sibling_narrow b ON b.doc = a.doc AND b.year = a.year AND b.model_id <> a.model_id
           JOIN sibling_total t ON t.model_id = a.model_id AND t.year = a.year
           GROUP BY a.model_id, a.year, b.model_id, t.total
           HAVING COUNT(*) >= {SIBLING_FEWEST_SHARED} AND COUNT(*) * {SIBLING_SHARE} >= t.total;
         DROP TABLE sibling_narrow;
         DROP TABLE sibling_total;
         DROP TABLE sibling_width;
         DROP TABLE sibling_doc;",
        documents = documents.join(" UNION ")
    ))?;
    Ok(())
}
