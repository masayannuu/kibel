use serde_json::{json, Value};
use std::fmt::Write;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_kibel_json(args: &[&str], envs: &[(&str, String)]) -> (Output, Value) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("kibel"));
    command.arg("--json").args(args);
    for key in [
        "KIBELA_ORIGIN",
        "KIBELA_TEAM",
        "KIBELA_ACCESS_TOKEN",
        "KIBEL_TEST_GRAPHQL_RESPONSE",
        "KIBEL_TEST_CREATE_NOTE_SCHEMA_RESPONSE",
        "KIBEL_TEST_TRANSPORT_ERROR",
        "KIBEL_TEST_CAPTURE_REQUEST_PATH",
    ] {
        command.env_remove(key);
    }
    for (key, value) in envs {
        command.env(key, value);
    }

    let output = command.output().expect("failed to run kibel");
    let payload = serde_json::from_slice::<Value>(&output.stdout)
        .expect("kibel should always print JSON in --json mode");
    (output, payload)
}

fn unique_value(prefix: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    format!("{prefix}-{now}")
}

fn isolated_config_path() -> String {
    let file_name = format!("{}.toml", unique_value("kibel-e2e-config"));
    std::env::temp_dir()
        .join(file_name)
        .to_string_lossy()
        .to_string()
}

fn isolated_capture_path() -> String {
    let file_name = format!("{}.json", unique_value("kibel-e2e-capture"));
    std::env::temp_dir()
        .join(file_name)
        .to_string_lossy()
        .to_string()
}

fn write_config(
    path: &str,
    default_team: Option<&str>,
    team: Option<&str>,
    token: Option<&str>,
    origin: Option<&str>,
) {
    let mut body = String::new();
    if let Some(default_team) = default_team {
        writeln!(&mut body, "default_team = \"{default_team}\"").expect("in-memory write");
        body.push('\n');
    }
    if let Some(team) = team {
        writeln!(&mut body, "[profiles.{team}]").expect("in-memory write");
        if let Some(token) = token {
            writeln!(&mut body, "token = \"{token}\"").expect("in-memory write");
        }
        if let Some(origin) = origin {
            writeln!(&mut body, "origin = \"{origin}\"").expect("in-memory write");
        }
    }

    std::fs::write(path, body).expect("failed to write config");
}

fn fixture_note(id: &str, title: &str, content: &str) -> String {
    json!({
        "data": {
            "note": {
                "id": id,
                "title": title,
                "content": content,
            }
        }
    })
    .to_string()
}

fn fixture_create_note(
    id: &str,
    title: &str,
    content: &str,
    client_mutation_id: Option<&str>,
) -> String {
    let mut payload = json!({
        "data": {
            "createNote": {
                "note": {
                    "id": id,
                    "title": title,
                    "content": content,
                }
            }
        }
    });
    if let Some(client_mutation_id) = client_mutation_id {
        payload["data"]["createNote"]["clientMutationId"] =
            Value::String(client_mutation_id.to_string());
    }

    payload.to_string()
}

fn fixture_update_note(id: &str, title: &str, content: &str) -> String {
    json!({
        "data": {
            "updateNoteContent": {
                "note": {
                    "id": id,
                    "title": title,
                    "content": content,
                }
            }
        }
    })
    .to_string()
}

fn fixture_create_note_schema(
    input_fields: &[&str],
    payload_fields: &[&str],
    note_fields: &[&str],
) -> String {
    let input_fields = input_fields
        .iter()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();
    let payload_fields = payload_fields
        .iter()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();
    let note_fields = note_fields
        .iter()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();

    json!({
        "data": {
            "createNoteInput": {
                "inputFields": input_fields,
            },
            "createNotePayload": {
                "fields": payload_fields,
            },
            "noteType": {
                "fields": note_fields,
            }
        }
    })
    .to_string()
}

