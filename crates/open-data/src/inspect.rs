//! Looks things up in a data file's vehicle catalog, so a build can be
//! checked by hand before the real command-line tool exists.

use std::path::Path;

use anyhow::{Result, anyhow};
use clap::Subcommand;
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeOptions, Decoder};

/// One question for the catalog.
#[derive(Debug, Clone, Subcommand)]
pub enum Query {
    /// Model years, newest first.
    Years {
        /// Leading digits of the year.
        #[arg(long, default_value = "")]
        term: String,
    },
    /// Makes, popular ones first.
    Makes {
        #[arg(long)]
        year: Option<u16>,
        /// Start of a make's name or alias.
        #[arg(long, default_value = "")]
        term: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Models of a make.
    Models {
        /// A name, an alias or an id form, such as Chevrolet, chevy or chevrolet.
        #[arg(long)]
        make: String,
        #[arg(long)]
        year: Option<u16>,
        #[arg(long, default_value = "")]
        term: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Submodels offered for a model year.
    Submodels {
        #[arg(long)]
        make: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        year: u16,
        #[arg(long, default_value = "")]
        term: String,
    },
    /// Engines of a model year, narrowed by a submodel if one is given.
    Engines {
        #[arg(long)]
        make: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        year: u16,
        #[arg(long)]
        submodel: Option<String>,
        #[arg(long, default_value = "")]
        term: String,
    },
    /// The entry a vehicle id names, such as 2019_honda_civic_si.
    Entry { id: String },
    /// Entries for free text, such as: 2019 civic si
    Search {
        #[arg(required = true)]
        text: Vec<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Decode a VIN and show the catalog entry it reaches.
    Vin { vin: String },
}

/// Answers one question from a data file, as JSON. An entry or selection
/// that does not exist is `null`.
pub fn run(data: &Path, scope: Scope, query: &Query) -> Result<serde_json::Value> {
    let source = SqliteSource::open(data).map_err(|error| anyhow!("{error}"))?;
    let catalog = Catalog::new(source)?;
    Ok(match query {
        Query::Years { term } => serde_json::to_value(catalog.years(scope, term)?)?,
        Query::Makes { year, term, limit } => {
            serde_json::to_value(catalog.makes(*year, scope, term, *limit)?)?
        }
        Query::Models {
            make,
            year,
            term,
            limit,
        } => serde_json::to_value(catalog.models(make, *year, scope, term, *limit)?)?,
        Query::Submodels {
            make,
            model,
            year,
            term,
        } => serde_json::to_value(catalog.submodels(make, model, *year, term)?)?,
        Query::Engines {
            make,
            model,
            year,
            submodel,
            term,
        } => {
            serde_json::to_value(catalog.engines(make, model, *year, submodel.as_deref(), term)?)?
        }
        Query::Entry { id } => serde_json::to_value(catalog.entry(id)?)?,
        Query::Search { text, limit } => {
            serde_json::to_value(catalog.search(&text.join(" "), scope, *limit)?)?
        }
        Query::Vin { vin } => {
            let decoding = SqliteData::open(data).map_err(|error| anyhow!("{error}"))?;
            let decoded = Decoder::new(decoding)
                .decode(vin, DecodeOptions::default())
                .map_err(|error| anyhow!("{error}"))?;
            serde_json::to_value(catalog.selection(&decoded)?)?
        }
    })
}
