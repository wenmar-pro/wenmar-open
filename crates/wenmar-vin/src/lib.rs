//! VIN decoding for auto repair shops.
//!
//! `wenmar-vin` validates a VIN, checks its check digit, works out the model
//! year, and matches it against NHTSA vPIC pattern data to describe the
//! vehicle. It is the decoder behind [Wenmar Open](https://open.wenmarpro.com).
//!
//! The crate holds no data of its own. It reads from anything that implements
//! [`VinData`]. [`MemoryData`] is a small in-memory implementation for tests
//! and examples.
//!
//! ```
//! use wenmar_vin::{DecodeOptions, Decoder, Element, Manufacturer, MemoryData};
//!
//! let data = MemoryData::new()
//!     .with_manufacturer(Manufacturer {
//!         wmi: "KM8".to_owned(),
//!         name: "Hyundai Motor Co".to_owned(),
//!         make: Some("Hyundai".to_owned()),
//!         country: None,
//!         vehicle_type: None,
//!     })
//!     .with_schema("KM8", 1, 2022, None)
//!     .with_pattern(1, "K2***", Element::Model, "Kona");
//!
//! let decoded = Decoder::new(data).decode("KM8K2CAB4PU001140", DecodeOptions::default())?;
//! assert_eq!(decoded.year, Some(2023));
//! assert_eq!(decoded.make.as_deref(), Some("Hyundai"));
//! assert_eq!(decoded.model.as_deref(), Some("Kona"));
//! assert!(decoded.valid);
//! # Ok::<(), wenmar_vin::DecodeError>(())
//! ```
//!
//! The pure parts are usable on their own: [`Vin::parse`],
//! [`check_digit::check`], [`model_year::candidates`], and the [`suggest`]
//! functions need no data.

pub mod check_digit;
pub mod data;
pub mod decode;
pub mod engine;
pub mod error;
pub mod memory;
pub mod model_year;
pub mod pattern;
pub mod result;
pub mod suggest;
pub mod vin;

pub use check_digit::CheckDigit;
pub use data::{DataError, Element, Manufacturer, Pattern, SchemaRef, VinData};
pub use decode::{DecodeError, DecodeOptions, Decoder};
pub use engine::Engine;
pub use error::{InvalidChar, VinError};
pub use memory::MemoryData;
pub use result::{Decoded, ManufacturerInfo, Plant, Safety, Warning, WarningCode};
pub use vin::Vin;
