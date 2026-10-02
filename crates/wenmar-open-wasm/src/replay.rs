//! A database made of the rows the caller has already read.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wenmar_vehicles::{Source, SourceError, Value};

use crate::cells::to_json;

/// One `SELECT` and what binds to `?1`, `?2` and so on.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}

/// The rows of one statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub statement: Statement,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Default)]
struct Inner {
    answers: HashMap<String, Vec<Vec<Value>>>,
    asked: HashSet<String>,
    missing: Vec<Statement>,
}

/// A [`Source`] that answers from memory.
///
/// A statement it has rows for gets them. Any other is written down, once,
/// and answered with no rows, so the work goes on and asks for everything
/// else it can ask for without that answer. Whatever such a run produced is
/// thrown away by the caller: see [`Replay::finish`].
///
/// Copies share the same rows, so the catalog can keep one.
#[derive(Debug, Clone, Default)]
pub struct Replay {
    inner: Rc<RefCell<Inner>>,
}

/// What a statement is filed under: its text and its parameters.
fn key(sql: &str, params: &[Value]) -> String {
    let params: Vec<serde_json::Value> = params.iter().map(to_json).collect();
    format!("{sql}\u{0}{}", serde_json::Value::Array(params))
}

impl Replay {
    pub fn new() -> Replay {
        Replay::default()
    }

    /// Replaces the rows held with `answers` and forgets what was missing.
    pub fn load(&self, answers: Vec<Answer>) {
        let mut inner = self.inner.borrow_mut();
        inner.answers = answers
            .into_iter()
            .map(|answer| {
                (
                    key(&answer.statement.sql, &answer.statement.params),
                    answer.rows,
                )
            })
            .collect();
        inner.asked.clear();
        inner.missing.clear();
    }

    /// Whether a statement has gone unanswered since the last [`Replay::load`].
    pub fn incomplete(&self) -> bool {
        !self.inner.borrow().missing.is_empty()
    }

    /// Drops the rows held and returns the statements that had none, in the
    /// order they were asked. When the list is not empty, the work that
    /// asked ran on partial rows and its result must not be used.
    pub fn finish(&self) -> Vec<Statement> {
        let mut inner = self.inner.borrow_mut();
        inner.answers.clear();
        inner.asked.clear();
        std::mem::take(&mut inner.missing)
    }
}

impl Source for Replay {
    fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, SourceError> {
        let key = key(sql, params);
        let mut inner = self.inner.borrow_mut();
        if let Some(rows) = inner.answers.get(&key) {
            return Ok(rows.clone());
        }
        if inner.asked.insert(key) {
            inner.missing.push(Statement {
                sql: sql.to_owned(),
                params: params.to_vec(),
            });
        }
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statement(sql: &str, params: Vec<Value>) -> Statement {
        Statement {
            sql: sql.to_owned(),
            params,
        }
    }

    #[test]
    fn an_unanswered_statement_is_written_down_once_and_gives_no_rows() {
        let replay = Replay::new();
        let copy = replay.clone();
        assert_eq!(
            copy.query("SELECT 1", &[]).unwrap(),
            Vec::<Vec<Value>>::new()
        );
        assert_eq!(
            copy.query("SELECT 1", &[]).unwrap(),
            Vec::<Vec<Value>>::new()
        );
        assert_eq!(
            copy.query("SELECT ?1", &[Value::Integer(7)]).unwrap(),
            Vec::<Vec<Value>>::new()
        );
        assert!(replay.incomplete());
        assert_eq!(
            replay.finish(),
            vec![
                statement("SELECT 1", vec![]),
                statement("SELECT ?1", vec![Value::Integer(7)])
            ]
        );
        assert!(!replay.incomplete());
    }

    #[test]
    fn rows_are_found_by_the_statement_and_its_parameters() {
        let replay = Replay::new();
        replay.load(vec![Answer {
            statement: statement("SELECT ?1", vec![Value::Text("a".to_owned())]),
            rows: vec![vec![Value::Integer(1)]],
        }]);
        let text = |text: &str| Value::Text(text.to_owned());
        assert_eq!(
            replay.query("SELECT ?1", &[text("a")]).unwrap(),
            vec![vec![Value::Integer(1)]]
        );
        assert!(!replay.incomplete());
        assert!(replay.query("SELECT ?1", &[text("b")]).unwrap().is_empty());
        // The text `1` and the number 1 are different parameters.
        assert!(replay.query("SELECT ?1", &[text("1")]).unwrap().is_empty());
        assert!(
            replay
                .query("SELECT ?1", &[Value::Integer(1)])
                .unwrap()
                .is_empty()
        );
        assert_eq!(replay.finish().len(), 3);
        // The rows are gone once the call is finished.
        assert!(replay.query("SELECT ?1", &[text("a")]).unwrap().is_empty());
    }
}
