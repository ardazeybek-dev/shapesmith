//! JSON Schema (draft 2020-12). Named objects go to `$defs` and are
//! referenced with `$ref`; the root object is written inline.

use serde_json::{Map, Value, json};

use super::{StringKind, format_name, root_is_plain_object, string_kind};
use crate::Options;
use crate::naming::Plan;
use crate::shape::{ObjectShape, Shape};

pub fn render(root: &Shape, root_name: &str, plan: &Plan, opts: &Options) -> String {
    let mut doc = Map::new();
    doc.insert("$schema".into(), json!("https://json-schema.org/draft/2020-12/schema"));
    doc.insert("title".into(), json!(root_name));

    let inline_root = if root_is_plain_object(root) { root.object.as_ref() } else { None };
    let body = match inline_root {
        Some(object) => object_schema(object, plan, opts),
        None => schema(root, plan, opts),
    };
    if let Value::Object(fields) = body {
        doc.extend(fields);
    }

    let defs: Map<String, Value> = plan
        .defs
        .iter()
        .filter(|d| inline_root.is_none_or(|r| !std::ptr::eq(r, d.object)))
        .map(|d| (d.name.clone(), object_schema(d.object, plan, opts)))
        .collect();
    if !defs.is_empty() {
        doc.insert("$defs".into(), Value::Object(defs));
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(doc)).expect("valid JSON");
    text.push('\n');
    text
}

fn object_schema(object: &ObjectShape, plan: &Plan, opts: &Options) -> Value {
    let properties: Map<String, Value> =
        object.fields.iter().map(|(k, f)| (k.clone(), schema(f, plan, opts))).collect();
    let required: Vec<&String> =
        object.fields.iter().filter(|(_, f)| !object.is_optional(f)).map(|(k, _)| k).collect();
    let mut out = json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        out["required"] = json!(required);
    }
    out
}

fn schema(shape: &Shape, plan: &Plan, opts: &Options) -> Value {
    let mut alternatives: Vec<Value> = Vec::new();
    if let Some(object) = &shape.object {
        alternatives.push(json!({ "$ref": format!("#/$defs/{}", plan.name_of(object)) }));
    }
    if let Some(array) = &shape.array {
        alternatives.push(json!({ "type": "array", "items": schema(&array.items, plan, opts) }));
    }
    if let Some(string) = &shape.string {
        alternatives.push(match string_kind(string, opts) {
            StringKind::Plain => json!({ "type": "string" }),
            StringKind::Format(f) => json!({ "type": "string", "format": format_name(f) }),
            StringKind::Enum(values) => json!({ "type": "string", "enum": values }),
        });
    }
    if shape.is_number() {
        alternatives.push(json!({ "type": if shape.float > 0 { "number" } else { "integer" } }));
    }
    if shape.boolean > 0 {
        alternatives.push(json!({ "type": "boolean" }));
    }
    if shape.null > 0 {
        alternatives.push(json!({ "type": "null" }));
    }

    let bare_type = |v: &Value| v.as_object().is_some_and(|o| o.len() == 1 && o.contains_key("type"));
    match alternatives.len() {
        0 => json!({}),
        1 => alternatives.remove(0),
        _ if alternatives.iter().all(bare_type) => {
            json!({ "type": alternatives.iter().map(|a| a["type"].clone()).collect::<Vec<_>>() })
        }
        _ => json!({ "anyOf": alternatives }),
    }
}
