//! The one error shape every JSON route answers with.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;
use wenmar_vehicles::CatalogError;
use wenmar_vin::{DecodeError, VinError};

use crate::db::DbError;

/// The body of every error: `{ "error": { "code", "message", "details" } }`.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ErrorDetail {
    /// A stable code to branch on. One of `invalid_vin`, `validation_failed`,
    /// `not_found`, `method_not_allowed`, `payload_too_large`,
    /// `rate_limited`, `unavailable`, `internal_error`.
    #[schema(example = "invalid_vin")]
    pub code: String,
    /// A sentence for a person to read.
    #[schema(example = "a VIN has 17 characters, this has 16")]
    pub message: String,
    /// More about the error. Always an object; empty when there is nothing
    /// to add.
    #[schema(value_type = Object)]
    pub details: Value,
}

/// Why a request could not be answered.
#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    /// 400. Not a well-formed VIN.
    InvalidVin { message: String, details: Value },
    /// 400. A parameter or body is missing or wrong.
    Validation { message: String, details: Value },
    /// 404.
    NotFound(String),
    /// 405.
    MethodNotAllowed,
    /// 413.
    PayloadTooLarge,
    /// 429. `retry_after` is in seconds.
    RateLimited { retry_after: u64 },
    /// 503. The server is too busy to answer in time.
    Unavailable,
    /// 500. The cause is logged, never sent.
    Internal,
}

impl ApiError {
    pub fn validation(message: impl Into<String>) -> ApiError {
        ApiError::Validation {
            message: message.into(),
            details: json!({}),
        }
    }

    /// Logs the cause and hides it from the caller.
    pub fn internal(cause: impl std::fmt::Display) -> ApiError {
        tracing::error!(%cause, "request failed");
        ApiError::Internal
    }

    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::InvalidVin { .. } | ApiError::Validation { .. } => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            ApiError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The body, as it is sent and as a batch or a tool result embeds it.
    pub fn body(&self) -> ErrorBody {
        let (code, message, details) = match self {
            ApiError::InvalidVin { message, details } => {
                ("invalid_vin", message.clone(), details.clone())
            }
            ApiError::Validation { message, details } => {
                ("validation_failed", message.clone(), details.clone())
            }
            ApiError::NotFound(message) => ("not_found", message.clone(), json!({})),
            ApiError::MethodNotAllowed => (
                "method_not_allowed",
                "This address does not accept that method.".to_owned(),
                json!({}),
            ),
            ApiError::PayloadTooLarge => (
                "payload_too_large",
                "The request body is too large.".to_owned(),
                json!({}),
            ),
            ApiError::RateLimited { retry_after } => (
                "rate_limited",
                "Too many requests from this address. The limit is per minute.".to_owned(),
                json!({ "retry_after": retry_after }),
            ),
            ApiError::Unavailable => (
                "unavailable",
                "The server is busy. Try again shortly.".to_owned(),
                json!({}),
            ),
            ApiError::Internal => (
                "internal_error",
                "Something went wrong on our side.".to_owned(),
                json!({}),
            ),
        };
        ErrorBody {
            error: ErrorDetail {
                code: code.to_owned(),
                message,
                details,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status(), Json(self.body())).into_response();
        let headers = response.headers_mut();
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        match self {
            ApiError::RateLimited { retry_after } => {
                headers.insert(header::RETRY_AFTER, HeaderValue::from(retry_after));
            }
            ApiError::Unavailable => {
                headers.insert(header::RETRY_AFTER, HeaderValue::from(1u64));
            }
            _ => {}
        }
        response
    }
}

impl From<DecodeError> for ApiError {
    fn from(error: DecodeError) -> ApiError {
        match error {
            DecodeError::InvalidVin { error, suggestions } => {
                let mut details = json!({ "suggestions": suggestions });
                if let VinError::InvalidCharacters(characters) = &error {
                    details["invalid_characters"] = json!(characters);
                }
                ApiError::InvalidVin {
                    message: error.to_string(),
                    details,
                }
            }
            DecodeError::UnknownManufacturer { wmi } => {
                ApiError::NotFound(format!("No manufacturer is registered for {wmi}."))
            }
            other => ApiError::internal(describe(&other)),
        }
    }
}

/// An error with the errors that caused it, for the log.
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> ApiError {
        ApiError::internal(describe(&error))
    }
}

impl From<DbError> for ApiError {
    fn from(error: DbError) -> ApiError {
        ApiError::internal(describe(&error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_has_a_code_a_message_and_details() {
        let errors = [
            ApiError::InvalidVin {
                message: "m".to_owned(),
                details: json!({ "suggestions": [] }),
            },
            ApiError::validation("m"),
            ApiError::NotFound("m".to_owned()),
            ApiError::MethodNotAllowed,
            ApiError::PayloadTooLarge,
            ApiError::RateLimited { retry_after: 7 },
            ApiError::Unavailable,
            ApiError::Internal,
        ];
        let codes: Vec<String> = errors
            .iter()
            .map(|error| {
                let body = serde_json::to_value(error.body()).unwrap();
                assert!(body["error"]["message"].is_string());
                assert!(body["error"]["details"].is_object());
                body["error"]["code"].as_str().unwrap().to_owned()
            })
            .collect();
        assert_eq!(
            codes,
            [
                "invalid_vin",
                "validation_failed",
                "not_found",
                "method_not_allowed",
                "payload_too_large",
                "rate_limited",
                "unavailable",
                "internal_error"
            ]
        );
    }

    #[test]
    fn an_internal_error_never_says_why() {
        let error = ApiError::internal("no such table: wmi at /app/data/file.sqlite3");
        let body = serde_json::to_string(&error.body()).unwrap();
        assert!(!body.contains("wmi"), "{body}");
        assert!(!body.contains("/app"), "{body}");
        assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn a_malformed_vin_carries_suggestions_and_the_bad_characters() {
        let error = wenmar_vin::Decoder::new(wenmar_vin::MemoryData::new())
            .decode("KM8K2CAB4PUO01140", wenmar_vin::DecodeOptions::default())
            .unwrap_err();
        let body = serde_json::to_value(ApiError::from(error).body()).unwrap();
        assert_eq!(body["error"]["code"], "invalid_vin");
        assert_eq!(
            body["error"]["details"]["suggestions"],
            json!(["KM8K2CAB4PU001140"])
        );
        assert_eq!(
            body["error"]["details"]["invalid_characters"],
            json!([{ "position": 12, "character": "O" }])
        );
    }
}
