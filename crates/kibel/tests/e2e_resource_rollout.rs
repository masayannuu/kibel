use serde_json::{json, Value};
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

fn base_env(response: Value) -> Vec<(&'static str, String)> {
    let response = match response {
        Value::String(raw) => raw,
        other => other.to_string(),
    };
    vec![
        ("KIBELA_ORIGIN", "http://fixture.local".to_string()),
        ("KIBELA_TEAM", "acme".to_string()),
        ("KIBELA_ACCESS_TOKEN", "test-token".to_string()),
        ("KIBEL_TEST_GRAPHQL_RESPONSE", response),
    ]
}

fn unique_value(prefix: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    format!("{prefix}-{now}")
}

fn isolated_capture_path() -> String {
    let file_name = format!("{}.json", unique_value("kibel-e2e-capture"));
    std::env::temp_dir()
        .join(file_name)
        .to_string_lossy()
        .to_string()
}

#[test]
fn search_note_success() {
    let response = json!({
        "data": {
            "search": {
                "edges": [
                    {
                        "node": {
                            "document": {"id": "N1"},
                            "title": "hello",
                            "url": "https://example.kibe.la/notes/N1",
                            "contentSummaryHtml": "summary",
                            "path": "/notes/N1",
                            "author": {"account": "alice", "realName": "Alice"}
                        }
                    }
                ]
            }
        }
    });
    let (output, payload) =
        run_kibel_json(&["search", "note", "--query", "hello"], &base_env(response));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["results"][0]["id"],
        Value::String("N1".to_string())
    );
}

#[test]
fn search_note_mine_success() {
    let response = json!({
        "data": {
            "currentUser": {
                "latestNotes": {
                    "edges": [
                        {
                            "node": {
                                "id": "N-mine",
                                "title": "my note",
                                "url": "https://example.kibe.la/notes/N-mine",
                                "updatedAt": "2026-02-25T00:00:00Z",
                                "contentSummaryHtml": "summary",
                                "path": "/notes/N-mine",
                                "author": { "account": "me", "realName": "Me" }
                            }
                        }
                    ]
                }
            }
        }
    });
    let capture_path = isolated_capture_path();
    let mut envs = base_env(response);
    envs.push(("KIBEL_TEST_CAPTURE_REQUEST_PATH", capture_path.clone()));
    let (output, payload) = run_kibel_json(&["search", "note", "--mine"], &envs);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["results"][0]["id"],
        Value::String("N-mine".to_string())
    );

    let captured_raw = std::fs::read_to_string(&capture_path).expect("capture file should exist");
    let captured =
        serde_json::from_str::<Value>(&captured_raw).expect("captured request must be JSON");
    assert!(captured["query"]
        .as_str()
        .unwrap_or_default()
        .contains("query GetCurrentUserLatestNotes"));
    assert_eq!(captured["variables"]["first"], Value::Number(16.into()));
}

#[test]
fn search_folder_success() {
    let response = json!({
        "data": {
            "searchFolder": {
                "edges": [
                    {
                        "node": {
                            "name": "Engineering",
                            "fixedPath": "/acme/engineering",
                            "group": {"name": "Acme", "isPrivate": false}
                        }
                    }
                ]
            }
        }
    });

    let (output, payload) =
        run_kibel_json(&["search", "folder", "--query", "eng"], &base_env(response));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["results"][0]["name"],
        Value::String("Engineering".to_string())
    );
}

#[test]
fn group_list_success() {
    let response = json!({
        "data": {
            "groups": {
                "edges": [
                    {"node": {"id": "G1", "name": "Acme", "isDefault": true, "isArchived": false}}
                ]
            }
        }
    });

    let (output, payload) = run_kibel_json(&["group", "list"], &base_env(response));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["groups"][0]["id"],
        Value::String("G1".to_string())
    );
}

