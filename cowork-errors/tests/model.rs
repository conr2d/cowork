use std::collections::BTreeSet;

use cowork_errors::{Code, Envelope, Stage, redact};
use serde_json::Value;

fn source_codes() -> serde_json::Map<String, Value> {
    let source: Value = serde_json::from_str(include_str!("../../errors.json")).unwrap();
    source["codes"].as_object().unwrap().clone()
}

fn code_for(dotted: &str) -> Code {
    *Code::ALL
        .iter()
        .find(|code| code.as_str() == dotted)
        .unwrap()
}

#[test]
fn enum_matches_errors_json() {
    let codes = source_codes();
    let json_codes = codes.keys().cloned().collect::<BTreeSet<_>>();
    let enum_codes = Code::ALL
        .iter()
        .map(|code| code.as_str().to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(Code::ALL.len(), json_codes.len());
    assert_eq!(enum_codes, json_codes);
}

#[test]
fn kind_mapping_is_correct() {
    for (dotted, entry) in source_codes() {
        let code = code_for(&dotted);
        let serialized = serde_json::to_value(code.kind()).unwrap();
        assert_eq!(serialized, entry["kind"]);
    }
}

#[test]
fn code_serializes_to_dotted_string() {
    let serialized = serde_json::to_string(&Code::CommonCancelled).unwrap();
    assert_eq!(serialized, "\"common.cancelled\"");
    let roundtrip: Code = serde_json::from_str(&serialized).unwrap();
    assert_eq!(roundtrip, Code::CommonCancelled);
}

#[test]
fn envelope_roundtrip() {
    let envelope = Envelope::new(Code::PreflightInsufficientDisk, Stage::Preflight)
        .with_context("requiredBytes", "17179869184");
    let serialized = serde_json::to_value(&envelope).unwrap();
    assert_eq!(serialized["stage"], "preflight");

    let roundtrip: Envelope = serde_json::from_value(serialized).unwrap();
    assert_eq!(roundtrip.code, envelope.code);
    assert_eq!(roundtrip.stage, envelope.stage);
    assert_eq!(roundtrip.context, envelope.context);
}

#[test]
fn redaction_strips_secrets() {
    let raw = "failed under /home/alice/.config after callback https://auth.example.com/cb?code=abcd1234efgh5678ijkl9012mnop3456 with Authorization: Bearer sk-verylongtokenstringthatislongerthan32chars1234";
    let redacted = redact(raw);

    assert!(!redacted.contains("alice"));
    assert!(!redacted.contains("sk-verylongtokenstringthatislongerthan32chars1234"));
    assert!(!redacted.contains("abcd1234efgh5678ijkl9012mnop3456"));
    assert!(!redacted.contains("auth.example.com"));
    assert!(redacted.contains("<url>"));
    assert!(redacted.contains("/home/<user>"));
}
