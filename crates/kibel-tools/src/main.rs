use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

const INTROSPECTION_QUERY: &str = r#"
query EndpointIntrospection {
  __schema {
    queryType {
      fields {
        name
        args {
          name
          defaultValue
          type {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
              }
            }
          }
        }
      }
    }
    mutationType {
      fields {
        name
        args {
          name
          defaultValue
          type {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
              }
            }
          }
        }
      }
    }
  }
}
"#;

#[derive(Debug, Clone, Copy)]
struct ResourceDefinition {
    name: &'static str,
    kind: &'static str,
    field: &'static str,
    client_method: &'static str,
}

const RESOURCE_DEFINITIONS: &[ResourceDefinition] = &[
    ResourceDefinition {
        name: "searchNote",
        kind: "query",
        field: "search",
        client_method: "search_note",
    },
    ResourceDefinition {
        name: "searchFolder",
        kind: "query",
        field: "searchFolder",
        client_method: "search_folder",
    },
    ResourceDefinition {
        name: "getGroups",
        kind: "query",
        field: "groups",
        client_method: "get_groups",
    },
    ResourceDefinition {
        name: "getFolders",
        kind: "query",
        field: "folders",
        client_method: "get_folders",
    },
    ResourceDefinition {
        name: "getNotes",
        kind: "query",
        field: "notes",
        client_method: "get_notes",
    },
    ResourceDefinition {
        name: "getNote",
        kind: "query",
        field: "note",
        client_method: "get_note",
    },
    ResourceDefinition {
        name: "getNoteFromPath",
        kind: "query",
        field: "noteFromPath",
        client_method: "get_note_from_path",
    },
    ResourceDefinition {
        name: "getFolder",
        kind: "query",
        field: "folder",
        client_method: "get_folder",
    },
    ResourceDefinition {
        name: "getFolderFromPath",
        kind: "query",
        field: "folderFromPath",
        client_method: "get_folder_from_path",
    },
    ResourceDefinition {
        name: "getFeedSections",
        kind: "query",
        field: "feedSections",
        client_method: "get_feed_sections",
    },
    ResourceDefinition {
        name: "createNote",
        kind: "mutation",
        field: "createNote",
        client_method: "create_note",
    },
    ResourceDefinition {
        name: "createComment",
        kind: "mutation",
        field: "createComment",
        client_method: "create_comment",
    },
    ResourceDefinition {
        name: "createCommentReply",
        kind: "mutation",
        field: "createCommentReply",
        client_method: "create_comment_reply",
    },
    ResourceDefinition {
        name: "createFolder",
        kind: "mutation",
        field: "createFolder",
        client_method: "create_folder",
    },
    ResourceDefinition {
        name: "moveNoteToAnotherFolder",
        kind: "mutation",
        field: "moveNoteToAnotherFolder",
        client_method: "move_note_to_another_folder",
    },
    ResourceDefinition {
        name: "attachNoteToFolder",
        kind: "mutation",
        field: "attachNoteToFolder",
        client_method: "attach_note_to_folder",
    },
    ResourceDefinition {
        name: "updateNoteContent",
        kind: "mutation",
        field: "updateNoteContent",
        client_method: "update_note",
    },
];

#[derive(Parser)]
#[command(name = "kibel-tools")]
#[command(about = "Contract maintenance tools for kibel")]
struct Cli {
    #[command(subcommand)]
    command: TopCommand,
}

#[derive(Subcommand)]
enum TopCommand {
    CreateNoteContract {
        #[command(subcommand)]
        action: CreateNoteContractAction,
    },
    ResourceContract {
        #[command(subcommand)]
        action: ResourceContractAction,
    },
}

#[derive(Subcommand)]
enum CreateNoteContractAction {
    Check(CreateNoteContractArgs),
    Write(CreateNoteContractArgs),
}

#[derive(Subcommand)]
enum ResourceContractAction {
    Check(ResourceContractArgs),
    Write(ResourceContractArgs),
    RefreshEndpoint(EndpointRefreshArgs),
}

#[derive(Args, Clone)]
struct CreateNoteContractArgs {
    #[arg(
        long,
        default_value = "research/schema/create_note_contract.snapshot.json"
    )]
    snapshot: String,
    #[arg(
        long,
        default_value = "crates/kibel-client/src/generated_create_note_contract.rs"
    )]
    generated: String,
}

#[derive(Args, Clone)]
struct ResourceContractArgs {
    #[arg(
        long,
        default_value = "research/schema/resource_contracts.endpoint.snapshot.json"
    )]
    endpoint_snapshot: String,
    #[arg(
        long,
        default_value = "research/schema/resource_contracts.snapshot.json"
    )]
    snapshot: String,
    #[arg(
        long,
        default_value = "crates/kibel-client/src/generated_resource_contracts.rs"
    )]
    generated: String,
}

