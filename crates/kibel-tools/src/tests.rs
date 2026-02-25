use super::*;
use serde_json::json;

fn resource(name: &str, all_variables: &[&str], required_variables: &[&str]) -> Value {
    json!({
        "name": name,
        "kind": "query",
        "operation": name,
        "all_variables": all_variables,
        "required_variables": required_variables,
        "graphql_file": format!("endpoint:query.{name}"),
        "client_method": name,
        "document": format!("query {name} {{ {name} }}"),
    })
}

#[test]
fn normalize_string_list_trims_and_deduplicates() {
    let normalized = normalize_string_list(&json!([" title ", "", "title", 5, "5", "  "]), "test")
        .expect("normalize_string_list should succeed");

    assert_eq!(normalized, vec!["title".to_string(), "5".to_string()]);
}

#[test]
fn normalize_resource_snapshot_sorts_by_resource_name() {
    let payload = json!({
        "schema_contract_version": 1,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": "schema/introspection/resource_contracts.endpoint.snapshot.json",
            "captured_at": "2026-02-23T00:00:00Z",
            "origin": "https://example.kibe.la",
            "endpoint": "https://example.kibe.la/api/v1",
            "upstream_commit": "",
        },
        "resources": [
            resource("searchNote", &["query"], &["query"]),
            resource("attachNoteToFolder", &["id"], &["id"]),
        ]
    });

    let normalized = normalize_resource_snapshot(&payload).expect("snapshot must normalize");
    let names = normalized
        .resources
        .iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["attachNoteToFolder", "searchNote"]);
}

#[test]
fn normalize_resource_snapshot_rejects_required_variables_outside_all_variables() {
    let payload = json!({
        "schema_contract_version": 1,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": "schema/introspection/resource_contracts.endpoint.snapshot.json",
            "captured_at": "2026-02-23T00:00:00Z",
            "origin": "https://example.kibe.la",
            "endpoint": "https://example.kibe.la/api/v1",
            "upstream_commit": "",
        },
        "resources": [
            resource("searchNote", &["query"], &["query", "groupId"]),
        ]
    });

    let error = normalize_resource_snapshot(&payload).expect_err("invalid required vars must fail");
    assert!(
        error
            .to_string()
            .contains("required vars not in all_variables"),
        "unexpected error: {error}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn build_endpoint_snapshot_from_introspection_extracts_required_args() {
    let payload = json!({
        "data": {
            "__schema": {
                "queryType": {
                    "fields": [
                        {
                            "name": "search",
                            "args": [
                                {
                                    "name": "query",
                                    "defaultValue": null,
                                    "type": {
                                        "kind": "NON_NULL",
                                        "name": null,
                                        "ofType": {
                                            "kind": "SCALAR",
                                            "name": "String",
                                            "ofType": null
                                        }
                                    }
                                },
                                {
                                    "name": "first",
                                    "defaultValue": "16",
                                    "type": {
                                        "kind": "SCALAR",
                                        "name": "Int",
                                        "ofType": null
                                    }
                                }
                            ],
                            "type": {
                                "kind": "OBJECT",
                                "name": "SearchConnection",
                                "ofType": null
                            }
                        }
                    ]
                },
                "mutationType": {
                    "fields": [
                        {
                            "name": "createNote",
                            "args": [
                                {
                                    "name": "input",
                                    "defaultValue": null,
                                    "type": {
                                        "kind": "NON_NULL",
                                        "name": null,
                                        "ofType": {
                                            "kind": "INPUT_OBJECT",
                                            "name": "CreateNoteInput",
                                            "ofType": null
                                        }
                                    }
                                }
                            ],
                            "type": {
                                "kind": "OBJECT",
                                "name": "CreateNotePayload",
                                "ofType": null
                            }
                        }
                    ]
                },
                "types": [
                    {
                        "name": "CreateNoteInput",
                        "inputFields": [
                            { "name": "title" },
                            { "name": "content" },
                            { "name": "groupIds" },
                            { "name": "coediting" },
                            { "name": "draft" }
                        ]
                    },
                    {
                        "name": "CreateNotePayload",
                        "fields": [
                            {
                                "name": "note",
                                "args": [],
                                "type": { "kind": "OBJECT", "name": "Note", "ofType": null }
                            },
                            {
                                "name": "clientMutationId",
                                "args": [],
                                "type": { "kind": "SCALAR", "name": "String", "ofType": null }
                            }
                        ]
                    },
                    {
                        "name": "Note",
                        "fields": [
                            {
                                "name": "id",
                                "args": [],
                                "type": { "kind": "SCALAR", "name": "ID", "ofType": null }
                            },
                            {
                                "name": "title",
                                "args": [],
                                "type": { "kind": "SCALAR", "name": "String", "ofType": null }
                            }
                        ]
                    }
                ]
            }
        }
    });
    let definitions = vec![
        ResourceDefinition {
            name: "searchNote",
            kind: "query",
            field: "search",
            client_method: "search_note",
        },
        ResourceDefinition {
            name: "createNote",
            kind: "mutation",
            field: "createNote",
            client_method: "create_note",
        },
    ];

    let snapshot = build_endpoint_snapshot_from_introspection(
        &definitions,
        &payload,
        "https://example.kibe.la",
        "https://example.kibe.la/api/v1",
        "2026-02-24T00:00:00Z",
    )
    .expect("snapshot must build");
    let resources = snapshot
        .get("resources")
        .and_then(Value::as_array)
        .expect("resources should be array");
    let search = resources
        .iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some("searchNote"))
        .expect("searchNote should exist");
    let required = search
        .get("required_variables")
        .and_then(Value::as_array)
        .expect("required_variables should be array");
    assert_eq!(required, &vec![Value::String("query".to_string())]);
    assert!(
        search
            .get("document")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("query SearchNote")),
        "generated snapshot should include document"
    );
}

