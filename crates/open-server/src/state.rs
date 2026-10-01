//! What every request handler shares.

use std::sync::Arc;

use axum::http::HeaderValue;
use wenmar_vehicles::Scope;

use crate::config::Config;
use crate::db::{Db, DbError};
use crate::headers;
use crate::limit::Limiter;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    db: Db,
    config: Config,
    limiter: Limiter,
    etag_text: String,
    etag: HeaderValue,
    data_version: HeaderValue,
    /// Model years, newest first, for light vehicles and for all vehicles.
    years_light: Vec<u16>,
    years_all: Vec<u16>,
}

fn header(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).unwrap_or_else(|_| HeaderValue::from_static("unknown"))
}

impl AppState {
    /// Opens the data file named by `config`.
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
        let etag_text = headers::etag(db.meta());
        Ok(AppState {
            inner: Arc::new(Inner {
                limiter: Limiter::new(config.requests_per_minute),
                etag: header(&etag_text),
                data_version: header(&db.meta().data_version),
                etag_text,
                years_light,
                years_all,
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

    pub fn db(&self) -> &Db {
        &self.inner.db
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn limiter(&self) -> &Limiter {
        &self.inner.limiter
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