fn fixture_error(code: &str, message: &str) -> String {
    json!({
        "errors": [{
            "message": message,
            "extensions": {
                "code": code,
            }
        }]
    })
    .to_string()
}

fn assert_error(payload: &Value, code: &str, retryable: bool) {
    assert_eq!(payload["ok"], Value::Bool(false));
    assert_eq!(payload["error"]["code"], Value::String(code.to_string()));
    assert_eq!(payload["error"]["retryable"], Value::Bool(retryable));
}

fn base_env(origin: &str, response: String) -> Vec<(&'static str, String)> {
    vec![
        ("KIBELA_ORIGIN", origin.to_string()),
        ("KIBELA_TEAM", "acme".to_string()),
        ("KIBELA_ACCESS_TOKEN", "test-token".to_string()),
        ("KIBEL_TEST_GRAPHQL_RESPONSE", response),
    ]
}

#[test]
fn note_get_success_with_stub_fixture() {
    let (output, payload) = run_kibel_json(
        &["note", "get", "--id", "N1"],
        &base_env(
            "http://fixture.local",
            fixture_note("N1", "stub-title", "stub-content"),
        ),
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["note"]["id"],
        Value::String("N1".to_string())
    );
    assert_eq!(
        payload["data"]["meta"]["token_source"],
        Value::String("env".to_string())
    );
}

#[test]
fn note_get_not_found_maps_to_not_found_error() {
    let (output, payload) = run_kibel_json(
        &["note", "get", "--id", "N404"],
        &base_env(
            "http://fixture.local",
            fixture_error("NOT_FOUND", "note not found"),
        ),
    );

    assert_eq!(output.status.code(), Some(4));
    assert_error(&payload, "NOT_FOUND", false);
    assert_eq!(
        payload["error"]["details"]["graphql_code"],
        Value::String("NOT_FOUND".to_string())
    );
}

#[test]
fn note_update_precondition_failed_maps_to_conflict_exit_code() {
    let (output, payload) = run_kibel_json(
        &[
            "note",
            "update",
            "--id",
            "N1",
            "--base-content",
            "stale",
            "--new-content",
            "updated-content",
        ],
        &base_env(
            "http://fixture.local",
            fixture_error("PRECONDITION_FAILED", "base content mismatch"),
        ),
    );

    assert_eq!(output.status.code(), Some(5));
    assert_error(&payload, "PRECONDITION_FAILED", false);
}

#[test]
fn note_create_idempotency_conflict_is_mapped() {
    let (output, payload) = run_kibel_json(
        &[
            "note",
            "create",
            "--title",
            "hello",
            "--content",
            "world",
            "--group-id",
            "G1",
        ],
        &base_env(
            "http://fixture.local",
            fixture_error("IDEMPOTENCY_CONFLICT", "idempotency conflict"),
        ),
    );

    assert_eq!(output.status.code(), Some(5));
    assert_error(&payload, "IDEMPOTENCY_CONFLICT", false);
}

#[test]
fn note_create_success_returns_note_without_idempotency_status() {
    let (output, payload) = run_kibel_json(
        &[
            "note",
            "create",
            "--title",
            "hello",
            "--content",
            "world",
            "--group-id",
            "G1",
        ],
        &base_env(
            "http://fixture.local",
            fixture_create_note("N1", "hello", "world", None),
        ),
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["note"]["id"],
        Value::String("N1".to_string())
    );
    assert_eq!(payload["data"]["meta"]["client_mutation_id"], Value::Null);
}

#[test]
fn note_create_returns_client_mutation_id_when_present() {
    let (output, payload) = run_kibel_json(
        &[
            "note",
            "create",
            "--title",
            "hello",
            "--content",
            "world",
            "--group-id",
            "G1",
            "--client-mutation-id",
            "cmid-1",
        ],
        &base_env(
            "http://fixture.local",
            fixture_create_note("N1", "hello", "world", Some("cmid-1")),
        ),
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["meta"]["client_mutation_id"],
        Value::String("cmid-1".to_string())
    );
}