fn endpoint_resource_json(definition: &ResourceDefinition, with_document: bool) -> Value {
    let mut object = serde_json::Map::new();
    object.insert(
        "name".to_string(),
        Value::String(definition.name.to_string()),
    );
    object.insert(
        "kind".to_string(),
        Value::String(definition.kind.to_string()),
    );
    object.insert(
        "field".to_string(),
        Value::String(definition.field.to_string()),
    );
    object.insert(
        "operation".to_string(),
        Value::String(to_pascal_case(definition.name)),
    );
    object.insert(
        "client_method".to_string(),
        Value::String(definition.client_method.to_string()),
    );
    object.insert("all_variables".to_string(), Value::Array(Vec::new()));
    object.insert("required_variables".to_string(), Value::Array(Vec::new()));
    if with_document {
        object.insert(
            "document".to_string(),
            Value::String(format!(
                "query {} {{ {} }}",
                to_pascal_case(definition.name),
                definition.field
            )),
        );
    }
    Value::Object(object)
}

#[test]
fn parse_endpoint_snapshot_strict_rejects_missing_document() {
    let resources = resource_definitions()
        .iter()
        .map(|definition| endpoint_resource_json(definition, false))
        .collect::<Vec<_>>();
    let payload = json!({
        "captured_at": "2026-02-25T00:00:00Z",
        "origin": "https://example.kibe.la",
        "endpoint": "https://example.kibe.la/api/v1",
        "resources": resources,
    });

    let error = parse_endpoint_snapshot(&payload).expect_err("missing document should fail");
    assert!(
        error.to_string().contains("missing `document`"),
        "unexpected error: {error}"
    );
}

#[test]
fn build_create_note_schema_from_endpoint_introspection_extracts_fields() {
    let payload = json!({
        "data": {
            "__schema": {
                "types": [
                    {
                        "name": "CreateNoteInput",
                        "inputFields": [
                            { "name": "title" },
                            { "name": "content" },
                            { "name": "groupIds" },
                            { "name": "coediting" },
                            { "name": "draft" }
                        ]
                    },
                    {
                        "name": "CreateNotePayload",
                        "fields": [
                            { "name": "note" },
                            { "name": "clientMutationId" }
                        ]
                    },
                    {
                        "name": "Note",
                        "fields": [
                            { "name": "id" },
                            { "name": "title" }
                        ]
                    }
                ]
            }
        }
    });

    let schema = build_create_note_schema_from_endpoint_introspection(&payload)
        .expect("schema should parse");
    assert!(schema.input.iter().any(|field| field == "title"));
    assert!(schema.payload.iter().any(|field| field == "note"));
    assert!(schema.note_projection.iter().any(|field| field == "id"));
}

#[test]
fn build_create_note_snapshot_from_endpoint_snapshot_extracts_fields() {
    let payload = json!({
        "captured_at": "2026-02-25T00:00:00Z",
        "origin": "https://example.kibe.la",
        "endpoint": "https://example.kibe.la/api/v1",
        "create_note_schema": {
            "input_fields": ["title", "content", "groupIds", "coediting", "draft"],
            "payload_fields": ["note", "clientMutationId"],
            "note_projection_fields": ["id", "title"],
        }
    });
    let snapshot =
        build_create_note_snapshot_from_endpoint_snapshot(&payload).expect("snapshot should build");
    assert_eq!(
        snapshot
            .pointer("/create_note_payload_fields/0")
            .and_then(Value::as_str),
        Some("note")
    );
}

