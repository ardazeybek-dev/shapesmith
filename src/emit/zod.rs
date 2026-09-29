//! Zod v4 schemas. Definitions are written dependencies-first because a
//! `const` must exist before another schema references it.

/// Names JavaScript objects inherit from `Object.prototype`. Zod reads fields
/// with `input[key]`, so a *missing* optional field with one of these names
/// would be seen as the inherited function and rejected.
const INHERITED: &[&str] = &[
    "__proto__",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
];

/// Copies an object's own keys onto a null-prototype object before Zod sees it.
const OWN_KEYS_ONLY: &str = "(v) => (v !== null && typeof v === \"object\" && !Array.isArray(v) ? Object.assign(Object.create(null), v) : v)";

use std::fmt::Write;

use super::{StringKind, property_key, quote, root_is_plain_object, string_kind};
use crate::Options;
use crate::format;
use crate::naming::Plan;
use crate::shape::Shape;

pub fn render(root: &Shape, root_name: &str, plan: &Plan, opts: &Options) -> String {
    let mut out = String::from("import { z } from \"zod\";\n\n");
    for def in &plan.defs {
        let object = def.object;
        let guard =
            object.fields.iter().any(|(key, f)| object.is_optional(f) && INHERITED.contains(&key.as_str()));
        let indent = if guard { "    " } else { "  " };
        if guard {
            writeln!(
                out,
                "export const {}Schema = z.preprocess(\n  {OWN_KEYS_ONLY},\n  z.object({{",
                def.name
            )
            .unwrap();
        } else {
            writeln!(out, "export const {}Schema = z.object({{", def.name).unwrap();
        }
        for (key, field) in &object.fields {
            let optional = if object.is_optional(field) { ".optional()" } else { "" };
            writeln!(out, "{indent}{}: {}{optional},", zod_key(key), expr(field, plan, opts)).unwrap();
        }
        out.push_str(if guard { "  }),\n);\n" } else { "});\n" });
        writeln!(out, "export type {0} = z.infer<typeof {0}Schema>;\n", def.name).unwrap();
    }
    if !root_is_plain_object(root) {
        writeln!(out, "export const {root_name}Schema = {};", expr(root, plan, opts)).unwrap();
        writeln!(out, "export type {0} = z.infer<typeof {0}Schema>;", root_name).unwrap();
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

/// `__proto__: x` in an object literal sets the prototype instead of adding a
/// key, so that one name is written as a computed key.
fn zod_key(key: &str) -> String {
    if key == "__proto__" { format!("[{}]", quote(key)) } else { property_key(key) }
}

fn expr(shape: &Shape, plan: &Plan, opts: &Options) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(object) = &shape.object {
        parts.push(format!("{}Schema", plan.name_of(object)));
    }
    if let Some(array) = &shape.array {
        parts.push(format!("z.array({})", expr(&array.items, plan, opts)));
    }
    if let Some(string) = &shape.string {
        parts.push(match string_kind(string, opts) {
            StringKind::Plain => "z.string()".into(),
            StringKind::Format(format::DATE_TIME) => "z.iso.datetime({ offset: true })".into(),
            StringKind::Format(format::DATE) => "z.iso.date()".into(),
            StringKind::Format(format::EMAIL) => "z.email()".into(),
            StringKind::Format(format::UUID) => "z.uuid()".into(),
            StringKind::Format(format::URL) => "z.url()".into(),
            StringKind::Format(_) => unreachable!("single format bit"),
            StringKind::Enum(values) => {
                let quoted: Vec<String> = values.into_iter().map(quote).collect();
                format!("z.enum([{}])", quoted.join(", "))
            }
        });
    }
    if shape.is_number() {
        parts.push(if shape.float > 0 { "z.number()" } else { "z.int()" }.into());
    }
    if shape.boolean > 0 {
        parts.push("z.boolean()".into());
    }
    match parts.len() {
        0 if shape.null > 0 => "z.null()".into(),
        0 => "z.unknown()".into(),
        n => {
            let base = if n == 1 { parts.remove(0) } else { format!("z.union([{}])", parts.join(", ")) };
            if shape.null > 0 { format!("{base}.nullable()") } else { base }
        }
    }
}
