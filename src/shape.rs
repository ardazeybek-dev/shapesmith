//! The inferred "shape" of a JSON position.
//!
//! A [`Shape`] is not a single type but a tally of everything observed at one
//! position across all samples: how often it was null, a boolean, a number, a
//! string, an array or an object. Observing another value only adds to the
//! tallies, so samples can be streamed in one at a time and the order they
//! arrive in never changes the result.

use std::collections::BTreeSet;

use indexmap::IndexMap;
use serde_json::Value;

use crate::format;

/// Distinct string values remembered per position before giving up on enums.
pub const MAX_TRACKED_VALUES: usize = 64;

/// Largest integer JavaScript represents exactly (`Number.MAX_SAFE_INTEGER`).
const MAX_SAFE_INTEGER: i128 = (1 << 53) - 1;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    /// Values observed at this position (missing keys are not counted).
    pub seen: usize,
    pub null: usize,
    pub boolean: usize,
    /// Integers inside JavaScript's safe range.
    pub integer: usize,
    /// Any other number: fractions, and integers too large for a JS number.
    pub float: usize,
    pub string: Option<StringShape>,
    pub array: Option<Box<ArrayShape>>,
    pub object: Option<ObjectShape>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StringShape {
    pub count: usize,
    /// Formats that every observed string satisfied.
    pub formats: u8,
    /// Distinct values, or `None` once more than [`MAX_TRACKED_VALUES`] were seen.
    pub values: Option<BTreeSet<String>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArrayShape {
    pub count: usize,
    /// Every element of every array observed here, folded together.
    pub items: Shape,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectShape {
    pub count: usize,
    /// Fields in first-seen order. A field is optional when `field.seen < count`.
    pub fields: IndexMap<String, Shape>,
}

impl Shape {
    /// Folds a sequence of sample values into one shape.
    pub fn infer<'a>(samples: impl IntoIterator<Item = &'a Value>) -> Self {
        let mut shape = Shape::default();
        for sample in samples {
            shape.observe(sample);
        }
        shape
    }

    pub fn observe(&mut self, value: &Value) {
        self.seen += 1;
        match value {
            Value::Null => self.null += 1,
            Value::Bool(_) => self.boolean += 1,
            Value::Number(n) => {
                let safe_int = n
                    .as_i64()
                    .map(i128::from)
                    .or_else(|| n.as_u64().map(i128::from))
                    .is_some_and(|i| i.abs() <= MAX_SAFE_INTEGER);
                if safe_int {
                    self.integer += 1;
                } else {
                    self.float += 1;
                }
            }
            Value::String(s) => self.string.get_or_insert_with(StringShape::new).observe(s),
            Value::Array(items) => {
                let array = self.array.get_or_insert_with(Default::default);
                array.count += 1;
                for item in items {
                    array.items.observe(item);
                }
            }
            Value::Object(map) => {
                let object = self.object.get_or_insert_with(Default::default);
                object.count += 1;
                for (key, v) in map {
                    object.fields.entry(key.clone()).or_default().observe(v);
                }
            }
        }
    }

    pub fn is_number(&self) -> bool {
        self.integer + self.float > 0
    }
}

impl StringShape {
    fn new() -> Self {
        StringShape { count: 0, formats: format::ALL, values: Some(BTreeSet::new()) }
    }

    fn observe(&mut self, s: &str) {
        self.count += 1;
        self.formats &= format::detect(s);
        if let Some(values) = &mut self.values {
            values.insert(s.to_owned());
            if values.len() > MAX_TRACKED_VALUES {
                self.values = None;
            }
        }
    }

    /// The single format shared by every value, if any.
    pub fn format(&self) -> Option<u8> {
        format::single(self.formats)
    }

    /// Values to emit as a literal union, when the field looks like an enum:
    /// few distinct short values, and at least one of them repeated.
    pub fn enum_values(&self, max: usize) -> Option<Vec<&str>> {
        let values = self.values.as_ref()?;
        let looks_like_enum = !values.is_empty()
            && values.len() <= max
            && self.count > values.len()
            && values.iter().all(|v| !v.is_empty() && v.chars().count() <= 40);
        looks_like_enum.then(|| values.iter().map(String::as_str).collect())
    }
}

impl ObjectShape {
    pub fn is_optional(&self, field: &Shape) -> bool {
        field.seen < self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_fields_become_optional() {
        let samples = [json!({"id": 1, "note": "a"}), json!({"id": 2})];
        let shape = Shape::infer(&samples);
        let object = shape.object.as_ref().unwrap();
        assert!(!object.is_optional(&object.fields["id"]));
        assert!(object.is_optional(&object.fields["note"]));
    }

    #[test]
    fn tallies_mixed_kinds() {
        let samples = [json!(1), json!(1.5), json!(null), json!("x"), json!(9_007_199_254_740_993_u64)];
        let shape = Shape::infer(&samples);
        assert_eq!((shape.integer, shape.float, shape.null), (1, 2, 1), "unsafe integers count as float");
        assert_eq!(shape.string.unwrap().count, 1);
    }

    #[test]
    fn array_items_are_folded_together() {
        let samples = [json!({"tags": ["a", "b"]}), json!({"tags": []}), json!({"tags": [1]})];
        let shape = Shape::infer(&samples);
        let tags = &shape.object.unwrap().fields["tags"];
        let array = tags.array.as_ref().unwrap();
        assert_eq!(array.count, 3);
        assert_eq!(array.items.string.as_ref().unwrap().count, 2);
        assert_eq!(array.items.integer, 1);
    }

    #[test]
    fn enums_need_repetition() {
        let one = Shape::infer(&[json!("paid"), json!("pending")]);
        assert_eq!(one.string.unwrap().enum_values(8), None, "every value unique: no evidence of an enum");
        let many = Shape::infer(&[json!("paid"), json!("pending"), json!("paid")]);
        assert_eq!(many.string.unwrap().enum_values(8), Some(vec!["paid", "pending"]));
    }

    #[test]
    fn formats_must_hold_for_every_value() {
        let shape = Shape::infer(&[json!("2024-01-01"), json!("2024-02-02")]);
        assert_eq!(shape.string.unwrap().format(), Some(format::DATE));
        let mixed = Shape::infer(&[json!("2024-01-01"), json!("soon")]);
        assert_eq!(mixed.string.unwrap().format(), None);
    }

    #[test]
    fn stops_tracking_values_after_the_cap() {
        let samples: Vec<Value> = (0..=MAX_TRACKED_VALUES).map(|i| json!(format!("v{i}"))).collect();
        assert!(Shape::infer(&samples).string.unwrap().values.is_none());
    }
}
