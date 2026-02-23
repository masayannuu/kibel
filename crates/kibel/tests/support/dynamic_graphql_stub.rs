use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub path: String,
    pub root_field: Option<String>,
    pub query: String,
    pub variables: Value,
}

#[derive(Debug, Clone)]
struct ResourceSpec {
    required_variables: Vec<String>,
}

#[derive(Debug, Clone)]
struct CreateNoteSchemaSnapshot {
    input: Vec<String>,
    payload: Vec<String>,
    note_projection: Vec<String>,
}

#[derive(Debug)]
struct ServerState {
    resource_specs_by_field: HashMap<String, ResourceSpec>,
    create_note_schema: CreateNoteSchemaSnapshot,
    captured_requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

pub struct DynamicGraphqlStubServer {
    origin: String,
    captured_requests: Arc<Mutex<Vec<CapturedRequest>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DynamicGraphqlStubServer {
    pub fn start() -> Self {
        let repo_root = repo_root();
        let state = ServerState {
            resource_specs_by_field: load_resource_specs(&repo_root)
                .expect("resource contracts should load"),
            create_note_schema: load_create_note_schema_snapshot(&repo_root)
                .expect("create-note schema should load"),
            captured_requests: Arc::new(Mutex::new(Vec::new())),
        };

        assert_eq!(
            state.resource_specs_by_field.len(),
            17,
            "resource contract snapshot should contain 17 resources"
        );

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind stub server");
        listener
            .set_nonblocking(true)
            .expect("failed to set non-blocking listener");
        let local_addr = listener
            .local_addr()
            .expect("listener should expose local addr");

        let origin = format!("http://{local_addr}");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_state = state;
        let captured_requests = Arc::clone(&thread_state.captured_requests);

        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(error) = handle_connection(stream, &thread_state) {
                            eprintln!("dynamic stub server connection error: {error}");
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => {
                        eprintln!("dynamic stub server accept error: {error}");
                        break;
                    }
                }
            }
        });

        Self {
            origin,
            captured_requests,
            stop,
            thread: Some(thread),
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn captured_requests(&self) -> Vec<CapturedRequest> {
        self.captured_requests
            .lock()
            .expect("captured requests mutex poisoned")
            .clone()
    }
}

impl Drop for DynamicGraphqlStubServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Debug)]
struct HttpRequest {
    path: String,
    body: Vec<u8>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate path should be crates/kibel")
        .to_path_buf()
}

