//! The one error every operation fails with, in the hosted API's shape.

use serde_json::{Value, json};
use wenmar_vehicles::CatalogError;
use wenmar_vin::{DecodeError, VinError};

/// Codes the hosted API also answers with.
pub const INVALID_VIN: &str = "invalid_vin";
pub const VALIDATION_FAILED: &str = "validation_failed";
pub const NOT_FOUND: &str = "not_found";
pub const INTERNAL_ERROR: &str = "internal_error";
/// The database is not a data file this build can read. The command-line
/// tool uses the same code for the same thing.
pub const DATA_INVALID: &str = "data_invalid";

/// Why an operation failed: `{ "code", "message", "details" }`.
#[derive(Debug, Clone, PartialEq)]
pub struct OpError {
    pub code: &'static str,
    pub message: String,
    /// Always an object.
    pub details: Value,
}

impl OpError {
    pub fn new(code: &'static str, message: impl Into<String>) -> OpError {
        OpError {
            code,
            message: message.into(),
            details: json!({}),
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: Value) -> OpError {
        if details.is_object() {
            self.details = details;
        }
        self
    }

    /// A parameter that is missing or wrong, as the API reports it.
    pub fn validation(field: &str, message: impl Into<String>) -> OpError {
        OpError::new(VALIDATION_FAILED, message).with_details(json!({ "field": field }))
    }

    pub fn not_found(message: impl Into<String>) -> OpError {
        OpError::new(NOT_FOUND, message)
    }

    /// The caller of this crate broke the protocol. Never a person's input.
    pub fn internal(message: impl Into<String>) -> OpError {
        OpError::new(INTERNAL_ERROR, message)
    }

    pub fn body(&self) -> Value {
        json!({ "code": self.code, "message": self.message, "details": self.details })
    }
}

impl From<DecodeError> for OpError {
    fn from(error: DecodeError) -> OpError {
        match error {
            DecodeError::InvalidVin { error, suggestions } => {
                let mut details = json!({ "suggestions": suggestions });
                if let VinError::InvalidCharacters(characters) = &error {
                    details["invalid_characters"] = json!(characters);
                }
                OpError::new(INVALID_VIN, error.to_string()).with_details(details)
            }
            DecodeError::UnknownManufacturer { wmi } => {
                OpError::not_found(format!("No manufacturer is registered for {wmi}."))
            }
            other => OpError::new(
                DATA_INVALID,
                format!("The data could not be read: {other}."),
            ),
        }
    }
}

impl From<CatalogError> for OpError {
    fn from(error: CatalogError) -> OpError {
        match error {
            CatalogError::Shape(table) => unexpected_row(table),
            other => OpError::new(
                DATA_INVALID,
                format!("The data could not be read: {other}."),
            ),
        }
    }
}

/// A row that does not have the columns or the kinds of value this build
/// expects: another layout, or a database that returns values oddly.
pub fn unexpected_row(table: &str) -> OpError {
    OpError::new(
        DATA_INVALID,
        format!("The data has an unexpected row in {table}."),
    )
    .with_details(json!({ "table": table }))
}