#[test]
fn auth_failed_when_no_token_in_any_source() {
    let config_path = isolated_config_path();
    let team = unique_value("no-token-team");
    let (output, payload) = run_kibel_json(
        &[
            "--config-path",
            &config_path,
            "--team",
            &team,
            "note",
            "get",
            "--id",
            "N1",
        ],
        &[
            ("KIBELA_ORIGIN", "http://fixture.local".to_string()),
            (
                "KIBEL_TEST_GRAPHQL_RESPONSE",
                fixture_note("N1", "stub-title", "stub-content"),
            ),
        ],
    );

    assert_eq!(output.status.code(), Some(3));
    assert_error(&payload, "AUTH_FAILED", false);
}

#[test]
fn auth_failed_from_unauthenticated_error_code() {
    let (output, payload) = run_kibel_json(
        &["note", "get", "--id", "N1"],
        &base_env(
            "http://fixture.local",
            fixture_error("UNAUTHENTICATED", "bad token"),
        ),
    );

    assert_eq!(output.status.code(), Some(3));
    assert_error(&payload, "AUTH_FAILED", false);
}

#[test]
fn throttled_rewrite_required_from_request_limit_exceeded() {
    let (output, payload) = run_kibel_json(
        &["note", "get", "--id", "N1"],
        &base_env(
            "http://fixture.local",
            fixture_error("REQUEST_LIMIT_EXCEEDED", "rewrite required"),
        ),
    );

    assert_eq!(output.status.code(), Some(7));
    assert_error(&payload, "THROTTLED_REWRITE_REQUIRED", false);
}

#[test]
fn throttled_retryable_from_token_budget_exhausted() {
    let (output, payload) = run_kibel_json(
        &["note", "get", "--id", "N1"],
        &base_env(
            "http://fixture.local",
            fixture_error("TOKEN_BUDGET_EXHAUSTED", "budget exhausted"),
        ),
    );

    assert_eq!(output.status.code(), Some(6));
    assert_error(&payload, "THROTTLED_RETRYABLE", true);
}

#[test]
fn throttled_retryable_from_team_budget_exhausted() {
    let (output, payload) = run_kibel_json(
        &["note", "get", "--id", "N1"],
        &base_env(
            "http://fixture.local",
            fixture_error("TEAM_BUDGET_EXHAUSTED", "team budget exhausted"),
        ),
    );

    assert_eq!(output.status.code(), Some(6));
    assert_error(&payload, "THROTTLED_RETRYABLE", true);
}

#[test]
fn transport_error_when_stub_transport_forced() {
    let config_path = isolated_config_path();
    let team = unique_value("transport-team");
    let (output, payload) = run_kibel_json(
        &[
            "--config-path",
            &config_path,
            "--team",
            &team,
            "note",
            "get",
            "--id",
            "N1",
        ],
        &[
            ("KIBELA_ORIGIN", "http://fixture.local".to_string()),
            ("KIBELA_ACCESS_TOKEN", "test-token".to_string()),
            (
                "KIBEL_TEST_TRANSPORT_ERROR",
                "forced transport error".to_string(),
            ),
        ],
    );

    assert_eq!(output.status.code(), Some(6));
    assert_error(&payload, "TRANSPORT_ERROR", true);
}