#[test]
fn folder_resources_success() {
    let list_response = json!({
        "data": {
            "folders": {
                "edges": [
                    {"node": {"id": "F1", "name": "Engineering"}}
                ]
            }
        }
    });
    let (list_output, list_payload) = run_kibel_json(&["folder", "list"], &base_env(list_response));
    assert_eq!(list_output.status.code(), Some(0));
    assert_eq!(list_payload["ok"], Value::Bool(true));

    let notes_response = json!({
        "data": {
            "notes": {
                "edges": [
                    {"node": {"id": "N1", "title": "hello", "url": "https://example/N1"}}
                ]
            }
        }
    });
    let (notes_output, notes_payload) = run_kibel_json(
        &["folder", "notes", "--folder-id", "F1"],
        &base_env(notes_response),
    );
    assert_eq!(notes_output.status.code(), Some(0));
    assert_eq!(notes_payload["ok"], Value::Bool(true));
    assert_eq!(
        notes_payload["data"]["notes"][0]["id"],
        Value::String("N1".to_string())
    );

    let get_response = json!({
        "data": {
            "folder": {
                "name": "Engineering",
                "fullName": "Acme/Engineering",
                "fixedPath": "/acme/engineering",
                "createdAt": "2026-02-23T00:00:00Z",
                "updatedAt": "2026-02-23T00:00:00Z",
                "group": {"id": "G1", "name": "Acme"},
                "folders": {"edges": []},
                "notes": {"edges": []}
            }
        }
    });
    let (get_output, get_payload) =
        run_kibel_json(&["folder", "get", "--id", "F1"], &base_env(get_response));
    assert_eq!(get_output.status.code(), Some(0));
    assert_eq!(get_payload["ok"], Value::Bool(true));

    let get_from_path_response = json!({
        "data": {
            "folderFromPath": {
                "name": "Engineering",
                "fullName": "Acme/Engineering",
                "fixedPath": "/acme/engineering",
                "createdAt": "2026-02-23T00:00:00Z",
                "updatedAt": "2026-02-23T00:00:00Z",
                "group": {"id": "G1", "name": "Acme"},
                "folders": {"edges": []},
                "notes": {"edges": []}
            }
        }
    });
    let (path_output, path_payload) = run_kibel_json(
        &["folder", "get-from-path", "--path", "/acme/engineering"],
        &base_env(get_from_path_response),
    );
    assert_eq!(path_output.status.code(), Some(0));
    assert_eq!(path_payload["ok"], Value::Bool(true));
}

#[test]
fn note_get_from_path_success() {
    let response = json!({
        "data": {
            "noteFromPath": {
                "id": "N1",
                "title": "hello",
                "content": "world",
                "url": "https://example/N1",
                "author": {"account": "alice", "realName": "Alice"},
                "folders": {"edges": []},
                "comments": {"edges": []},
                "inlineComments": {"edges": []}
            }
        }
    });

    let (output, payload) = run_kibel_json(
        &["note", "get-from-path", "--path", "/notes/N1"],
        &base_env(response),
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["note"]["id"],
        Value::String("N1".to_string())
    );
}

