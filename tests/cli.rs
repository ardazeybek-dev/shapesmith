use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shapesmith"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn reads_ndjson_from_stdin() {
    let out = run(&["--name", "event"], "{\"id\": 1}\n{\"id\": 2, \"tag\": \"x\"}\n");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("export interface Event {"), "{text}");
    assert!(text.contains("  tag?: string;"));
}

#[test]
fn schema_target_prints_json() {
    let out = run(&["-t", "schema"], r#"{"ok": true}"#);
    let schema: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(schema["properties"]["ok"]["type"], "boolean");
}

#[test]
fn invalid_json_fails_with_line_number() {
    let out = run(&[], "{\"a\": 1}\n{nope}\n");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("stdin: line 2"));
}

#[test]
fn bad_flag_is_a_usage_error() {
    let out = run(&["--target", "java"], "{}");
    assert_eq!(out.status.code(), Some(2));
}
