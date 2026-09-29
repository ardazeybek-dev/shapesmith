//! Infer TypeScript types, Zod schemas and JSON Schema from JSON samples.
//!
//! ```
//! let out = shapesmith::generate(r#"[{"id": 1, "tags": ["a"]}, {"id": 2}]"#, &Default::default()).unwrap();
//! assert!(out.typescript.contains("tags?: string[];"));
//! ```

pub mod emit;
pub mod format;
pub mod input;
pub mod naming;
pub mod shape;

#[cfg(target_arch = "wasm32")]
mod wasm;

use serde_json::Value;

pub use input::ParseError;
use naming::{Plan, pascal};
use shape::Shape;

#[derive(Clone, Debug)]
pub struct Options {
    /// Name of the top-level type.
    pub root_name: String,
    /// Recognise date-time, date, email, uuid and URL strings.
    pub detect_formats: bool,
    /// Turn strings with a few repeating values into literal unions.
    pub detect_enums: bool,
    /// Most distinct values a string may have and still count as an enum.
    pub max_enum_values: usize,
    /// Read a top-level JSON array as a list of samples.
    pub split_top_level_arrays: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            root_name: "Root".into(),
            detect_formats: true,
            detect_enums: true,
            max_enum_values: 8,
            split_top_level_arrays: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub typescript: String,
    pub zod: String,
    pub json_schema: String,
    /// Number of samples the types were inferred from.
    pub samples: usize,
    /// Number of named object types emitted.
    pub types: usize,
}

/// Parses `text` (a JSON document, a JSON array of samples, or NDJSON) and
/// renders every target.
pub fn generate(text: &str, opts: &Options) -> Result<Output, ParseError> {
    let samples = input::parse_samples(text, opts.split_top_level_arrays)?;
    Ok(generate_from_values(&samples, opts))
}

pub fn generate_from_values(samples: &[Value], opts: &Options) -> Output {
    let root = Shape::infer(samples);
    let root_name = pascal(&opts.root_name);
    // A root that is sometimes not an object keeps `root_name` for the alias.
    let object_name =
        if emit::root_is_plain_object(&root) { root_name.clone() } else { format!("{root_name}Object") };
    let plan = Plan::build(&root, &object_name, opts);
    Output {
        typescript: emit::typescript::render(&root, &root_name, &plan, opts),
        zod: emit::zod::render(&root, &root_name, &plan, opts),
        json_schema: emit::json_schema::render(&root, &root_name, &plan, opts),
        samples: samples.len(),
        types: plan.defs.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDERS: &str = r#"[
      {"id": 1, "status": "paid", "createdAt": "2026-09-01T10:00:00Z", "email": "ada@example.com",
       "customer": {"name": "Ada", "vip": true},
       "items": [{"sku": "A-1", "qty": 2, "price": 9.5}], "coupon": null},
      {"id": 2, "status": "pending", "createdAt": "2026-09-02T11:30:00+03:00", "email": "alan@example.org",
       "customer": {"name": "Alan", "vip": false},
       "items": [{"sku": "B-2", "qty": 1, "price": 20}, {"sku": "C-3", "qty": 5, "price": 1.25}]},
      {"id": 3, "status": "paid", "createdAt": "2026-09-03T08:15:00Z", "email": "grace@example.com",
       "customer": {"name": "Grace", "vip": false},
       "items": [], "coupon": "WELCOME10"}
    ]"#;

    fn orders() -> Output {
        generate(ORDERS, &Options { root_name: "order".into(), ..Default::default() }).unwrap()
    }

    #[test]
    fn typescript_output() {
        let expected = r#"export interface Order {
  id: number;
  status: "paid" | "pending";
  /** format: date-time */
  createdAt: string;
  /** format: email */
  email: string;
  customer: Customer;
  items: Item[];
  /** present in 2 of 3 samples */
  coupon?: string | null;
}

export interface Item {
  sku: string;
  qty: number;
  price: number;
}

export interface Customer {
  name: string;
  vip: boolean;
}
"#;
        assert_eq!(orders().typescript, expected);
    }

    #[test]
    fn zod_output() {
        let zod = orders().zod;
        assert!(zod.starts_with("import { z } from \"zod\";"));
        for line in [
            "  status: z.enum([\"paid\", \"pending\"]),",
            "  createdAt: z.iso.datetime({ offset: true }),",
            "  email: z.email(),",
            "  qty: z.int(),",
            "  price: z.number(),",
            "  items: z.array(ItemSchema),",
            "  coupon: z.string().nullable().optional(),",
        ] {
            assert!(zod.contains(line), "missing {line:?} in\n{zod}");
        }
        let customer = zod.find("export const CustomerSchema").unwrap();
        let order = zod.find("export const OrderSchema").unwrap();
        assert!(customer < order, "dependencies must be declared first");
    }

    #[test]
    fn json_schema_output() {
        let schema: Value = serde_json::from_str(&orders().json_schema).unwrap();
        assert_eq!(schema["title"], "Order");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["coupon"]["type"], serde_json::json!(["string", "null"]));
        assert_eq!(schema["properties"]["items"]["items"]["$ref"], "#/$defs/Item");
        assert_eq!(schema["$defs"]["Item"]["properties"]["qty"]["type"], "integer");
        assert!(!schema["required"].as_array().unwrap().contains(&"coupon".into()));
        assert!(schema["$defs"].get("Order").is_none(), "root is inlined");
    }

    #[test]
    fn non_object_root_gets_an_alias() {
        let out = generate(r#"[{"a": 1}, null, "x"]"#, &Default::default()).unwrap();
        assert!(out.typescript.starts_with("export type Root = RootObject | string | null;"));
        assert!(
            out.zod.contains("export const RootSchema = z.union([RootObjectSchema, z.string()]).nullable();")
        );
    }

    #[test]
    fn options_turn_detection_off() {
        let opts = Options { detect_formats: false, detect_enums: false, ..Default::default() };
        let out = generate(ORDERS, &opts).unwrap();
        assert!(out.typescript.contains("  status: string;"));
        assert!(!out.typescript.contains("format:"));
    }

    #[test]
    fn awkward_keys_are_quoted() {
        let out = generate(r#"{"content-type": "x", "2fa": true, "ok_1": 1}"#, &Default::default()).unwrap();
        assert!(out.typescript.contains("  \"content-type\": string;"));
        assert!(out.typescript.contains("  \"2fa\": boolean;"));
        assert!(out.typescript.contains("  ok_1: number;"));
    }

    #[test]
    fn inherited_key_names_are_guarded_in_zod() {
        let out = generate(r#"[{"constructor": 1, "__proto__": 2}, {"__proto__": 3}]"#, &Default::default())
            .unwrap();
        assert!(out.zod.contains("z.preprocess("), "optional `constructor` needs own-keys-only input");
        assert!(out.zod.contains(r#"    ["__proto__"]: z.int(),"#));
        let required = generate(r#"{"toString": "x"}"#, &Default::default()).unwrap();
        assert!(!required.zod.contains("z.preprocess("), "a required field is always an own key");
    }

    #[test]
    fn empty_arrays_are_unknown() {
        let out = generate(r#"{"tags": []}"#, &Default::default()).unwrap();
        assert!(out.typescript.contains("tags: unknown[];"));
        assert!(out.zod.contains("tags: z.array(z.unknown()),"));
    }
}
