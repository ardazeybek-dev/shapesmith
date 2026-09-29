//! Zod v4 schemas. Definitions are written dependencies-first because a
//! `const` must exist before another schema references it.

use std::fmt::Write;

use super::{StringKind, property_key, quote, root_is_plain_object, string_kind};
use crate::Options;
use crate::format;
use crate::naming::Plan;
use crate::shape::Shape;

pub fn render(root: &Shape, root_name: &str, plan: &Plan, opts: &Options) -> String {
    let mut out = String::from("import { z } from \"zod\";\n\n");
    for def in &plan.defs {
        writeln!(out, "export const {}Schema = z.object({{", def.name).unwrap();
        for (key, field) in &def.object.fields {
            let optional = if def.object.is_optional(field) { ".optional()" } else { "" };
            writeln!(out, "  {}: {}{optional},", property_key(key), expr(field, plan, opts)).unwrap();
        }
        out.push_str("});\n");
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