#[test]
fn feed_sections_success() {
    let response = json!({
        "data": {
            "feedSections": {
                "edges": [
                    {
                        "node": {
                            "date": "2026-02-23",
                            "note": {"id": "N1", "title": "hello", "contentSummaryHtml": "summary"}
                        }
                    }
                ]
            }
        }
    });

    let (output, payload) = run_kibel_json(
        &["feed", "sections", "--kind", "ALL", "--group-id", "G1"],
        &base_env(response),
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["sections"][0]["node"]["date"],
        Value::String("2026-02-23".to_string())
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn command_request_shapes_are_preserved() {
    let capture_path = isolated_capture_path();
    let mut envs = base_env(json!({ "data": { "createComment": { "comment": { "id": "C1" } } } }));
    envs.push(("KIBEL_TEST_CAPTURE_REQUEST_PATH", capture_path.clone()));

    let (comment_output, comment_payload) = run_kibel_json(
        &["comment", "create", "--content", "hello", "--note-id", "N1"],
        &envs,
    );
    assert_eq!(comment_output.status.code(), Some(0));
    assert_eq!(comment_payload["ok"], Value::Bool(true));
    let captured_raw = std::fs::read_to_string(&capture_path).expect("capture file should exist");
    let captured =
        serde_json::from_str::<Value>(&captured_raw).expect("captured request must be JSON");
    assert!(captured["query"]
        .as_str()
        .unwrap_or_default()
        .contains("mutation CreateComment"));
    assert_eq!(
        captured["variables"]["input"]["commentableId"],
        Value::String("N1".to_string())
    );

    let reply_capture_path = isolated_capture_path();
    let mut reply_envs =
        base_env(json!({ "data": { "createCommentReply": { "reply": { "id": "R1" } } } }));
    reply_envs.push((
        "KIBEL_TEST_CAPTURE_REQUEST_PATH",
        reply_capture_path.clone(),
    ));
    let (reply_output, _) = run_kibel_json(
        &[
            "comment",
            "reply",
            "--content",
            "hello",
            "--comment-id",
            "C1",
        ],
        &reply_envs,
    );
    assert_eq!(reply_output.status.code(), Some(0));
    let reply_raw =
        std::fs::read_to_string(&reply_capture_path).expect("capture file should exist");
    let reply_json =
        serde_json::from_str::<Value>(&reply_raw).expect("captured request must be JSON");
    assert!(reply_json["query"]
        .as_str()
        .unwrap_or_default()
        .contains("mutation CreateCommentReply"));
    assert_eq!(
        reply_json["variables"]["input"]["commentId"],
        Value::String("C1".to_string())
    );

    let folder_capture_path = isolated_capture_path();
    let mut folder_envs =
        base_env(json!({ "data": { "createFolder": { "folder": { "id": "F1" } } } }));
    folder_envs.push((
        "KIBEL_TEST_CAPTURE_REQUEST_PATH",
        folder_capture_path.clone(),
    ));
    let (folder_output, _) = run_kibel_json(
        &[
            "folder",
            "create",
            "--group-id",
            "G1",
            "--full-name",
            "Engineering",
        ],
        &folder_envs,
    );
    assert_eq!(folder_output.status.code(), Some(0));
    let folder_raw =
        std::fs::read_to_string(&folder_capture_path).expect("capture file should exist");
    let folder_json =
        serde_json::from_str::<Value>(&folder_raw).expect("captured request must be JSON");
    assert!(folder_json["query"]
        .as_str()
        .unwrap_or_default()
        .contains("mutation CreateFolder"));
    assert_eq!(
        folder_json["variables"]["input"]["folder"]["folderName"],
        Value::String("Engineering".to_string())
    );

    let move_capture_path = isolated_capture_path();
    let mut move_envs =
        base_env(json!({ "data": { "moveNoteToAnotherFolder": { "note": { "id": "N1" } } } }));
    move_envs.push(("KIBEL_TEST_CAPTURE_REQUEST_PATH", move_capture_path.clone()));
    let (move_output, _) = run_kibel_json(
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
        &move_envs,
    );
    assert_eq!(move_output.status.code(), Some(0));
    let move_raw = std::fs::read_to_string(&move_capture_path).expect("capture file should exist");
    let move_json =
        serde_json::from_str::<Value>(&move_raw).expect("captured request must be JSON");
    assert!(move_json["query"]
        .as_str()
        .unwrap_or_default()
        .contains("mutation MoveNoteToAnotherFolder"));
    assert_eq!(
        move_json["variables"]["input"]["fromFolder"]["folderName"],
        Value::String("Old".to_string())
    );

    let attach_capture_path = isolated_capture_path();
    let mut attach_envs =
        base_env(json!({ "data": { "attachNoteToFolder": { "note": { "id": "N1" } } } }));
    attach_envs.push((
        "KIBEL_TEST_CAPTURE_REQUEST_PATH",
        attach_capture_path.clone(),
    ));
    let (attach_output, _) = run_kibel_json(
        &[
            "note",
            "attach-to-folder",
            "--id",
            "N1",
            "--folder",
            "G1:Engineering",
        ],
        &attach_envs,
    );
    assert_eq!(attach_output.status.code(), Some(0));
    let attach_raw =
        std::fs::read_to_string(&attach_capture_path).expect("capture file should exist");
    let attach_json =
        serde_json::from_str::<Value>(&attach_raw).expect("captured request must be JSON");
    assert!(attach_json["query"]
        .as_str()
        .unwrap_or_default()
        .contains("mutation AttachNoteToFolder"));
    assert_eq!(
        attach_json["variables"]["input"]["folder"]["groupId"],
        Value::String("G1".to_string())
    );
}

#[test]
fn completion_command_generates_shell_script() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("kibel"))
        .args(["completion", "bash"])
        .output()
        .expect("failed to run completion command");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    assert!(
        stdout.contains("kibel"),
        "completion output should contain command name"
    );
}