#[test]
fn compute_resource_contract_diff_detects_breaking_changes() {
    let base_payload = json!({
        "schema_contract_version": 1,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": "schema/introspection/resource_contracts.endpoint.snapshot.json",
            "captured_at": "2026-02-23T00:00:00Z",
            "origin": "https://example.kibe.la",
            "endpoint": "https://example.kibe.la/api/v1",
            "upstream_commit": "",
        },
        "resources": [{
            "name": "searchNote",
            "kind": "query",
            "operation": "SearchNote",
            "all_variables": ["query", "first"],
            "required_variables": ["query"],
            "graphql_file": "endpoint:query.search",
            "client_method": "search_note",
            "document": "query SearchNote($query: String!) { search(query: $query) { __typename } }"
        }]
    });
    let target_payload = json!({
        "schema_contract_version": 1,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": "schema/introspection/resource_contracts.endpoint.snapshot.json",
            "captured_at": "2026-02-24T00:00:00Z",
            "origin": "https://example.kibe.la",
            "endpoint": "https://example.kibe.la/api/v1",
            "upstream_commit": "",
        },
        "resources": [{
            "name": "searchNote",
            "kind": "mutation",
            "operation": "SearchNote",
            "all_variables": ["query", "first"],
            "required_variables": ["query", "first"],
            "graphql_file": "endpoint:mutation.searchFolder",
            "client_method": "search_note",
            "document": "mutation SearchNote($query: String!) { searchFolder(query: $query) { __typename } }"
        }]
    });
    let base = normalize_resource_snapshot(&base_payload).expect("base should parse");
    let target = normalize_resource_snapshot(&target_payload).expect("target should parse");
    let diff = compute_resource_contract_diff(&base, &target);
    assert!(
        diff.breaking
            .iter()
            .any(|item| item.contains("kind changed")),
        "expected kind change in diff: {:?}",
        diff.breaking
    );
    assert!(
        diff.breaking
            .iter()
            .any(|item| item.contains("root field changed")),
        "expected root field change in diff: {:?}",
        diff.breaking
    );
    assert!(
        diff.breaking
            .iter()
            .any(|item| item.contains("required variable(s) added")),
        "expected required variable addition in diff: {:?}",
        diff.breaking
    );
}

#[test]
fn compute_resource_contract_diff_marks_variable_removal_as_breaking() {
    let base_payload = json!({
        "schema_contract_version": 1,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": "schema/introspection/resource_contracts.endpoint.snapshot.json",
            "captured_at": "2026-02-23T00:00:00Z",
            "origin": "https://example.kibe.la",
            "endpoint": "https://example.kibe.la/api/v1",
            "upstream_commit": "",
        },
        "resources": [{
            "name": "searchNote",
            "kind": "query",
            "operation": "SearchNote",
            "all_variables": ["query", "first", "after"],
            "required_variables": ["query"],
            "graphql_file": "endpoint:query.search",
            "client_method": "search_note",
            "document": "query SearchNote($query: String!) { search(query: $query) { __typename } }"
        }]
    });
    let target_payload = json!({
        "schema_contract_version": 1,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": "schema/introspection/resource_contracts.endpoint.snapshot.json",
            "captured_at": "2026-02-24T00:00:00Z",
            "origin": "https://example.kibe.la",
            "endpoint": "https://example.kibe.la/api/v1",
            "upstream_commit": "",
        },
        "resources": [{
            "name": "searchNote",
            "kind": "query",
            "operation": "SearchNote",
            "all_variables": ["query"],
            "required_variables": ["query"],
            "graphql_file": "endpoint:query.search",
            "client_method": "search_note",
            "document": "query SearchNote($query: String!) { search(query: $query) { __typename } }"
        }]
    });
    let base = normalize_resource_snapshot(&base_payload).expect("base should parse");
    let target = normalize_resource_snapshot(&target_payload).expect("target should parse");
    let diff = compute_resource_contract_diff(&base, &target);
    assert!(
        diff.breaking
            .iter()
            .any(|item| item.contains("variable(s) removed")),
        "expected variable removal in diff: {:?}",
        diff.breaking
    );
}

#[test]
fn resource_contract_diff_json_contains_counts_and_items() {
    let diff = ResourceContractDiffResult {
        breaking: vec!["breaking change".to_string()],
        notes: vec!["non breaking note".to_string(), "another note".to_string()],
    };
    let payload = resource_contract_diff_json(&diff);
    assert_eq!(
        payload.get("breaking_count").and_then(Value::as_u64),
        Some(1),
        "breaking_count should reflect breaking item length"
    );
    assert_eq!(
        payload.get("notes_count").and_then(Value::as_u64),
        Some(2),
        "notes_count should reflect notes length"
    );
    assert_eq!(
        payload.pointer("/breaking/0").and_then(Value::as_str),
        Some("breaking change")
    );
}
