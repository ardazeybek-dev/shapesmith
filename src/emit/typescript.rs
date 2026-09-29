use std::fmt::Write;

use super::{StringKind, format_name, property_key, quote, root_is_plain_object, string_kind};
use crate::Options;
use crate::naming::Plan;
use crate::shape::Shape;

pub fn render(root: &Shape, root_name: &str, plan: &Plan, opts: &Options) -> String {
    let mut out = String::new();
    if !root_is_plain_object(root) {
        writeln!(out, "export type {root_name} = {};\n", expr(root, plan, opts)).unwrap();
    }
    // Root first reads best; defs are stored dependencies-first.
    for def in plan.defs.iter().rev() {
        writeln!(out, "export interface {} {{", def.name).unwrap();
        for (key, field) in &def.object.fields {
            let mut notes = Vec::new();
            if let Some(StringKind::Format(f)) = field.string.as_ref().map(|s| string_kind(s, opts)) {
                notes.push(format!("format: {}", format_name(f)));
            }
            let optional = def.object.is_optional(field);
            if optional {
                notes.push(format!("present in {} of {} samples", field.seen, def.object.count));
            }
            if !notes.is_empty() {
                writeln!(out, "  /** {} */", notes.join("; ")).unwrap();
            }
            let mark = if optional { "?" } else { "" };
            writeln!(out, "  {}{mark}: {};", property_key(key), expr(field, plan, opts)).unwrap();
        }
        out.push_str("}\n\n");
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

fn expr(shape: &Shape, plan: &Plan, opts: &Options) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(object) = &shape.object {
        parts.push(plan.name_of(object).to_owned());
    }
    if let Some(array) = &shape.array {
        let item = expr(&array.items, plan, opts);
        parts.push(if item.contains(" | ") { format!("({item})[]") } else { format!("{item}[]") });
    }
    if let Some(string) = &shape.string {
        match string_kind(string, opts) {
            StringKind::Enum(values) => parts.extend(values.into_iter().map(quote)),
            _ => parts.push("string".into()),
        }
    }
    if shape.is_number() {
        parts.push("number".into());
    }
    if shape.boolean > 0 {
        parts.push("boolean".into());
    }
    if shape.null > 0 {
        parts.push("null".into());
    }
    if parts.is_empty() { "unknown".into() } else { parts.join(" | ") }
}
