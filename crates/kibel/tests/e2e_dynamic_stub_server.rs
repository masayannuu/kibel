mod support;

use serde_json::Value;
use std::collections::HashSet;
use std::process::{Command, Output};
use support::dynamic_graphql_stub::DynamicGraphqlStubServer;

fn run_kibel_json(server: &DynamicGraphqlStubServer, args: &[&str]) -> (Output, Value) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("kibel"));
    command
        .arg("--json")
        .arg("--origin")
        .arg(server.origin())
        .arg("--team")
        .arg("acme")
        .args(args);

    for key in [
        "KIBELA_ORIGIN",
        "KIBELA_TEAM",
        "KIBELA_ACCESS_TOKEN",
        "KIBEL_TEST_GRAPHQL_RESPONSE",
        "KIBEL_TEST_CREATE_NOTE_SCHEMA_RESPONSE",
        "KIBEL_TEST_TRANSPORT_ERROR",
        "KIBEL_TEST_CAPTURE_REQUEST_PATH",
        "KIBEL_DISABLE_RUNTIME_INTROSPECTION",
        "KIBEL_ENABLE_RUNTIME_INTROSPECTION",
    ] {
        command.env_remove(key);
    }
    command.env("KIBELA_ACCESS_TOKEN", "test-token");

    let output = command.output().expect("failed to run kibel");
    let payload = serde_json::from_slice::<Value>(&output.stdout)
        .expect("kibel should always print JSON in --json mode");
    (output, payload)
}

fn assert_ok(output: &Output, payload: &Value) {
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
}

#[test]
#[allow(clippy::too_many_lines)]
fn all_resources_work_against_dynamic_contract_stub_server() {
    let server = DynamicGraphqlStubServer::start();

    let (output, payload) = run_kibel_json(&server, &["search", "note", "--query", "rust"]);
    assert_ok(&output, &payload);
    assert_eq!(
        payload["data"]["results"][0]["id"],
        Value::String("N-search".to_string())
    );

    let (output, payload) = run_kibel_json(&server, &["search", "folder", "--query", "eng"]);
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(&server, &["group", "list"]);
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(&server, &["folder", "list"]);
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(&server, &["folder", "get", "--id", "F1"]);
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &["folder", "get-from-path", "--path", "/acme/engineering"],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(&server, &["folder", "notes", "--folder-id", "F1"]);
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &[
            "folder",
            "create",
            "--group-id",
            "G1",
            "--full-name",
            "Engineering",
        ],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &["feed", "sections", "--kind", "ALL", "--group-id", "G1"],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &["comment", "create", "--note-id", "N1", "--content", "hello"],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &[
            "comment",
            "reply",
            "--comment-id",
            "C1",
            "--content",
            "hello",
        ],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
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
            "--client-mutation-id",
            "req-001",
        ],
    );
    assert_ok(&output, &payload);
    assert_eq!(
        payload["data"]["note"]["id"],
        Value::String("N-created".to_string())
    );
    assert_eq!(
        payload["data"]["meta"]["client_mutation_id"],
        Value::String("req-001".to_string())
    );

    let (output, payload) = run_kibel_json(&server, &["note", "get", "--id", "N1"]);
    assert_ok(&output, &payload);

    let (output, payload) =
        run_kibel_json(&server, &["note", "get-from-path", "--path", "/notes/N1"]);
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &[
            "note",
            "move-to-folder",
            "--id",
            "N1",
            "--from-folder",
            "G1:Old",
            "--to-folder",
            "G1:New",
        ],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &[
            "note",
            "attach-to-folder",
            "--id",
            "N1",
            "--folder",
            "G1:Engineering",
        ],
    );
    assert_ok(&output, &payload);

    let (output, payload) = run_kibel_json(
        &server,
        &[
            "note",
            "update",
            "--id",
            "N1",
            "--base-content",
            "old",
            "--new-content",
            "new",
        ],
    );
    assert_ok(&output, &payload);

    let requests = server.captured_requests();
    assert!(
        !requests.is_empty(),
        "dynamic stub server should capture at least one request"
    );

    for request in &requests {
        assert_eq!(request.path, "/api/v1");
        assert!(
            request.query.is_empty() || request.query.contains('{'),
            "graphql query should include selection set when present"
        );
        assert!(
            request.variables.is_object(),
            "graphql variables should be a JSON object"
        );
        assert!(
            request
                .accept
                .as_deref()
                .unwrap_or("")
                .contains("application/graphql-response+json"),
            "graphql requests should send Accept header for graphql-response+json"
        );
    }

    let seen_methods = requests
        .iter()
        .map(|request| request.method.as_str())
        .collect::<HashSet<_>>();
    assert!(
        seen_methods.contains("GET"),
        "trusted query path should attempt GET transport"
    );
    assert!(
        seen_methods.contains("POST"),
        "fallback and mutation paths should use POST transport"
    );

    let seen_fields = requests
        .iter()
        .filter_map(|request| request.root_field.clone())
        .collect::<HashSet<_>>();

    for field in [
        "search",
        "searchFolder",
        "groups",
        "folders",
        "notes",
        "note",
        "noteFromPath",
        "folder",
        "folderFromPath",
        "feedSections",
        "createNote",
        "createComment",
        "createCommentReply",
        "createFolder",
        "moveNoteToAnotherFolder",
        "attachNoteToFolder",
        "updateNoteContent",
    ] {
        assert!(
            seen_fields.contains(field),
            "missing request root field: {field}"
        );
    }
}

