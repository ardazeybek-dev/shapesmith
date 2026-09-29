//! Assigning names to object shapes.
//!
//! Objects are visited children-first, so by the time an object is named all
//! of its nested objects already have names. Each object gets a structural
//! signature built from its fields and the names of its children; two objects
//! with the same signature share one named type.

use std::collections::HashMap;

use crate::Options;
use crate::emit::{StringKind, string_kind};
use crate::shape::{ObjectShape, Shape};

pub struct Def<'a> {
    pub name: String,
    pub object: &'a ObjectShape,
}

pub struct Plan<'a> {
    /// Named objects, dependencies before the objects that use them.
    pub defs: Vec<Def<'a>>,
    by_address: HashMap<*const ObjectShape, usize>,
    by_signature: HashMap<String, usize>,
    opts: &'a Options,
}

impl<'a> Plan<'a> {
    pub fn build(root: &'a Shape, root_name: &str, opts: &'a Options) -> Self {
        let mut plan =
            Plan { defs: Vec::new(), by_address: HashMap::new(), by_signature: HashMap::new(), opts };
        plan.visit(root, root_name, "");
        plan
    }

    pub fn name_of(&self, object: &ObjectShape) -> &str {
        &self.defs[self.by_address[&(object as *const ObjectShape)]].name
    }

    fn visit(&mut self, shape: &'a Shape, hint: &str, parent: &str) {
        if let Some(array) = &shape.array {
            self.visit(&array.items, &singular(hint), parent);
        }
        let Some(object) = &shape.object else { return };
        for (key, field) in &object.fields {
            self.visit(field, &pascal(key), hint);
        }
        let signature = self.object_signature(object);
        let index = match self.by_signature.get(&signature) {
            Some(&index) => index,
            None => {
                let name = self.unique_name(hint, parent);
                self.defs.push(Def { name, object });
                self.by_signature.insert(signature, self.defs.len() - 1);
                self.defs.len() - 1
            }
        };
        self.by_address.insert(object, index);
    }

    fn unique_name(&self, hint: &str, parent: &str) -> String {
        let taken = |name: &str| self.defs.iter().any(|d| d.name == name);
        if !taken(hint) {
            return hint.to_owned();
        }
        let qualified = format!("{parent}{hint}");
        if !taken(&qualified) {
            return qualified;
        }
        (2..).map(|n| format!("{qualified}{n}")).find(|name| !taken(name)).expect("unbounded range")
    }

    fn object_signature(&self, object: &ObjectShape) -> String {
        let mut sig = String::from("{");
        for (key, field) in &object.fields {
            let optional = if object.is_optional(field) { "?" } else { "" };
            sig.push_str(&format!("{key:?}{optional}:{};", self.type_signature(field)));
        }
        sig.push('}');
        sig
    }

    fn type_signature(&self, shape: &Shape) -> String {
        let mut sig = String::new();
        if let Some(object) = &shape.object {
            sig.push_str(&format!("#{}", self.by_address[&(object as *const ObjectShape)]));
        }
        if let Some(array) = &shape.array {
            sig.push_str(&format!("[{}]", self.type_signature(&array.items)));
        }
        if let Some(string) = &shape.string {
            match string_kind(string, self.opts) {
                StringKind::Plain => sig.push('s'),
                StringKind::Format(f) => sig.push_str(&format!("s:{f}")),
                StringKind::Enum(values) => sig.push_str(&format!("e{values:?}")),
            }
        }
        if shape.is_number() {
            sig.push(if shape.float > 0 { 'f' } else { 'i' });
        }
        if shape.boolean > 0 {
            sig.push('b');
        }
        if shape.null > 0 {
            sig.push('n');
        }
        sig
    }
}

/// `created_at` -> `CreatedAt`, `line-items` -> `LineItems`, `9lives` -> `T9lives`.
pub fn pascal(key: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for c in key.chars() {
        if !c.is_alphanumeric() {
            upper_next = true;
            continue;
        }
        if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    match out.chars().next() {
        None => "Type".to_owned(),
        Some(c) if c.is_ascii_digit() => format!("T{out}"),
        Some(_) => out,
    }
}

/// Best-effort English singular for naming array items: `Items` -> `Item`.
fn singular(name: &str) -> String {
    let irregular = [("Children", "Child"), ("People", "Person"), ("Data", "Datum")];
    if let Some((_, one)) = irregular.iter().find(|(many, _)| *many == name) {
        return (*one).to_owned();
    }
    if let Some(stem) = name.strip_suffix("ies").filter(|s| s.len() > 1) {
        return format!("{stem}y");
    }
    if let Some(stem) = name.strip_suffix("sses") {
        return format!("{stem}ss");
    }
    if let Some(stem) = name.strip_suffix('s').filter(|s| !s.is_empty() && !s.ends_with('s')) {
        return stem.to_owned();
    }
    format!("{name}Item")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pascal_case() {
        assert_eq!(pascal("created_at"), "CreatedAt");
        assert_eq!(pascal("line-items"), "LineItems");
        assert_eq!(pascal("userID"), "UserID");
        assert_eq!(pascal("9lives"), "T9lives");
        assert_eq!(pascal("--"), "Type");
    }

    #[test]
    fn singulars() {
        assert_eq!(singular("Items"), "Item");
        assert_eq!(singular("Categories"), "Category");
        assert_eq!(singular("Addresses"), "Address");
        assert_eq!(singular("Children"), "Child");
        assert_eq!(singular("Status"), "Statu", "known limitation, still a valid name");
        assert_eq!(singular("Matrix"), "MatrixItem");
    }

    #[test]
    fn identical_objects_share_a_name() {
        let opts = Options::default();
        let shape = Shape::infer(&[json!({
            "billing": {"street": "a", "city": "b"},
            "shipping": {"street": "c", "city": "d"},
        })]);
        let plan = Plan::build(&shape, "Order", &opts);
        let names: Vec<&str> = plan.defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Billing", "Order"]);
    }

    #[test]
    fn clashing_names_are_qualified_by_parent() {
        let opts = Options::default();
        let shape = Shape::infer(&[json!({
            "user": {"meta": {"a": 1}},
            "post": {"meta": {"b": "x"}},
        })]);
        let plan = Plan::build(&shape, "Root", &opts);
        let names: Vec<&str> = plan.defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Meta", "User", "PostMeta", "Post", "Root"]);
    }
}
