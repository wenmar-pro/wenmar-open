//! Where the catalog's rows come from.
//!
//! The catalog needs one thing from a database: run a `SELECT` with some
//! parameters and hand back the rows. Everything else, including every line
//! of SQL, is in this crate and uses only plain SQLite. An adapter for
//! another SQLite engine is one small `impl`.

/// A value bound to a statement or read from a row.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

impl Value {
    pub fn integer(&self) -> Option<i64> {
        match self {
            Value::Integer(number) => Some(*number),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }
}

impl From<i64> for Value {
    fn from(number: i64) -> Self {
        Value::Integer(number)
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Self {
        Value::Text(text.to_owned())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Value::Null, Into::into)
    }
}

/// An error from the underlying database.
pub type SourceError = Box<dyn std::error::Error + Send + Sync>;

/// A read-only database holding the catalog tables.
pub trait Source {
    /// Runs one `SELECT`. `params` bind to `?1`, `?2` and so on, in order.
    /// Returns every row, each with one value per selected column.
    fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, SourceError>;
}

impl<T: Source + ?Sized> Source for &T {
    fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, SourceError> {
        (**self).query(sql, params)
    }
}
