//! Turning raw text into a list of sample values.

use std::fmt;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    /// 1-based line of the offending input, when known.
    pub line: Option<usize>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses `text` as one JSON document or, failing that, as NDJSON
/// (one JSON value per line). With `split_arrays`, a top-level array is
/// read as a list of samples rather than as a single sample.
pub fn parse_samples(text: &str, split_arrays: bool) -> Result<Vec<Value>, ParseError> {
    if text.trim().is_empty() {
        return Err(ParseError { message: "input is empty".into(), line: None });
    }
    let document_error = match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(items)) if split_arrays => {
            return if items.is_empty() {
                Err(ParseError { message: "top-level array is empty".into(), line: None })
            } else {
                Ok(items)
            };
        }
        Ok(value) => return Ok(vec![value]),
        Err(e) => e,
    };

    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if lines.len() < 2 {
        return Err(ParseError { message: document_error.to_string(), line: None });
    }
    lines
        .into_iter()
        .map(|(line, l)| {
            serde_json::from_str(l).map_err(|e| ParseError { message: format!("not valid JSON ({e})"), line: Some(line) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn single_document() {
        assert_eq!(parse_samples(r#"{"a": 1}"#, true).unwrap(), vec![json!({"a": 1})]);
    }

    #[test]
    fn top_level_array_is_split_only_when_asked() {
        assert_eq!(parse_samples("[1, 2]", true).unwrap(), vec![json!(1), json!(2)]);
        assert_eq!(parse_samples("[1, 2]", false).unwrap(), vec![json!([1, 2])]);
    }

    #[test]
    fn ndjson() {
        let text = "{\"a\": 1}\n\n{\"a\": 2}\n";
        assert_eq!(parse_samples(text, true).unwrap().len(), 2);
    }

    #[test]
    fn reports_the_bad_ndjson_line() {
        let err = parse_samples("{\"a\": 1}\n{oops}\n", true).unwrap_err();
        assert_eq!(err.line, Some(2));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(parse_samples("  \n", true).is_err());
        assert!(parse_samples("[]", true).is_err());
    }
}
