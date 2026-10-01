//! `--jq`: filter the JSON with a jq expression, without needing `jq`.
//!
//! The `jaq` crates run the expression. A result that is text is written
//! as it is, without quotes, so `--jq .make` gives `Hyundai`; anything
//! else is written as one line of JSON.

use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Vars, data, unwrap_valr};
use jaq_json::Val;
use serde_json::Value;

use crate::error::{CliError, JQ_ERROR, USAGE};

/// The longest expression read, in bytes.
const LONGEST: usize = 1_000;
/// The deepest nesting of brackets read. The parser calls itself once per
/// level, so an expression nested thousands deep would overflow the stack.
const DEEPEST: usize = 32;

/// Whether the expression is short and shallow enough to hand to the
/// parser.
fn within_bounds(expression: &str) -> bool {
    let mut depth = 0usize;
    let mut deepest = 0usize;
    for byte in expression.bytes() {
        match byte {
            b'(' | b'[' | b'{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    expression.len() <= LONGEST && deepest <= DEEPEST
}

/// The stack the expression is parsed and run on. The parser and the
/// interpreter both call themselves once per operator, and a chain of a few
/// hundred operators needs more than a thread's usual stack.
const STACK: usize = 64 * 1024 * 1024;

fn refuse() -> CliError {
    CliError::new(USAGE, "the --jq expression could not be read")
        .with_hint("It is a jq expression, such as `.make` or `.[].id`.")
}

/// The lines `expression` writes for `value`, in order.
pub fn filter(expression: &str, value: &Value) -> Result<Vec<String>, CliError> {
    if !within_bounds(expression) {
        return Err(refuse());
    }
    let failed = || CliError::new(JQ_ERROR, "the --jq expression could not be run");
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(scope, || run(expression, value))
            .map_err(|_| failed())?
            .join()
            .map_err(|_| failed())?
    })
}

fn run(expression: &str, value: &Value) -> Result<Vec<String>, CliError> {
    let definitions = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let functions = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let arena = Arena::default();
    let program = File {
        code: expression,
        path: (),
    };
    let modules = Loader::new(definitions)
        .load(&arena, program)
        .map_err(|_| refuse())?;
    let compiled = Compiler::default()
        .with_funs(functions)
        .compile(modules)
        .map_err(|_| refuse())?;

    let input = jaq_json::read::parse_single(value.to_string().as_bytes())
        .map_err(|_| CliError::new(JQ_ERROR, "the answer could not be given to --jq"))?;
    let context = Ctx::<data::JustLut<Val>>::new(&compiled.lut, Vars::new([]));
    let mut lines = Vec::new();
    for result in compiled.id.run((context, input)).map(unwrap_valr) {
        let found = result.map_err(|error| {
            CliError::new(JQ_ERROR, format!("the --jq expression failed: {error}"))
        })?;
        let written = found.to_string();
        lines.push(match serde_json::from_str::<Value>(&written) {
            Ok(Value::String(text)) => text,
            _ => written,
        });
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn text_is_written_without_quotes_and_the_rest_as_json() {
        let value = json!({ "make": "Hyundai", "year": 2023, "engine": { "label": "2.0L" }, "warnings": [] });
        assert_eq!(filter(".make", &value).unwrap(), ["Hyundai"]);
        assert_eq!(filter(".year", &value).unwrap(), ["2023"]);
        assert_eq!(filter(".engine", &value).unwrap(), [r#"{"label":"2.0L"}"#]);
        assert_eq!(filter(".warnings | length", &value).unwrap(), ["0"]);
        assert_eq!(filter(".missing", &value).unwrap(), ["null"]);
        assert_eq!(
            filter(r#""\(.year) \(.make)""#, &value).unwrap(),
            ["2023 Hyundai"]
        );
    }

    #[test]
    fn each_result_is_a_line() {
        let value = json!([{ "id": "ford", "popular": true }, { "id": "kia", "popular": false }]);
        assert_eq!(filter(".[].id", &value).unwrap(), ["ford", "kia"]);
        assert_eq!(
            filter("map(select(.popular)) | .[].id", &value).unwrap(),
            ["ford"]
        );
        assert!(filter("empty", &value).unwrap().is_empty());
    }

    #[test]
    fn an_expression_that_cannot_be_read_is_a_usage_error() {
        for expression in ["", ".[", "nonsense(", "| |"] {
            let error = filter(expression, &json!({})).unwrap_err();
            assert_eq!(error.code, "usage", "{expression:.20}");
        }
    }

    #[test]
    fn an_expression_too_long_or_too_deep_is_refused_before_it_is_parsed() {
        let deep = format!("{}1{}", "(".repeat(33), ")".repeat(33));
        let unclosed = "[".repeat(100_000);
        let long = format!(".a{}", " | .a".repeat(200));
        for expression in [deep.as_str(), unclosed.as_str(), long.as_str()] {
            let error = filter(expression, &json!({})).unwrap_err();
            assert_eq!(error.code, "usage", "{expression:.20}");
        }
        // The longest and deepest that are read do not exhaust the stack.
        let nested = format!("{}1{}", "(".repeat(32), ")".repeat(32));
        assert_eq!(filter(&nested, &json!({})).unwrap(), ["1"]);
        for expression in [
            format!(".{}", " | .".repeat(249)),
            format!("{}1", "-".repeat(999)),
            format!(".a{}", ".a".repeat(499)),
            format!("1{}", "+1".repeat(499)),
        ] {
            assert!(expression.len() <= 1_000);
            let _ = filter(&expression, &json!({ "a": null }));
        }
    }

    #[test]
    fn an_expression_that_fails_as_it_runs_is_a_jq_error() {
        let error = filter(".a.b", &json!({ "a": 1 })).unwrap_err();
        assert_eq!(error.code, "jq_error");
        assert_eq!(
            error.message,
            "the --jq expression failed: cannot index 1 with \"b\""
        );
        let error = filter("error(\"no\")", &json!(1)).unwrap_err();
        assert_eq!(error.code, "jq_error");
    }
}
