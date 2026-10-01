//! What every request handler shares.

use std::sync::Arc;

use axum::http::HeaderValue;
use tokio::sync::Semaphore;
use wenmar_vehicles::Scope;

use crate::config::Config;
use crate::db::{Db, DbError};
use crate::headers;
use crate::limit::Limiter;
use crate::search_index::{self, SearchIndex};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    db: Db,
    config: Config,
    limiter: Limiter,
    /// One place for every request being answered.
    places: Semaphore,
    etag_text: String,
    etag: HeaderValue,
    data_version: HeaderValue,
    /// Model years, newest first, for light vehicles and for all vehicles.
    years_light: Vec<u16>,
    years_all: Vec<u16>,
    search: Option<SearchIndex>,
}

fn header(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).unwrap_or_else(|_| HeaderValue::from_static("unknown"))
}

/// Builds the search index from the data file. The index is a convenience:
/// if it cannot be built the server still starts, and search uses the
/// catalog's own matching alone.
async fn search_index(db: &Db) -> Option<SearchIndex> {
    let rows = match db.run(|worker| search_index::rows(&worker.source)).await {
        Ok(Ok(rows)) => rows,
        Ok(Err(error)) => {
            tracing::warn!(%error, "search index not built");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "search index not built");
            return None;
        }
    };
    match SearchIndex::build(&rows).await {
        Ok(index) => {
            tracing::info!("search index built over {} models", rows.len());
            Some(index)
        }
        Err(error) => {
            tracing::warn!(%error, "search index not built");
            None
        }
    }
}

impl AppState {
    /// Opens the data file named by `config` and builds the search index.
    pub async fn open(config: Config) -> Result<AppState, DbError> {
        let db = Db::open(&config.data, config.connections).await?;
        // The list of years is asked for by every vehicle form and by the
        // home page, and reading it scans every model year: 80 ms on the
        // real data file. It cannot change while the server runs.
        let (years_light, years_all) = db
            .run(|worker| {
                let newest_first = |scope| {
                    worker.catalog.years(scope, "").map(|mut years| {
                        years.sort_unstable_by(|left: &u16, right: &u16| right.cmp(left));
                        years
                    })
                };
                Ok::<_, wenmar_vehicles::CatalogError>((
                    newest_first(Scope::Light)?,
                    newest_first(Scope::All)?,
                ))
            })
            .await?
            .map_err(|error| DbError::Read(error.into()))?;
        let search = search_index(&db).await;
        let etag_text = headers::etag(db.meta());
        Ok(AppState {
            inner: Arc::new(Inner {
                limiter: Limiter::new(config.requests_per_minute),
                places: Semaphore::new(crate::MOST_IN_FLIGHT),
                etag: header(&etag_text),
                data_version: header(&db.meta().data_version),
                etag_text,
                years_light,
                years_all,
                search,
                db,
                config,
            }),
        })
    }

    /// Every model year in `scope`, newest first. `None` for a single
    /// vehicle type, which is not kept.
    pub fn years(&self, scope: Scope) -> Option<&[u16]> {
        match scope {
            Scope::Light => Some(&self.inner.years_light),
            Scope::All => Some(&self.inner.years_all),
            Scope::Type(_) => None,
        }
    }

    /// The full-text index, when it could be built.
    pub fn search(&self) -> Option<&SearchIndex> {
        self.inner.search.as_ref()
    }

    pub fn db(&self) -> &Db {
        &self.inner.db
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn limiter(&self) -> &Limiter {
        &self.inner.limiter
    }

    /// The places for requests being answered: [`crate::MOST_IN_FLIGHT`].
    pub fn places(&self) -> &Semaphore {
        &self.inner.places
    }

    pub fn etag_text(&self) -> &str {
        &self.inner.etag_text
    }

    pub fn etag(&self) -> &HeaderValue {
        &self.inner.etag
    }

    pub fn data_version(&self) -> &HeaderValue {
        &self.inner.data_version
    }
}