#[derive(Args, Clone)]
struct EndpointRefreshArgs {
    #[arg(long, env = "KIBELA_ORIGIN")]
    origin: String,
    #[arg(long, env = "KIBELA_ACCESS_TOKEN")]
    token: String,
    #[arg(
        long,
        default_value = "research/schema/resource_contracts.endpoint.snapshot.json"
    )]
    endpoint_snapshot: String,
    #[arg(long)]
    endpoint: Option<String>,
    #[arg(long, default_value_t = 30)]
    timeout_secs: u64,
}

#[derive(Debug, Clone)]
struct CreateNoteSnapshot {
    input: Vec<String>,
    payload: Vec<String>,
    note_projection: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSnapshot {
    schema_contract_version: u32,
    source: NormalizedSource,
    resources: Vec<NormalizedResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSource {
    mode: String,
    endpoint_snapshot: String,
    captured_at: String,
    origin: String,
    endpoint: String,
    upstream_commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedResource {
    name: String,
    kind: String,
    operation: String,
    all_variables: Vec<String>,
    required_variables: Vec<String>,
    graphql_file: String,
    client_method: String,
}

#[derive(Debug, Clone)]
struct ResourceModuleSnapshot {
    schema_contract_version: u32,
    source_upstream_commit: String,
    resources: Vec<NormalizedResource>,
}

#[derive(Debug, Clone)]
struct EndpointSnapshot {
    captured_at: String,
    origin: String,
    endpoint: String,
    resources: HashMap<String, EndpointResource>,
}

#[derive(Debug, Clone)]
struct EndpointResource {
    name: String,
    kind: String,
    field: String,
    operation: String,
    client_method: String,
    all_variables: Vec<String>,
    required_variables: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let root = repo_root();
    match cli.command {
        TopCommand::CreateNoteContract { action } => match action {
            CreateNoteContractAction::Check(args) => run_create_note_contract_check(&root, &args),
            CreateNoteContractAction::Write(args) => run_create_note_contract_write(&root, &args),
        },
        TopCommand::ResourceContract { action } => match action {
            ResourceContractAction::Check(args) => run_resource_contract_check(&root, &args),
            ResourceContractAction::Write(args) => run_resource_contract_write(&root, &args),
            ResourceContractAction::RefreshEndpoint(args) => {
                run_resource_contract_refresh_endpoint(&root, &args)
            }
        },
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate path should be crates/kibel-tools")
        .to_path_buf()
}

fn resolve_path(root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn resource_definitions() -> &'static [ResourceDefinition] {
    RESOURCE_DEFINITIONS
}

fn endpoint_from_origin(origin: &str) -> String {
    let normalized = origin.trim().trim_end_matches('/');
    if normalized.ends_with("/api/v1") {
        normalized.to_string()
    } else {
        format!("{normalized}/api/v1")
    }
}

fn now_rfc3339() -> Result<String, String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|error| format!("failed to format timestamp: {error}"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut rendered = serde_json::to_string_pretty(value)
        .map_err(|error| format!("json render failed: {error}"))?;
    rendered.push('\n');
    fs::write(path, rendered)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

fn normalize_string_list(value: &Value, context: &str) -> Result<Vec<String>, String> {
    let items = value
        .as_array()
        .ok_or_else(|| format!("{context} must be an array"))?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let normalized = value_to_string(item).trim().to_string();
        if normalized.is_empty() || seen.contains(&normalized) {
            continue;
        }
        seen.insert(normalized.clone());
        result.push(normalized);
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct GraphqlArg {
    name: String,
    required: bool,
}

fn fetch_introspection_payload(
    endpoint: &str,
    token: &str,
    timeout_secs: u64,
) -> Result<Value, String> {
    let payload = json!({
        "query": INTROSPECTION_QUERY,
        "variables": {}
    });
    let payload_raw =
        serde_json::to_string(&payload).map_err(|error| format!("json render failed: {error}"))?;

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .build();
    let request = agent
        .post(endpoint)
        .set("Content-Type", "application/json")
        .set("Authorization", &format!("Bearer {token}"));

    let (raw, status_code) = match request.send_string(&payload_raw) {
        Ok(response) => {
            let body = response
                .into_string()
                .map_err(|err| format!("failed to read response: {err}"))?;
            (body, None)
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response
                .into_string()
                .map_err(|err| format!("failed to read error response: {err}"))?;
            (body, Some(code))
        }
        Err(err) => {
            return Err(format!("request failed: {err}"));
        }
    };

    let payload = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse response: {error}"))?;
    if let Some(message) = extract_graphql_error_message(&payload) {
        if let Some(code) = status_code {
            return Err(format!("graphql error (status {code}): {message}"));
        }
        return Err(format!("graphql error: {message}"));
    }
    Ok(payload)
}

fn extract_graphql_error_message(payload: &Value) -> Option<String> {
    let errors = payload.get("errors")?.as_array()?;
    if errors.is_empty() {
        return None;
    }
    let mut messages = Vec::new();
    for error in errors {
        if let Some(message) = error.get("message").and_then(Value::as_str) {
            messages.push(message.to_string());
            continue;
        }
        messages.push(error.to_string());
    }
    Some(messages.join(" | "))
}

fn parse_graphql_fields(
    payload: &Value,
    kind: &str,
) -> Result<HashMap<String, Vec<GraphqlArg>>, String> {
    let pointer = match kind {
        "query" => "/data/__schema/queryType/fields",
        "mutation" => "/data/__schema/mutationType/fields",
        _ => return Err(format!("unsupported graphql kind: {kind}")),
    };

    let fields = payload
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("introspection missing {kind} fields"))?;

    let mut result = HashMap::new();
    for (index, field) in fields.iter().enumerate() {
        let context = format!("{kind} fields[{index}]");
        let object = field
            .as_object()
            .ok_or_else(|| format!("{context} must be an object"))?;
        let name = get_trimmed_string(object, "name", &context)?;
        let args = object
            .get("args")
            .and_then(Value::as_array)
            .map(|value| value.as_slice())
            .unwrap_or(&[]);
        let mut parsed_args = Vec::new();
        for (arg_index, arg) in args.iter().enumerate() {
            let arg_context = format!("{context}.args[{arg_index}]");
            let arg_object = arg
                .as_object()
                .ok_or_else(|| format!("{arg_context} must be an object"))?;
            let arg_name = get_trimmed_string(arg_object, "name", &arg_context)?;
            let required = arg_is_required(arg_object, &arg_context)?;
            parsed_args.push(GraphqlArg {
                name: arg_name,
                required,
            });
        }
        result.insert(name, parsed_args);
    }

    Ok(result)
}

fn arg_is_required(
    arg_object: &serde_json::Map<String, Value>,
    context: &str,
) -> Result<bool, String> {
    let type_value = arg_object
        .get("type")
        .ok_or_else(|| format!("{context} missing type"))?;
    let kind = type_value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} type.kind missing"))?;
    let default_value = arg_object.get("defaultValue");
    Ok(kind == "NON_NULL" && default_value.is_none_or(|value| value.is_null()))
}

fn get_trimmed_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format!("{context} missing `{key}`"))?;
    Ok(value_to_string(value).trim().to_string())
}

fn parse_schema_contract_version(
    object: &serde_json::Map<String, Value>,
    context: &str,
) -> Result<u32, String> {
    let raw = object
        .get("schema_contract_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} must contain numeric `schema_contract_version`"))?;
    u32::try_from(raw).map_err(|_| format!("{context} schema_contract_version out of range"))
}

fn rust_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn to_pascal_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn render_array_const(name: &str, values: &[String]) -> String {
    let inline_values = values
        .iter()
        .map(|value| rust_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    let inline_line = format!("pub const {name}: &[&str] = &[{inline_values}];");
    if inline_line.len() <= 98 {
        return inline_line;
    }

    let mut lines = vec![format!("pub const {name}: &[&str] = &[")];
    for value in values {
        lines.push(format!("    {},", rust_string(value)));
    }
    lines.push("];".to_string());
    lines.join("\n")
}

fn load_create_note_snapshot(path: &Path) -> Result<CreateNoteSnapshot, String> {
    let payload = read_json(path)?;
    let object = payload
        .as_object()
        .ok_or_else(|| "create note snapshot must be an object".to_string())?;

    let input_fields = normalize_string_list(
        object.get("create_note_input_fields").ok_or_else(|| {
            "`create_note_input_fields` is required and must be an array".to_string()
        })?,
        "`create_note_input_fields`",
    )?;
    let payload_fields = normalize_string_list(
        object.get("create_note_payload_fields").ok_or_else(|| {
            "`create_note_payload_fields` is required and must be an array".to_string()
        })?,
        "`create_note_payload_fields`",
    )?;
    let note_projection_fields = normalize_string_list(
        object
            .get("create_note_note_projection_fields")
            .ok_or_else(|| {
                "`create_note_note_projection_fields` is required and must be an array".to_string()
            })?,
        "`create_note_note_projection_fields`",
    )?;
    let required_input_fields = normalize_string_list(
        object.get("required_input_fields").ok_or_else(|| {
            "`required_input_fields` is required and must be an array".to_string()
        })?,
        "`required_input_fields`",
    )?;
    let required_payload_fields = normalize_string_list(
        object.get("required_payload_fields").ok_or_else(|| {
            "`required_payload_fields` is required and must be an array".to_string()
        })?,
        "`required_payload_fields`",
    )?;

    let input_set = input_fields.iter().collect::<HashSet<_>>();
    let payload_set = payload_fields.iter().collect::<HashSet<_>>();

    let missing_input = required_input_fields
        .iter()
        .filter(|field| !input_set.contains(*field))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_input.is_empty() {
        return Err(format!("missing required input fields: {missing_input:?}"));
    }

    let missing_payload = required_payload_fields
        .iter()
        .filter(|field| !payload_set.contains(*field))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_payload.is_empty() {
        return Err(format!(
            "missing required payload fields: {missing_payload:?}"
        ));
    }

    if !note_projection_fields.iter().any(|field| field == "id") {
        return Err("create_note_note_projection_fields must include `id`".to_string());
    }

    Ok(CreateNoteSnapshot {
        input: input_fields,
        payload: payload_fields,
        note_projection: note_projection_fields,
    })
}

fn render_create_note_module(snapshot: &CreateNoteSnapshot) -> String {
    let parts = vec![
        "// This file is generated by crates/kibel-tools.".to_string(),
        "// Do not edit by hand.".to_string(),
        String::new(),
        render_array_const("CREATE_NOTE_INPUT_FIELDS", &snapshot.input),
        String::new(),
        render_array_const("CREATE_NOTE_PAYLOAD_FIELDS", &snapshot.payload),
        String::new(),
        render_array_const(
            "CREATE_NOTE_NOTE_PROJECTION_FIELDS",
            &snapshot.note_projection,
        ),
        String::new(),
    ];
    let mut rendered = parts.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

fn run_create_note_contract_check(
    root: &Path,
    args: &CreateNoteContractArgs,
) -> Result<(), String> {
    let snapshot_path = resolve_path(root, &args.snapshot);
    let generated_path = resolve_path(root, &args.generated);
    let snapshot = load_create_note_snapshot(&snapshot_path)?;
    let expected = render_create_note_module(&snapshot);
    let actual = fs::read_to_string(&generated_path)
        .map_err(|error| format!("failed to read {}: {error}", generated_path.display()))?;
    if actual != expected {
        return Err("generated file is stale. run:\n\
             cargo run -p kibel-tools -- create-note-contract write"
            .to_string());
    }
    println!("schema contract check: ok");
    Ok(())
}

fn run_create_note_contract_write(
    root: &Path,
    args: &CreateNoteContractArgs,
) -> Result<(), String> {
    let snapshot_path = resolve_path(root, &args.snapshot);
    let generated_path = resolve_path(root, &args.generated);
    let snapshot = load_create_note_snapshot(&snapshot_path)?;
    let rendered = render_create_note_module(&snapshot);
    if let Some(parent) = generated_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&generated_path, rendered)
        .map_err(|error| format!("failed to write {}: {error}", generated_path.display()))?;
    println!("schema contract check: ok (written)");
    Ok(())
}

fn build_endpoint_snapshot_from_introspection(
    definitions: &[ResourceDefinition],
    payload: &Value,
    origin: &str,
    endpoint: &str,
    captured_at: &str,
) -> Result<Value, String> {
    let query_fields = parse_graphql_fields(payload, "query")?;
    let mutation_fields = parse_graphql_fields(payload, "mutation")?;

    let mut resources = Vec::new();
    for definition in definitions {
        let fields = match definition.kind {
            "query" => &query_fields,
            "mutation" => &mutation_fields,
            other => return Err(format!("unsupported kind: {other}")),
        };
        let args = fields
            .get(definition.field)
            .ok_or_else(|| format!("missing graphql field: {}", definition.field))?;

        let mut all_variables = Vec::new();
        let mut required_variables = Vec::new();
        let mut seen = HashSet::new();
        for arg in args {
            if !seen.insert(arg.name.clone()) {
                continue;
            }
            all_variables.push(arg.name.clone());
            if arg.required {
                required_variables.push(arg.name.clone());
            }
        }

        resources.push(json!({
            "name": definition.name,
            "kind": definition.kind,
            "field": definition.field,
            "operation": to_pascal_case(definition.name),
            "client_method": definition.client_method,
            "all_variables": all_variables,
            "required_variables": required_variables,
        }));
    }

    Ok(json!({
        "schema_contract_version": 1,
        "captured_at": captured_at,
        "origin": origin,
        "endpoint": endpoint,
        "resource_count": definitions.len(),
        "resources": resources,
    }))
}

#[allow(clippy::too_many_lines)]
fn load_endpoint_snapshot(path: &Path) -> Result<EndpointSnapshot, String> {
    let payload = read_json(path)?;
    let object = payload
        .as_object()
        .ok_or_else(|| "endpoint snapshot must be an object".to_string())?;
    let resources_value = object
        .get("resources")
        .ok_or_else(|| "endpoint snapshot must contain array `resources`".to_string())?;
    let resources_array = resources_value
        .as_array()
        .ok_or_else(|| "endpoint snapshot must contain array `resources`".to_string())?;

    let mut resources = HashMap::new();
    for (index, item) in resources_array.iter().enumerate() {
        let context = format!("resources[{index}]");
        let object = item
            .as_object()
            .ok_or_else(|| format!("{context} must be an object"))?;

        let name = get_trimmed_string(object, "name", &context)?;
        if name.is_empty() {
            return Err(format!("{context} has empty name"));
        }
        if resources.contains_key(&name) {
            return Err(format!(
                "duplicate resource name in endpoint snapshot: {name}"
            ));
        }

        let kind = get_trimmed_string(object, "kind", &context)?;
        if kind != "query" && kind != "mutation" {
            return Err(format!("resource `{name}` has invalid kind: {kind}"));
        }
        let field = get_trimmed_string(object, "field", &context)?;
        let operation = get_trimmed_string(object, "operation", &context)?;
        let client_method = get_trimmed_string(object, "client_method", &context)?;
        if field.is_empty() || operation.is_empty() || client_method.is_empty() {
            return Err(format!(
                "resource `{name}` must have non-empty field/operation/client_method"
            ));
        }

        let all_variables = normalize_string_list(
            object
                .get("all_variables")
                .ok_or_else(|| format!("{context} missing `all_variables`"))?,
            &format!("{context}.all_variables"),
        )?;
        let required_variables = normalize_string_list(
            object
                .get("required_variables")
                .ok_or_else(|| format!("{context} missing `required_variables`"))?,
            &format!("{context}.required_variables"),
        )?;

        let all_set = all_variables.iter().collect::<HashSet<_>>();
        let missing_required = required_variables
            .iter()
            .filter(|value| !all_set.contains(*value))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_required.is_empty() {
            return Err(format!(
                "resource `{name}` has required vars not in all_variables: {missing_required:?}"
            ));
        }

        resources.insert(
            name.clone(),
            EndpointResource {
                name,
                kind,
                field,
                operation,
                client_method,
                all_variables,
                required_variables,
            },
        );
    }

    let missing = resource_definitions()
        .iter()
        .filter(|definition| !resources.contains_key(definition.name))
        .map(|definition| definition.name.to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!("endpoint snapshot missing resources: {missing:?}"));
    }

    let expected = resource_definitions()
        .iter()
        .map(|definition| definition.name)
        .collect::<BTreeSet<_>>();
    let unexpected = resources
        .keys()
        .filter(|name| !expected.contains(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        return Err("endpoint snapshot contains unknown resources. \
             update RESOURCE_ORDER/CLI/client/tests first: "
            .to_string()
            + &format!("{unexpected:?}"));
    }

    Ok(EndpointSnapshot {
        captured_at: object
            .get("captured_at")
            .map(value_to_string)
            .unwrap_or_default(),
        origin: object
            .get("origin")
            .map(value_to_string)
            .unwrap_or_default(),
        endpoint: object
            .get("endpoint")
            .map(value_to_string)
            .unwrap_or_default(),
        resources,
    })
}

fn build_resource_snapshot_value(
    root: &Path,
    endpoint_snapshot_path: &Path,
    endpoint_payload: &EndpointSnapshot,
) -> Result<Value, String> {
    let endpoint_snapshot_rel = endpoint_snapshot_path
        .strip_prefix(root)
        .map_err(|_| {
            format!(
                "{} is not in repository root {}",
                endpoint_snapshot_path.display(),
                root.display()
            )
        })?
        .to_string_lossy()
        .to_string();

    let mut rendered_resources = Vec::new();
    for definition in resource_definitions() {
        let item = endpoint_payload
            .resources
            .get(definition.name)
            .ok_or_else(|| format!("endpoint snapshot missing resource `{}`", definition.name))?;
        rendered_resources.push(json!({
            "name": item.name,
            "kind": item.kind,
            "operation": item.operation,
            "all_variables": item.all_variables,
            "required_variables": item.required_variables,
            "graphql_file": format!("endpoint:{}.{}", item.kind, item.field),
            "client_method": item.client_method,
        }));
    }

    Ok(json!({
        "schema_contract_version": 1,
        "captured_at": endpoint_payload.captured_at,
        "source": {
            "mode": "endpoint_introspection_snapshot",
            "endpoint_snapshot": endpoint_snapshot_rel,
            "captured_at": endpoint_payload.captured_at,
            "origin": endpoint_payload.origin,
            "endpoint": endpoint_payload.endpoint,
            "upstream_commit": "",
        },
        "resources": rendered_resources,
    }))
}

fn normalize_resource_snapshot(payload: &Value) -> Result<NormalizedSnapshot, String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "snapshot must be an object".to_string())?;
    let version = parse_schema_contract_version(object, "snapshot")?;