fn load_resource_specs(root: &Path) -> Result<HashMap<String, ResourceSpec>, String> {
    let path = root.join("research/schema/resource_contracts.snapshot.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let payload = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let resources = payload
        .get("resources")
        .and_then(Value::as_array)
        .ok_or_else(|| "resource snapshot must include `resources` array".to_string())?;

    let mut by_field = HashMap::new();
    for item in resources {
        let object = item
            .as_object()
            .ok_or_else(|| "resource entry must be object".to_string())?;
        let graphql_file = object
            .get("graphql_file")
            .and_then(Value::as_str)
            .ok_or_else(|| "resource entry missing graphql_file".to_string())?;
        let field = graphql_file
            .split('.')
            .next_back()
            .ok_or_else(|| format!("invalid graphql_file format: {graphql_file}"))?
            .trim();

        let required_variables = object
            .get("required_variables")
            .and_then(Value::as_array)
            .ok_or_else(|| "resource entry missing required_variables".to_string())?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();

        by_field.insert(field.to_string(), ResourceSpec { required_variables });
    }

    Ok(by_field)
}

fn load_create_note_schema_snapshot(root: &Path) -> Result<CreateNoteSchemaSnapshot, String> {
    let path = root.join("research/schema/create_note_contract.snapshot.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let payload = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;

    let input_fields = normalize_string_array(
        payload
            .get("create_note_input_fields")
            .ok_or_else(|| "create_note_input_fields missing".to_string())?,
        "create_note_input_fields",
    )?;
    let payload_fields = normalize_string_array(
        payload
            .get("create_note_payload_fields")
            .ok_or_else(|| "create_note_payload_fields missing".to_string())?,
        "create_note_payload_fields",
    )?;
    let note_projection_fields = normalize_string_array(
        payload
            .get("create_note_note_projection_fields")
            .ok_or_else(|| "create_note_note_projection_fields missing".to_string())?,
        "create_note_note_projection_fields",
    )?;

    Ok(CreateNoteSchemaSnapshot {
        input: input_fields,
        payload: payload_fields,
        note_projection: note_projection_fields,
    })
}

fn normalize_string_array(value: &Value, context: &str) -> Result<Vec<String>, String> {
    let items = value
        .as_array()
        .ok_or_else(|| format!("{context} must be an array"))?;
    let mut result = Vec::new();
    for item in items {
        let raw = item
            .as_str()
            .ok_or_else(|| format!("{context} should only contain strings"))?;
        let normalized = raw.trim();
        if !normalized.is_empty() {
            result.push(normalized.to_string());
        }
    }
    Ok(result)
}

fn handle_connection(mut stream: TcpStream, state: &ServerState) -> Result<(), String> {
    stream
        .set_nonblocking(false)
        .map_err(|error| format!("failed to set blocking stream: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;

    let request = read_http_request(&mut stream)?;
    let payload = serde_json::from_slice::<Value>(&request.body)
        .map_err(|error| format!("invalid JSON request body: {error}"))?;
    let query = payload
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| "request missing string query".to_string())?
        .to_string();
    let variables = payload
        .get("variables")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let root_field = extract_root_field(&query);
    state
        .captured_requests
        .lock()
        .map_err(|_| "captured requests mutex poisoned".to_string())?
        .push(CapturedRequest {
            path: request.path,
            root_field: root_field.clone(),
            query: query.clone(),
            variables: variables.clone(),
        });

    let response_payload = route_graphql_request(&query, &variables, root_field, state);
    write_json_response(&mut stream, &response_payload)
}

fn route_graphql_request(
    query: &str,
    variables: &Value,
    root_field: Option<String>,
    state: &ServerState,
) -> Value {
    if is_create_note_schema_query(query) {
        return create_note_schema_response(&state.create_note_schema);
    }

    let Some(root_field) = root_field else {
        return graphql_error(
            "failed to extract root field from graphql query",
            "INPUT_INVALID",
        );
    };

    let Some(resource_spec) = state.resource_specs_by_field.get(&root_field) else {
        return graphql_error(
            &format!("unsupported root field: {root_field}"),
            "INPUT_INVALID",
        );
    };

    if let Some(error) = validate_required_variables(resource_spec, variables) {
        return error;
    }

    response_for_root_field(&root_field, variables)
}

fn is_create_note_schema_query(query: &str) -> bool {
    query.contains("CreateNoteSchema")
        || (query.contains("CreateNoteInput")
            && query.contains("CreateNotePayload")
            && query.contains("noteType"))
}

fn validate_required_variables(spec: &ResourceSpec, variables: &Value) -> Option<Value> {
    let Some(object) = variables.as_object() else {
        return Some(graphql_error(
            "variables must be a JSON object",
            "INPUT_INVALID",
        ));
    };

    let missing = spec
        .required_variables
        .iter()
        .filter(|name| {
            let value = object.get(*name);
            value.is_none() || value.is_some_and(Value::is_null)
        })
        .cloned()
        .collect::<Vec<_>>();

    if missing.is_empty() {
        None
    } else {
        Some(graphql_error(
            &format!("missing required variable(s): {}", missing.join(", ")),
            "INPUT_INVALID",
        ))
    }
}

fn create_note_schema_response(snapshot: &CreateNoteSchemaSnapshot) -> Value {
    json!({
        "data": {
            "createNoteInput": {
                "inputFields": snapshot
                    .input
                    .iter()
                    .map(|name| json!({ "name": name }))
                    .collect::<Vec<_>>()
            },
            "createNotePayload": {
                "fields": snapshot
                    .payload
                    .iter()
                    .map(|name| json!({ "name": name }))
                    .collect::<Vec<_>>()
            },
            "noteType": {
                "fields": snapshot
                    .note_projection
                    .iter()
                    .map(|name| json!({ "name": name }))
                    .collect::<Vec<_>>()
            }
        }
    })
}

#[allow(clippy::too_many_lines)]
fn response_for_root_field(field: &str, variables: &Value) -> Value {
    match field {
        "search" => json!({
            "data": {
                "search": {
                    "edges": [{
                        "node": {
                            "document": { "id": "N-search" },
                            "title": "search-title",
                            "url": "https://example.kibe.la/notes/N-search",
                            "contentSummaryHtml": "summary",
                            "path": "/notes/N-search",
                            "author": { "account": "stub", "realName": "Stub User" }
                        }
                    }]
                }
            }
        }),
        "searchFolder" => json!({
            "data": {
                "searchFolder": {
                    "edges": [{
                        "node": {
                            "name": "Engineering",
                            "fixedPath": "/acme/engineering",
                            "group": { "name": "Acme", "isPrivate": false }
                        }
                    }]
                }
            }
        }),
        "groups" => json!({
            "data": {
                "groups": {
                    "edges": [{
                        "node": {
                            "id": "G1",
                            "name": "Acme",
                            "isDefault": true,
                            "isArchived": false
                        }
                    }]
                }
            }
        }),
        "folders" => json!({
            "data": {
                "folders": {
                    "edges": [{
                        "node": {
                            "id": "F1",
                            "name": "Engineering"
                        }
                    }]
                }
            }
        }),
        "notes" => json!({
            "data": {
                "notes": {
                    "edges": [{
                        "node": {
                            "id": "N-folder",
                            "title": "folder-note",
                            "url": "https://example.kibe.la/notes/N-folder"
                        }
                    }]
                }
            }
        }),
        "note" => {
            let id = variable_string(variables, "/id", "N1");
            json!({
                "data": {
                    "note": {
                        "id": id,
                        "title": "note-title",
                        "content": "note-content"
                    }
                }
            })
        }
        "noteFromPath" => {
            let path = variable_string(variables, "/path", "/notes/N-path");
            json!({
                "data": {
                    "noteFromPath": {
                        "id": "N-path",
                        "title": format!("note-from-{path}"),
                        "content": "note-from-path-content",
                        "url": "https://example.kibe.la/notes/N-path",
                        "author": { "account": "stub", "realName": "Stub User" },
                        "folders": { "edges": [] },
                        "comments": { "edges": [] },
                        "inlineComments": { "edges": [] }
                    }
                }
            })
        }
        "folder" => {
            let id = variable_string(variables, "/id", "F1");
            json!({
                "data": {
                    "folder": {
                        "id": id,
                        "name": "Engineering",
                        "fullName": "Acme/Engineering",
                        "fixedPath": "/acme/engineering",
                        "createdAt": "2026-02-23T00:00:00Z",
                        "updatedAt": "2026-02-23T00:00:00Z",
                        "group": { "id": "G1", "name": "Acme" },
                        "folders": { "edges": [] },
                        "notes": { "edges": [] }
                    }
                }
            })
        }
        "folderFromPath" => {
            let path = variable_string(variables, "/path", "/acme/engineering");
            json!({
                "data": {
                    "folderFromPath": {
                        "name": "Engineering",
                        "fullName": "Acme/Engineering",
                        "fixedPath": path,
                        "createdAt": "2026-02-23T00:00:00Z",
                        "updatedAt": "2026-02-23T00:00:00Z",
                        "group": { "id": "G1", "name": "Acme" },
                        "folders": { "edges": [] },
                        "notes": { "edges": [] }
                    }
                }
            })
        }
        "feedSections" => json!({
            "data": {
                "feedSections": {
                    "edges": [{
                        "node": {
                            "date": "2026-02-23",
                            "note": {
                                "id": "N-feed",
                                "title": "feed-title",
                                "contentSummaryHtml": "feed-summary"
                            }
                        }
                    }]
                }
            }
        }),
        "createNote" => {
            let title = variable_string(variables, "/input/title", "created-title");
            let content = variable_string(variables, "/input/content", "created-content");
            let client_mutation_id = variables
                .pointer("/input/clientMutationId")
                .and_then(Value::as_str)
                .map(str::to_string);

            let mut payload = json!({
                "data": {
                    "createNote": {
                        "note": {
                            "id": "N-created",
                            "title": title,
                            "content": content,
                            "url": "https://example.kibe.la/notes/N-created"
                        }
                    }
                }
            });

            if let Some(client_mutation_id) = client_mutation_id {
                payload["data"]["createNote"]["clientMutationId"] =
                    Value::String(client_mutation_id);
            }

            payload
        }
        "createComment" => json!({
            "data": {
                "createComment": {
                    "comment": { "id": "C-created" }
                }
            }
        }),
        "createCommentReply" => json!({
            "data": {
                "createCommentReply": {
                    "reply": { "id": "R-created" }
                }
            }
        }),
        "createFolder" => json!({
            "data": {
                "createFolder": {
                    "folder": { "id": "F-created" }
                }
            }
        }),
        "moveNoteToAnotherFolder" => {
            let id = variable_string(variables, "/input/noteId", "N1");
            json!({
                "data": {
                    "moveNoteToAnotherFolder": {
                        "note": { "id": id }
                    }
                }
            })
        }
        "attachNoteToFolder" => {
            let id = variable_string(variables, "/input/noteId", "N1");
            json!({
                "data": {
                    "attachNoteToFolder": {
                        "note": { "id": id }
                    }
                }
            })
        }
        "updateNoteContent" => {
            let id = variable_string(variables, "/input/id", "N1");
            let content = variable_string(variables, "/input/newContent", "updated-content");
            json!({
                "data": {
                    "updateNoteContent": {
                        "note": {
                            "id": id,
                            "title": "updated-title",
                            "content": content
                        }
                    }
                }
            })
        }
        _ => graphql_error(&format!("unsupported root field: {field}"), "INPUT_INVALID"),
    }
}

fn variable_string(variables: &Value, pointer: &str, fallback: &str) -> String {
    variables
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn graphql_error(message: &str, code: &str) -> Value {
    json!({
        "errors": [{
            "message": message,
            "extensions": {
                "code": code,
            }
        }]
    })
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];

    let header_end = loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            return Err("connection closed before request headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };

    let headers_raw = String::from_utf8(buffer[..header_end].to_vec())
        .map_err(|error| format!("request headers are not utf-8: {error}"))?;
    let request_line = headers_raw
        .lines()
        .next()
        .ok_or_else(|| "request line missing".to_string())?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();

    let mut content_length = 0_usize;
    for line in headers_raw.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse::<usize>().unwrap_or(0);
        }
    }

    let body_start = header_end + 4;
    let mut body = if buffer.len() > body_start {
        buffer[body_start..].to_vec()
    } else {
        Vec::new()
    };

    while body.len() < content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read request body: {error}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(HttpRequest { path, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_json_response(stream: &mut TcpStream, payload: &Value) -> Result<(), String> {
    let body = payload.to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("failed to write response: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("failed to flush response: {error}"))
}

fn extract_root_field(query: &str) -> Option<String> {
    let start = query.find('{')? + 1;
    let bytes = query.as_bytes();
    let mut index = start;

    skip_whitespace(bytes, &mut index);
    let mut field = read_identifier(bytes, &mut index)?;
    skip_whitespace(bytes, &mut index);

    if bytes.get(index).copied() == Some(b':') {
        index += 1;
        skip_whitespace(bytes, &mut index);
        field = read_identifier(bytes, &mut index)?;
    }

    Some(field)
}

fn skip_whitespace(bytes: &[u8], index: &mut usize) {
    while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
}

fn read_identifier(bytes: &[u8], index: &mut usize) -> Option<String> {
    let start = *index;
    while *index < bytes.len() {
        let c = bytes[*index];
        if c.is_ascii_alphanumeric() || c == b'_' {
            *index += 1;
        } else {
            break;
        }
    }
    if *index == start {
        None
    } else {
        std::str::from_utf8(&bytes[start..*index])
            .ok()
            .map(str::to_string)
    }
}