#[test]
fn contract_request_shape_for_create_note_is_preserved() {
    let capture_path = isolated_capture_path();
    let envs = vec![
        ("KIBELA_ORIGIN", "http://fixture.local".to_string()),
        ("KIBELA_TEAM", "acme".to_string()),
        ("KIBELA_ACCESS_TOKEN", "test-token".to_string()),
        (
            "KIBEL_TEST_GRAPHQL_RESPONSE",
            fixture_create_note("N1", "hello", "world", Some("cmid-1")),
        ),
        (
            "KIBEL_TEST_CREATE_NOTE_SCHEMA_RESPONSE",
            fixture_create_note_schema(
                &[
                    "title",
                    "content",
                    "groupIds",
                    "coediting",
                    "draft",
                    "folders",
                    "authorId",
                    "publishedAt",
                    "clientMutationId",
                ],
                &["clientMutationId", "note"],
                &["id", "title", "content"],
            ),
        ),
        ("KIBEL_TEST_CAPTURE_REQUEST_PATH", capture_path.clone()),
    ];

    let (output, payload) = run_kibel_json(
        &[
            "note",
            "create",
            "--title",
            "hello",
            "--content",
            "world",
            "--group-id",
            "G1",
            "--draft",
            "--coediting",
            "--folder",
            "G1:Engineering",
            "--author-id",
            "U1",
            "--published-at",
            "2026-02-23T00:00:00Z",
            "--client-mutation-id",
            "cmid-1",
        ],
        &envs,
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));

    let captured_raw = std::fs::read_to_string(&capture_path).expect("capture file should exist");
    let captured =
        serde_json::from_str::<Value>(&captured_raw).expect("captured request must be JSON");

    assert!(captured["query"]
        .as_str()
        .expect("query must be string")
        .contains("mutation CreateNote"));
    assert!(
        !captured["query"]
            .as_str()
            .expect("query must be string")
            .contains("url"),
        "query should respect introspected note fields"
    );
    assert_eq!(captured["variables"]["input"]["groupIds"], json!(["G1"]));
    assert_eq!(
        captured["variables"]["input"]["coediting"],
        Value::Bool(true)
    );
    assert_eq!(captured["variables"]["input"]["draft"], Value::Bool(true));
    assert_eq!(
        captured["variables"]["input"]["folders"],
        json!([{ "groupId": "G1", "folderName": "Engineering" }])
    );
    assert_eq!(
        captured["variables"]["input"]["authorId"],
        Value::String("U1".to_string())
    );
    assert_eq!(
        captured["variables"]["input"]["publishedAt"],
        Value::String("2026-02-23T00:00:00Z".to_string())
    );
    assert_eq!(
        captured["variables"]["input"]["clientMutationId"],
        Value::String("cmid-1".to_string())
    );
    let input = captured["variables"]["input"]
        .as_object()
        .expect("input should be an object");
    assert!(
        !input.contains_key("idempotencyKey"),
        "idempotencyKey must not be sent"
    );
}

#[test]
fn contract_request_shape_for_update_note_is_preserved() {
    let capture_path = isolated_capture_path();
    let envs = vec![
        ("KIBELA_ORIGIN", "http://fixture.local".to_string()),
        ("KIBELA_TEAM", "acme".to_string()),
        ("KIBELA_ACCESS_TOKEN", "test-token".to_string()),
        (
            "KIBEL_TEST_GRAPHQL_RESPONSE",
            fixture_update_note("N1", "stub-N1", "new-content"),
        ),
        ("KIBEL_TEST_CAPTURE_REQUEST_PATH", capture_path.clone()),
    ];

    let (output, payload) = run_kibel_json(
        &[
            "note",
            "update",
            "--id",
            "N1",
            "--base-content",
            "seed-v1",
            "--new-content",
            "seed-v2",
        ],
        &envs,
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));

    let captured_raw = std::fs::read_to_string(&capture_path).expect("capture file should exist");
    let captured =
        serde_json::from_str::<Value>(&captured_raw).expect("captured request must be JSON");

    assert!(captured["query"]
        .as_str()
        .expect("query must be string")
        .contains("mutation UpdateNoteContent"));
    assert_eq!(
        captured["variables"]["input"]["baseContent"],
        Value::String("seed-v1".to_string())
    );
    assert_eq!(
        captured["variables"]["input"]["newContent"],
        Value::String("seed-v2".to_string())
    );
}

