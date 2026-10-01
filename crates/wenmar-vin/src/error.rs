use serde::Serialize;

/// A character that can never appear in a VIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvalidChar {
    /// 1-based position in the normalized input.
    pub position: usize,
    pub character: char,
}

/// Why a string is not a well-formed VIN.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VinError {
    #[error("a VIN has 17 characters, this has {0}")]
    InvalidLength(usize),
    #[error("a VIN uses only digits and letters other than I, O and Q")]
    InvalidCharacters(Vec<InvalidChar>),
}