#[test]
fn graphql_run_query_works_with_guardrails() {
    let server = DynamicGraphqlStubServer::start();
    let (output, payload) = run_kibel_json(
        &server,
        &[
            "graphql",
            "run",
            "--query",
            "query FreeNote($id: ID!) { note(id: $id) { id title content } }",
            "--variables",
            "{\"id\":\"N1\"}",
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["data"]["response"]["data"]["note"]["id"], "N1");
    assert_eq!(payload["data"]["meta"]["guardrails"]["timeout_secs"], 15);
    let methods = server
        .captured_requests()
        .into_iter()
        .map(|request| request.method)
        .collect::<HashSet<_>>();
    assert!(
        methods.contains("POST"),
        "graphql run query should remain on POST transport"
    );
}

#[test]
fn graphql_run_blocks_mutation_without_allow_flag() {
    let server = DynamicGraphqlStubServer::start();
    let (output, payload) = run_kibel_json(
        &server,
        &[
            "graphql",
            "run",
            "--query",
            "mutation FreeCreateFolder($input: CreateFolderInput!) { createFolder(input: $input) { folder { id } } }",
            "--variables",
            "{\"input\":{\"folder\":{\"groupId\":\"G1\",\"folderName\":\"Engineering\"}}}",
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(payload["ok"], Value::Bool(false));
    assert_eq!(payload["error"]["code"], "INPUT_INVALID");
    assert!(payload["error"]["message"]
        .as_str()
        .expect("error message should be string")
        .contains("--allow-mutation"));
}

#[test]
fn graphql_run_allows_mutation_with_opt_in_flag() {
    let server = DynamicGraphqlStubServer::start();
    let (output, payload) = run_kibel_json(
        &server,
        &[
            "graphql",
            "run",
            "--allow-mutation",
            "--query",
            "mutation FreeCreateFolder($input: CreateFolderInput!) { createFolder(input: $input) { folder { id } } }",
            "--variables",
            "{\"input\":{\"folder\":{\"groupId\":\"G1\",\"folderName\":\"Engineering\"}}}",
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["response"]["data"]["createFolder"]["folder"]["id"],
        "F-created"
    );
}

#[test]
fn graphql_run_blocks_non_allowlisted_mutation_even_with_allow_flag() {
    let server = DynamicGraphqlStubServer::start();
    let (output, payload) = run_kibel_json(
        &server,
        &[
            "graphql",
            "run",
            "--allow-mutation",
            "--query",
            "mutation DangerousDelete($id: ID!) { deleteNote(input: { id: $id }) { clientMutationId } }",
            "--variables",
            "{\"id\":\"N1\"}",
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(payload["ok"], Value::Bool(false));
    assert_eq!(payload["error"]["code"], "INPUT_INVALID");
    assert!(payload["error"]["message"]
        .as_str()
        .expect("error message should be string")
        .contains("not allowlisted"));

    assert!(
        server.captured_requests().is_empty(),
        "blocked mutations should fail before HTTP request dispatch"
    );
}
