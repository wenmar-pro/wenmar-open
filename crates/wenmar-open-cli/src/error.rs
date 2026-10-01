//! The one error every command can fail with.
//!
//! It has the shape of the hosted API's errors, `{ "error": { "code",
//! "message", "details" } }`, and the API's codes where the API has one, so
//! a caller handles an error the same way whether the answer came from the
//! local data file or from the API.

use std::io;

use serde_json::{Value, json};

/// Codes the hosted API also uses.
pub const INVALID_VIN: &str = "invalid_vin";
pub const VALIDATION_FAILED: &str = "validation_failed";
pub const NOT_FOUND: &str = "not_found";
pub const RATE_LIMITED: &str = "rate_limited";
pub const INTERNAL_ERROR: &str = "internal_error";

/// Codes only this tool has.
///
/// The command line could not be read.
pub const USAGE: &str = "usage";
/// `--offline` was asked for and there is no data file.
pub const NO_DATA: &str = "no_data";
/// There is a data file and this build cannot read it.
pub const DATA_INVALID: &str = "data_invalid";
/// The hosted API, or the release server, could not be reached.
pub const NETWORK: &str = "network";
/// Something answered, and it was not the API.
pub const BAD_RESPONSE: &str = "bad_response";
/// A download did not arrive whole.
pub const DOWNLOAD_FAILED: &str = "download_failed";
/// A file or directory could not be read or written.
pub const IO: &str = "io";
/// A `--jq` expression failed while it ran.
pub const JQ_ERROR: &str = "jq_error";
/// The reader of standard output went away. Not reported: the process ends
/// quietly, as `head` expects of whatever feeds it.
pub const PIPE_CLOSED: &str = "pipe_closed";

/// Why a command failed.
#[derive(Debug, Clone, PartialEq)]
pub struct CliError {
    pub code: String,
    pub message: String,
    /// Always an object.
    pub details: Value,
}

impl CliError {
    pub fn new(code: &str, message: impl Into<String>) -> CliError {
        CliError {
            code: code.to_owned(),
            message: message.into(),
            details: json!({}),
        }
    }

    /// Replaces the details. Anything but an object is ignored.
    #[must_use]
    pub fn with_details(mut self, details: Value) -> CliError {
        if details.is_object() {
            self.details = details;
        }
        self
    }

    /// Adds what to do about it, as `details.hint`.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> CliError {
        if let Some(details) = self.details.as_object_mut() {
            details.insert("hint".to_owned(), Value::String(hint.into()));
        }
        self
    }

    pub fn validation(field: &str, message: impl Into<String>) -> CliError {
        CliError::new(VALIDATION_FAILED, message).with_details(json!({ "field": field }))
    }

    /// A failed write to standard output or standard error.
    pub fn from_write(error: &io::Error) -> CliError {
        if error.kind() == io::ErrorKind::BrokenPipe {
            CliError::new(PIPE_CLOSED, "the reader of the output went away")
        } else {
            CliError::new(IO, format!("the output could not be written: {error}"))
        }
    }

    pub fn hint(&self) -> Option<&str> {
        self.details.get("hint").and_then(Value::as_str)
    }

    /// The process exit code. 0 is success and is never returned here,
    /// except for a closed pipe.
    pub fn exit_code(&self) -> u8 {
        match self.code.as_str() {
            PIPE_CLOSED => 0,
            USAGE => 2,
            NOT_FOUND => 3,
            INVALID_VIN | VALIDATION_FAILED => 4,
            RATE_LIMITED => 5,
            INTERNAL_ERROR | "unavailable" | "method_not_allowed" | "payload_too_large"
            | BAD_RESPONSE => 6,
            NETWORK => 10,
            NO_DATA | DATA_INVALID => 11,
            _ => 1,
        }
    }

    /// The error as JSON, in the API's shape.
    pub fn body(&self) -> Value {
        json!({ "error": { "code": self.code, "message": self.message, "details": self.details } })
    }

    /// Reads an error the API sent. `None` when the JSON is not one.
    pub fn from_body(body: &Value) -> Option<CliError> {
        let error = body.get("error")?;
        let code = error.get("code")?.as_str()?;
        let message = error.get("message")?.as_str()?;
        let details = error.get("details").cloned().unwrap_or_else(|| json!({}));
        Some(CliError::new(code, message).with_details(details))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_has_the_shape_of_an_api_error() {
        let error = CliError::validation("make", "make is required");
        assert_eq!(
            error.body(),
            json!({ "error": { "code": "validation_failed", "message": "make is required", "details": { "field": "make" } } })
        );
        assert_eq!(CliError::from_body(&error.body()), Some(error));
    }

    #[test]
    fn details_are_always_an_object() {
        let error = CliError::new(NOT_FOUND, "No.").with_details(json!("text"));
        assert_eq!(error.details, json!({}));
        let error = error.with_hint("Try again.");
        assert_eq!(error.details, json!({ "hint": "Try again." }));
        assert_eq!(error.hint(), Some("Try again."));
    }

    #[test]
    fn each_code_has_its_exit_code() {
        let cases = [
            (USAGE, 2),
            (NOT_FOUND, 3),
            (INVALID_VIN, 4),
            (VALIDATION_FAILED, 4),
            (RATE_LIMITED, 5),
            (INTERNAL_ERROR, 6),
            ("unavailable", 6),
            (BAD_RESPONSE, 6),
            (NETWORK, 10),
            (NO_DATA, 11),
            (DATA_INVALID, 11),
            (IO, 1),
            (DOWNLOAD_FAILED, 1),
            (JQ_ERROR, 1),
            ("a_code_added_to_the_api_later", 1),
            (PIPE_CLOSED, 0),
        ];
        for (code, exit) in cases {
            assert_eq!(CliError::new(code, "m").exit_code(), exit, "{code}");
        }
    }

    #[test]
    fn something_that_is_not_an_api_error_is_not_read_as_one() {
        for body in [
            json!({}),
            json!([]),
            json!({ "error": "no" }),
            json!({ "error": { "code": 7, "message": "m" } }),
            json!({ "error": { "code": "x" } }),
        ] {
            assert_eq!(CliError::from_body(&body), None, "{body}");
        }
        // Details may be missing.
        let read = CliError::from_body(&json!({ "error": { "code": "x", "message": "m" } }));
        assert_eq!(read, Some(CliError::new("x", "m")));
    }

    #[test]
    fn a_closed_pipe_is_told_apart_from_other_write_failures() {
        let closed = io::Error::from(io::ErrorKind::BrokenPipe);
        assert_eq!(CliError::from_write(&closed).code, PIPE_CLOSED);
        let full = io::Error::other("disk full");
        assert_eq!(CliError::from_write(&full).code, IO);
    }
}
