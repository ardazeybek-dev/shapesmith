//! Code generators. Each one walks the same [`Plan`] and renders it in its
//! own syntax; decisions shared by all targets live here.

pub mod json_schema;
pub mod typescript;
pub mod zod;

use crate::Options;
use crate::format;
use crate::shape::{Shape, StringShape};

pub enum StringKind<'a> {
    Plain,
    Format(u8),
    Enum(Vec<&'a str>),
}

/// How a string position is rendered. A detected format wins over an enum.
pub fn string_kind<'a>(string: &'a StringShape, opts: &Options) -> StringKind<'a> {
    if opts.detect_formats {
        if let Some(f) = string.format() {
            return StringKind::Format(f);
        }
    }
    if opts.detect_enums {
        if let Some(values) = string.enum_values(opts.max_enum_values) {
            return StringKind::Enum(values);
        }
    }
    StringKind::Plain
}

pub fn format_name(f: u8) -> &'static str {
    match f {
        format::DATE_TIME => "date-time",
        format::DATE => "date",
        format::EMAIL => "email",
        format::UUID => "uuid",
        format::URL => "uri",
        _ => unreachable!("single format bit"),
    }
}

/// True when the root is always an object, so it becomes the root type itself.
pub fn root_is_plain_object(root: &Shape) -> bool {
    root.object.as_ref().is_some_and(|o| o.count == root.seen)
}

pub fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Object key as it must be written in TypeScript / JavaScript source.
pub fn property_key(key: &str) -> String {
    if is_identifier(key) { key.to_owned() } else { quote(key) }
}

pub fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("strings always serialize")
}
