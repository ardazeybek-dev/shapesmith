//! JavaScript bindings, compiled only for `wasm32`.

use wasm_bindgen::prelude::*;

use crate::Options;

#[wasm_bindgen(typescript_custom_section)]
const TS_OPTIONS: &str = r#"
export interface GenerateOptions {
  /** Name of the top-level type. Default "Root". */
  rootName?: string;
  /** Recognise date-time, date, email, uuid and URL strings. Default true. */
  detectFormats?: boolean;
  /** Turn strings with a few repeating values into literal unions. Default true. */
  detectEnums?: boolean;
  /** Most distinct values a string may have and still count as an enum. Default 8. */
  maxEnumValues?: number;
  /** Read a top-level JSON array as a list of samples. Default true. */
  splitTopLevelArrays?: boolean;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "GenerateOptions")]
    pub type GenerateOptions;
}

#[wasm_bindgen(getter_with_clone)]
pub struct GenerateResult {
    pub typescript: String,
    pub zod: String,
    #[wasm_bindgen(js_name = jsonSchema)]
    pub json_schema: String,
    /// Number of samples the types were inferred from.
    pub samples: usize,
    /// Number of named object types emitted.
    pub types: usize,
}

/// Infers types from JSON text: one document, an array of samples, or NDJSON.
/// Throws an `Error` with a readable message when the input is not valid JSON.
#[wasm_bindgen]
pub fn generate(input: &str, options: Option<GenerateOptions>) -> Result<GenerateResult, JsError> {
    let opts = options.map(|o| read_options(&o)).unwrap_or_default();
    let out = crate::generate(input, &opts).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(GenerateResult {
        typescript: out.typescript,
        zod: out.zod,
        json_schema: out.json_schema,
        samples: out.samples,
        types: out.types,
    })
}

fn read_options(value: &JsValue) -> Options {
    let mut opts = Options::default();
    let get = |key: &str| js_sys::Reflect::get(value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED);
    if let Some(name) = get("rootName").as_string().filter(|s| !s.trim().is_empty()) {
        opts.root_name = name;
    }
    if let Some(b) = get("detectFormats").as_bool() {
        opts.detect_formats = b;
    }
    if let Some(b) = get("detectEnums").as_bool() {
        opts.detect_enums = b;
    }
    if let Some(n) = get("maxEnumValues").as_f64().filter(|n| *n >= 1.0) {
        opts.max_enum_values = n as usize;
    }
    if let Some(b) = get("splitTopLevelArrays").as_bool() {
        opts.split_top_level_arrays = b;
    }
    opts
}