    let source_object = object
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| "snapshot must contain object `source`".to_string())?;
    let source = NormalizedSource {
        mode: source_object
            .get("mode")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
        endpoint_snapshot: source_object
            .get("endpoint_snapshot")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
        captured_at: source_object
            .get("captured_at")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
        origin: source_object
            .get("origin")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
        endpoint: source_object
            .get("endpoint")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
        upstream_commit: source_object
            .get("upstream_commit")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
    };

    let resources_array = object
        .get("resources")
        .and_then(Value::as_array)
        .ok_or_else(|| "snapshot must contain array `resources`".to_string())?;

    let mut resources = Vec::new();
    let mut seen_names = HashSet::new();
    for (index, item) in resources_array.iter().enumerate() {
        let context = format!("resource[{index}]");
        let resource = parse_normalized_resource(item, &context)?;
        if seen_names.contains(&resource.name) {
            return Err(format!("duplicate resource name: {}", resource.name));
        }
        seen_names.insert(resource.name.clone());
        resources.push(resource);
    }
    if resources.is_empty() {
        return Err("snapshot resources cannot be empty".to_string());
    }
    resources.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(NormalizedSnapshot {
        schema_contract_version: version,
        source,
        resources,
    })
}

fn parse_normalized_resource(item: &Value, context: &str) -> Result<NormalizedResource, String> {
    let object = item
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    for key in [
        "name",
        "kind",
        "operation",
        "all_variables",
        "required_variables",
        "graphql_file",
        "client_method",
    ] {
        if !object.contains_key(key) {
            return Err(format!("{context} is missing `{key}`"));
        }
    }

    let name = get_trimmed_string(object, "name", context)?;
    if name.is_empty() {
        return Err(format!("{context} name is empty"));
    }
    let kind = get_trimmed_string(object, "kind", context)?;
    if kind != "query" && kind != "mutation" {
        return Err(format!("resource `{name}` has invalid kind: {kind}"));
    }
    let operation = get_trimmed_string(object, "operation", context)?;
    let graphql_file = get_trimmed_string(object, "graphql_file", context)?;
    let client_method = get_trimmed_string(object, "client_method", context)?;
    let all_variables = normalize_string_list(
        object
            .get("all_variables")
            .ok_or_else(|| format!("{context} missing `all_variables`"))?,
        &format!("{context}.all_variables"),
    )?;
    let required_variables = normalize_string_list(
        object
            .get("required_variables")
            .ok_or_else(|| format!("{context} missing `required_variables`"))?,
        &format!("{context}.required_variables"),
    )?;

    let all_set = all_variables.iter().collect::<HashSet<_>>();
    let missing_required = required_variables
        .iter()
        .filter(|value| !all_set.contains(*value))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        return Err(format!(
            "resource `{name}` has required vars not in all_variables: {missing_required:?}"
        ));
    }

    Ok(NormalizedResource {
        name,
        kind,
        operation,
        all_variables,
        required_variables,
        graphql_file,
        client_method,
    })
}

