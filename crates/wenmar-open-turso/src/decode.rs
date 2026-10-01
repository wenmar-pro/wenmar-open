//! Decoding one VIN against the data file, and reading the catalog.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use wenmar_vehicles::{Catalog, CatalogError, Selection, SourceError};
use wenmar_vin::{DecodeError, DecodeOptions, Decoded, Decoder, Vin};

use crate::db::{Db, DbError, TursoSource, Worker};
use crate::vin_rows::{self, VinRows};

/// Longest text read as a VIN, in bytes. A VIN has 17 characters; spaces and
/// dashes are allowed, so there is some room. Anything longer is refused
/// unread.
pub const LONGEST_INPUT: usize = 64;

/// A decoded VIN with the catalog entry it reaches.
///
/// It serializes as the hosted API's decode does: the decoder's fields, and
/// `catalog` when the catalog can place the vehicle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decode {
    #[serde(flatten)]
    pub decoded: Decoded,
    /// The catalog entry a vehicle form would have reached. `None` when the
    /// decode has no year, make or model, or the catalog has no such model
    /// year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<Selection>,
}

/// Why a VIN could not be decoded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeFailure {
    /// The text is over [`LONGEST_INPUT`] bytes. Nothing was read.
    #[error("a VIN has 17 characters, this is far longer")]
    TooLong,
    /// The text is not a well-formed VIN, or no manufacturer is registered
    /// for it. This is the caller's to report; the data file is fine.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// The rows for the VIN could not be read from the data file.
    #[error("the data file could not be read: {0}")]
    Read(#[source] SourceError),
    /// The catalog could not be read.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// The task that ran the decode stopped before it finished.
    #[error(transparent)]
    Db(#[from] DbError),
}

/// The calendar year, worked out the way `wenmar-vin` does. It is only an
/// upper bound on model years, so a day's error at New Year does not matter.
pub fn current_year() -> u16 {
    const SECONDS_PER_YEAR: u64 = 31_556_952;
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    u16::try_from(1970 + seconds / SECONDS_PER_YEAR).unwrap_or(u16::MAX)
}

impl Worker {
    /// Decodes one VIN: fetches its rows, decodes, then asks the catalog
    /// which entry it is.
    ///
    /// Blocks. Call it inside [`Db::run`], or use [`Db::decode_vin`].
    pub fn decode(&self, input: &str, options: DecodeOptions) -> Result<Decode, DecodeFailure> {
        if input.len() > LONGEST_INPUT {
            return Err(DecodeFailure::TooLong);
        }
        // The rows are fetched for the same years the decoder will try, so
        // both must be given the same current year.
        let current_year = options.current_year.unwrap_or_else(current_year);
        let options = DecodeOptions {
            model_year: options.model_year,
            current_year: Some(current_year),
        };
        let rows = match Vin::parse(input) {
            Ok(vin) => vin_rows::fetch(&self.source, &vin, options.model_year, current_year)
                .map_err(DecodeFailure::Read)?,
            // The decoder says what is wrong with it and suggests corrections.
            Err(_) => VinRows::default(),
        };
        let decoded = Decoder::new(&rows).decode(input, options)?;
        let catalog = self.catalog.selection(&decoded)?;
        Ok(Decode { decoded, catalog })
    }
}

impl Db {
    /// Decodes one VIN on the blocking thread pool.
    ///
    /// Spaces and dashes in the VIN are ignored and letters may be in either
    /// case. A wrong check digit is not a failure: the decode has
    /// `valid: false` and a warning.
    pub async fn decode_vin(
        &self,
        vin: &str,
        options: DecodeOptions,
    ) -> Result<Decode, DecodeFailure> {
        let input = vin.to_owned();
        self.run(move |worker| worker.decode(&input, options))
            .await?
    }

    /// Runs `work` with the catalog on the blocking thread pool.
    ///
    /// Every catalog call blocks, so the catalog is never handed to async
    /// code. Put the calls that belong together in one closure: they use
    /// one connection.
    pub async fn catalog<T, F>(&self, work: F) -> Result<T, DbError>
    where
        F: FnOnce(&Catalog<TursoSource>) -> T + Send + 'static,
        T: Send + 'static,
    {
        self.run(move |worker| work(&worker.catalog)).await
    }
}