#[test]
fn config_set_team_and_profiles_are_machine_readable_without_token_leak() {
    let config_path = isolated_config_path();
    write_config(
        &config_path,
        Some("acme"),
        Some("acme"),
        Some("super-secret-token"),
        Some("https://acme.kibe.la"),
    );

    let (set_output, set_payload) = run_kibel_json(
        &[
            "--config-path",
            &config_path,
            "config",
            "set",
            "team",
            "team-b",
        ],
        &[],
    );
    assert_eq!(set_output.status.code(), Some(0));
    assert_eq!(
        set_payload["data"]["default_team"],
        Value::String("team-b".to_string())
    );

    let (profiles_output, profiles_payload) =
        run_kibel_json(&["--config-path", &config_path, "config", "profiles"], &[]);
    assert_eq!(profiles_output.status.code(), Some(0));
    assert_eq!(
        profiles_payload["data"]["default_team"],
        Value::String("team-b".to_string())
    );

    let profiles = profiles_payload["data"]["profiles"]
        .as_array()
        .expect("profiles should be array");
    assert!(!profiles.is_empty(), "profiles should not be empty");
    assert_eq!(
        profiles[0]["has_token"],
        Value::Bool(true),
        "token should be represented as metadata only"
    );
    assert_eq!(
        profiles[0]["origin"],
        Value::String("https://acme.kibe.la".to_string())
    );

    let stdout = String::from_utf8_lossy(&profiles_output.stdout);
    assert!(
        !stdout.contains("super-secret-token"),
        "raw token should never appear in command output"
    );
}

#[test]
fn note_get_uses_profile_origin_when_origin_flag_is_missing() {
    let config_path = isolated_config_path();
    write_config(
        &config_path,
        Some("acme"),
        Some("acme"),
        None,
        Some("https://acme.kibe.la"),
    );

    let (output, payload) = run_kibel_json(
        &[
            "--config-path",
            &config_path,
            "--team",
            "acme",
            "note",
            "get",
            "--id",
            "N1",
        ],
        &[
            ("KIBELA_ACCESS_TOKEN", "env-token".to_string()),
            (
                "KIBEL_TEST_GRAPHQL_RESPONSE",
                fixture_note("N1", "stub-title", "stub-content"),
            ),
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        payload["data"]["meta"]["origin"],
        Value::String("https://acme.kibe.la".to_string())
    );
    assert_eq!(
        payload["data"]["meta"]["team"],
        Value::String("acme".to_string())
    );
}

#[test]
fn note_get_uses_default_team_origin_when_team_is_missing() {
    let config_path = isolated_config_path();
    write_config(
        &config_path,
        Some("acme"),
        Some("acme"),
        None,
        Some("https://default.kibe.la"),
    );

    let (output, payload) = run_kibel_json(
        &["--config-path", &config_path, "note", "get", "--id", "N1"],
        &[
            ("KIBELA_ACCESS_TOKEN", "env-token".to_string()),
            (
                "KIBEL_TEST_GRAPHQL_RESPONSE",
                fixture_note("N1", "stub-title", "stub-content"),
            ),
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        payload["data"]["meta"]["team"],
        Value::String("acme".to_string())
    );
    assert_eq!(
        payload["data"]["meta"]["origin"],
        Value::String("https://default.kibe.la".to_string())
    );
}

#[test]
fn with_token_flag_does_not_break_config_profiles() {
    let config_path = isolated_config_path();
    write_config(
        &config_path,
        Some("acme"),
        Some("acme"),
        Some("secret-token"),
        Some("https://acme.kibe.la"),
    );

    let (output, payload) = run_kibel_json(
        &[
            "--with-token",
            "--config-path",
            &config_path,
            "config",
            "profiles",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
}

#[test]
fn with_token_flag_does_not_break_version() {
    let (output, payload) = run_kibel_json(&["--with-token", "version"], &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert!(
        payload["data"]["version"].as_str().is_some(),
        "version should be present"
    );
}