fn load_resource_module_snapshot(path: &Path) -> Result<ResourceModuleSnapshot, String> {
    let payload = read_json(path)?;
    let object = payload
        .as_object()
        .ok_or_else(|| "snapshot must be an object".to_string())?;
    let version = parse_schema_contract_version(object, "snapshot")?;
    let source_object = object
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| "snapshot must contain object `source`".to_string())?;
    let resources_array = object
        .get("resources")
        .and_then(Value::as_array)
        .ok_or_else(|| "snapshot must contain array `resources`".to_string())?;
    if resources_array.is_empty() {
        return Err("snapshot resources cannot be empty".to_string());
    }

    let mut resources = Vec::new();
    let mut seen_names = HashSet::new();
    for (index, item) in resources_array.iter().enumerate() {
        let context = format!("resource[{index}]");
        let resource = parse_normalized_resource(item, &context)?;
        if seen_names.contains(&resource.name) {
            return Err(format!("duplicate resource name: {}", resource.name));
        }
        seen_names.insert(resource.name.clone());
        resources.push(resource);
    }

    Ok(ResourceModuleSnapshot {
        schema_contract_version: version,
        source_upstream_commit: source_object
            .get("upstream_commit")
            .map(value_to_string)
            .unwrap_or_default()
            .trim()
            .to_string(),
        resources,
    })
}

