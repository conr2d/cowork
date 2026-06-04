use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../errors.json");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let errors_path = Path::new(&manifest_dir).join("../errors.json");
    let raw = fs::read_to_string(errors_path)?;
    let json: Value = serde_json::from_str(&raw)?;
    let codes = json
        .get("codes")
        .and_then(Value::as_object)
        .expect("errors.json must contain a codes object");

    let mut sorted = BTreeMap::new();
    for (code, entry) in codes {
        let kind = entry
            .get("kind")
            .and_then(Value::as_str)
            .expect("each code must have a string kind");
        let context_keys = entry
            .get("contextKeys")
            .and_then(Value::as_array)
            .expect("each code must have contextKeys");
        let context_keys = context_keys
            .iter()
            .map(|key| {
                key.as_str()
                    .expect("contextKeys entries must be strings")
                    .to_string()
            })
            .collect::<Vec<_>>();
        sorted.insert(code.to_string(), (kind.to_string(), context_keys));
    }

    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ::serde::Serialize, ::serde::Deserialize)]\n");
    out.push_str("pub enum Code {\n");
    for code in sorted.keys() {
        out.push_str(&format!("    #[serde(rename = \"{code}\")]\n"));
        out.push_str(&format!("    {},\n", variant_name(code)));
    }
    out.push_str("}\n\n");

    out.push_str("impl Code {\n");
    out.push_str("    pub const ALL: &'static [Code] = &[\n");
    for code in sorted.keys() {
        out.push_str(&format!("        Code::{},\n", variant_name(code)));
    }
    out.push_str("    ];\n\n");

    out.push_str("    pub fn as_str(&self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for code in sorted.keys() {
        out.push_str(&format!(
            "            Code::{} => \"{}\",\n",
            variant_name(code),
            code
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    out.push_str("    pub fn kind(&self) -> crate::Kind {\n");
    out.push_str("        match self {\n");
    for (code, (kind, _)) in &sorted {
        out.push_str(&format!(
            "            Code::{} => crate::Kind::{},\n",
            variant_name(code),
            kind
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    out.push_str("    pub fn context_keys(&self) -> &'static [&'static str] {\n");
    out.push_str("        match self {\n");
    for (code, (_, context_keys)) in &sorted {
        let keys = context_keys
            .iter()
            .map(|key| format!("\"{key}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "            Code::{} => &[{}],\n",
            variant_name(code),
            keys
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    fs::write(Path::new(&env::var("OUT_DIR")?).join("codes.rs"), out)?;
    Ok(())
}

fn variant_name(code: &str) -> String {
    code.split(['.', '_'])
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