fn render_string_array(values: &[String], indent: &str) -> String {
    if values.is_empty() {
        return "&[]".to_string();
    }
    if values.len() > 4 {
        let mut lines = vec!["&[".to_string()];
        for value in values {
            lines.push(format!("{indent}    {},", rust_string(value)));
        }
        lines.push(format!("{indent}]"));
        return lines.join("\n");
    }

    let inline = values
        .iter()
        .map(|value| rust_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    let inline_rendered = format!("&[{inline}]");
    if inline_rendered.len() <= 72 {
        return inline_rendered;
    }

    let mut lines = vec!["&[".to_string()];
    for value in values {
        lines.push(format!("{indent}    {},", rust_string(value)));
    }
    lines.push(format!("{indent}]"));
    lines.join("\n")
}

fn render_resource_contract(resource: &NormalizedResource) -> String {
    let all_variables = render_string_array(&resource.all_variables, "        ");
    let required_variables = render_string_array(&resource.required_variables, "        ");
    [
        "    ResourceContract {".to_string(),
        format!("        name: {},", rust_string(&resource.name)),
        format!("        kind: {},", rust_string(&resource.kind)),
        format!("        operation: {},", rust_string(&resource.operation)),
        format!("        all_variables: {all_variables},"),
        format!("        required_variables: {required_variables},"),
        format!(
            "        graphql_file: {},",
            rust_string(&resource.graphql_file)
        ),
        format!(
            "        client_method: {},",
            rust_string(&resource.client_method)
        ),
        "    },".to_string(),
    ]
    .join("\n")
}

fn render_resource_module(snapshot: &ResourceModuleSnapshot) -> String {
    let mut lines = vec![
        "// This file is generated by crates/kibel-tools.".to_string(),
        "// Do not edit by hand.".to_string(),
        String::new(),
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]".to_string(),
        "pub struct ResourceContract {".to_string(),
        "    pub name: &'static str,".to_string(),
        "    pub kind: &'static str,".to_string(),
        "    pub operation: &'static str,".to_string(),
        "    pub all_variables: &'static [&'static str],".to_string(),
        "    pub required_variables: &'static [&'static str],".to_string(),
        "    pub graphql_file: &'static str,".to_string(),
        "    pub client_method: &'static str,".to_string(),
        "}".to_string(),
        String::new(),
        format!(
            "pub const RESOURCE_CONTRACT_VERSION: u32 = {};",
            snapshot.schema_contract_version
        ),
        format!(
            "pub const RESOURCE_CONTRACT_UPSTREAM_COMMIT: &str = {};",
            rust_string(&snapshot.source_upstream_commit)
        ),
        String::new(),
        "pub const RESOURCE_CONTRACTS: &[ResourceContract] = &[".to_string(),
    ];
    for resource in &snapshot.resources {
        lines.push(render_resource_contract(resource));
    }
    lines.push("];".to_string());
    lines.push(String::new());

    let mut rendered = lines.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

fn rustfmt_source(source: &str) -> Result<String, String> {
    let temp_dir =
        tempfile::tempdir().map_err(|error| format!("failed to create temp dir: {error}"))?;
    let path = temp_dir.path().join("generated.rs");
    fs::write(&path, source).map_err(|error| format!("failed to write temp rust file: {error}"))?;
    let status = Command::new("rustfmt")
        .arg(&path)
        .status()
        .map_err(|error| format!("failed to run rustfmt: {error}"))?;
    if !status.success() {
        return Err("rustfmt failed for generated module".to_string());
    }
    fs::read_to_string(&path).map_err(|error| format!("failed to read rustfmt output: {error}"))
}

fn run_resource_contract_check(root: &Path, args: &ResourceContractArgs) -> Result<(), String> {
    let endpoint_snapshot_path = resolve_path(root, &args.endpoint_snapshot);
    let snapshot_path = resolve_path(root, &args.snapshot);
    let generated_path = resolve_path(root, &args.generated);

    let endpoint_snapshot = load_endpoint_snapshot(&endpoint_snapshot_path)?;
    let expected_snapshot_value =
        build_resource_snapshot_value(root, &endpoint_snapshot_path, &endpoint_snapshot)?;
    let expected_snapshot = normalize_resource_snapshot(&expected_snapshot_value)?;
    let actual_snapshot_value = read_json(&snapshot_path)?;
    let actual_snapshot = normalize_resource_snapshot(&actual_snapshot_value)?;

    if actual_snapshot != expected_snapshot {
        return Err("resource snapshot is stale. run:\n\
             cargo run -p kibel-tools -- resource-contract write"
            .to_string());
    }

    let module_snapshot = load_resource_module_snapshot(&snapshot_path)?;
    let expected_generated = rustfmt_source(&render_resource_module(&module_snapshot))?;
    let actual_generated = fs::read_to_string(&generated_path)
        .map_err(|error| format!("failed to read {}: {error}", generated_path.display()))?;
    if actual_generated != expected_generated {
        return Err("generated resource contract module is stale. run:\n\
             cargo run -p kibel-tools -- resource-contract write"
            .to_string());
    }

    println!("resource contract check: ok");
    Ok(())
}

fn run_resource_contract_write(root: &Path, args: &ResourceContractArgs) -> Result<(), String> {
    let endpoint_snapshot_path = resolve_path(root, &args.endpoint_snapshot);
    let snapshot_path = resolve_path(root, &args.snapshot);
    let generated_path = resolve_path(root, &args.generated);

    let endpoint_snapshot = load_endpoint_snapshot(&endpoint_snapshot_path)?;
    let snapshot_value =
        build_resource_snapshot_value(root, &endpoint_snapshot_path, &endpoint_snapshot)?;
    write_json_pretty(&snapshot_path, &snapshot_value)?;

    let module_snapshot = load_resource_module_snapshot(&snapshot_path)?;
    let rendered = rustfmt_source(&render_resource_module(&module_snapshot))?;
    if let Some(parent) = generated_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&generated_path, rendered)
        .map_err(|error| format!("failed to write {}: {error}", generated_path.display()))?;

    println!("resource contract check: ok (written)");
    Ok(())
}

fn run_resource_contract_refresh_endpoint(
    root: &Path,
    args: &EndpointRefreshArgs,
) -> Result<(), String> {
    let origin = args.origin.trim();
    if origin.is_empty() {
        return Err("origin is required (use --origin or KIBELA_ORIGIN)".to_string());
    }
    let token = args.token.trim();
    if token.is_empty() {
        return Err("token is required (use --token or KIBELA_ACCESS_TOKEN)".to_string());
    }

    let endpoint = args
        .endpoint
        .clone()
        .unwrap_or_else(|| endpoint_from_origin(origin));
    let payload = fetch_introspection_payload(&endpoint, token, args.timeout_secs)?;
    let captured_at = now_rfc3339()?;
    let snapshot_value = build_endpoint_snapshot_from_introspection(
        resource_definitions(),
        &payload,
        origin,
        &endpoint,
        &captured_at,
    )?;

    let endpoint_snapshot_path = resolve_path(root, &args.endpoint_snapshot);
    write_json_pretty(&endpoint_snapshot_path, &snapshot_value)?;
    println!("endpoint snapshot refresh: ok (written)");
    Ok(())
}

#[cfg(test)]
mod tests {
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
        })
    }

    #[test]
    fn normalize_string_list_trims_and_deduplicates() {
        let normalized =
            normalize_string_list(&json!([" title ", "", "title", 5, "5", "  "]), "test")
                .expect("normalize_string_list should succeed");

        assert_eq!(normalized, vec!["title".to_string(), "5".to_string()]);
    }

    #[test]
    fn normalize_resource_snapshot_sorts_by_resource_name() {
        let payload = json!({
            "schema_contract_version": 1,
            "source": {
                "mode": "endpoint_introspection_snapshot",
                "endpoint_snapshot": "research/schema/resource_contracts.endpoint.snapshot.json",
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
                "endpoint_snapshot": "research/schema/resource_contracts.endpoint.snapshot.json",
                "captured_at": "2026-02-23T00:00:00Z",
                "origin": "https://example.kibe.la",
                "endpoint": "https://example.kibe.la/api/v1",
                "upstream_commit": "",
            },
            "resources": [
                resource("searchNote", &["query"], &["query", "groupId"]),
            ]
        });

        let error =
            normalize_resource_snapshot(&payload).expect_err("invalid required vars must fail");
        assert!(
            error.contains("required vars not in all_variables"),
            "unexpected error: {error}"
        );
    }

    #[test]
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
                                ]
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
                                ]
                            }
                        ]
                    }
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
    }
}
