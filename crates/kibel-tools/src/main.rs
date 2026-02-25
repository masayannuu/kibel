use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;
use thiserror::Error;

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
            ...TypeRef
          }
        }
        type {
          ...TypeRef
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
            ...TypeRef
          }
        }
        type {
          ...TypeRef
        }
      }
    }
    types {
      kind
      name
      fields {
        name
        args {
          name
          defaultValue
          type {
            ...TypeRef
          }
        }
        type {
          ...TypeRef
        }
      }
      inputFields {
        name
      }
      possibleTypes {
        name
      }
      enumValues {
        name
      }
    }
  }
}

fragment TypeRef on __Type {
  kind
  name
  ofType {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
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
const GRAPHQL_ACCEPT_HEADER: &str = "application/graphql-response+json, application/json;q=0.9";
const REQUIRED_CREATE_NOTE_INPUT_FIELDS: &[&str] = &["title", "content", "groupIds", "coediting"];
const REQUIRED_CREATE_NOTE_PAYLOAD_FIELDS: &[&str] = &["note"];

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
    RefreshFromEndpoint(CreateNoteRefreshFromEndpointArgs),
}

#[derive(Subcommand)]
enum ResourceContractAction {
    Check(ResourceContractArgs),
    Write(ResourceContractArgs),
    RefreshEndpoint(EndpointRefreshArgs),
    Diff(ResourceContractDiffArgs),
}

#[derive(Args, Clone)]
struct CreateNoteContractArgs {
    #[arg(
        long,
        default_value = "schema/contracts/create_note_contract.snapshot.json"
    )]
    snapshot: String,
    #[arg(
        long,
        default_value = "crates/kibel-client/src/generated_create_note_contract.rs"
    )]
    generated: String,
}

#[derive(Args, Clone)]
struct CreateNoteRefreshFromEndpointArgs {
    #[arg(
        long,
        default_value = "schema/introspection/resource_contracts.endpoint.snapshot.json"
    )]
    endpoint_snapshot: String,
    #[arg(
        long,
        default_value = "schema/contracts/create_note_contract.snapshot.json"
    )]
    snapshot: String,
}

#[derive(Args, Clone)]
struct ResourceContractArgs {
    #[arg(
        long,
        default_value = "schema/introspection/resource_contracts.endpoint.snapshot.json"
    )]
    endpoint_snapshot: String,
    #[arg(
        long,
        default_value = "schema/contracts/resource_contracts.snapshot.json"
    )]
    snapshot: String,
    #[arg(
        long,
        default_value = "crates/kibel-client/src/generated_resource_contracts.rs"
    )]
    generated: String,
}

#[derive(Args, Clone)]
struct ResourceContractDiffArgs {
    #[arg(long)]
    base: String,
    #[arg(long)]
    target: String,
    #[arg(long, value_enum, default_value_t = DiffOutputFormat::Text)]
    format: DiffOutputFormat,
    #[arg(long, default_value_t = false)]
    fail_on_breaking: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum DiffOutputFormat {
    Text,
    Json,
}

#[derive(Args, Clone)]
struct EndpointRefreshArgs {
    #[arg(long, env = "KIBELA_ORIGIN")]
    origin: String,
    #[arg(long, env = "KIBELA_ACCESS_TOKEN")]
    token: String,
    #[arg(
        long,
        default_value = "schema/introspection/resource_contracts.endpoint.snapshot.json"
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
    document: String,
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
    document: String,
}

type ToolResult<T> = Result<T, ToolError>;

#[derive(Debug, Error)]
enum ToolError {
    #[error("{0}")]
    Message(String),
}

impl ToolError {
    fn message(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }
}

impl From<String> for ToolError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

impl From<&str> for ToolError {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
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

fn run(cli: Cli) -> ToolResult<()> {
    let root = repo_root();
    match cli.command {
        TopCommand::CreateNoteContract { action } => match action {
            CreateNoteContractAction::Check(args) => run_create_note_contract_check(&root, &args),
            CreateNoteContractAction::Write(args) => run_create_note_contract_write(&root, &args),
            CreateNoteContractAction::RefreshFromEndpoint(args) => {
                run_create_note_contract_refresh_from_endpoint(&root, &args)
            }
        },
        TopCommand::ResourceContract { action } => match action {
            ResourceContractAction::Check(args) => run_resource_contract_check(&root, &args),
            ResourceContractAction::Write(args) => run_resource_contract_write(&root, &args),
            ResourceContractAction::RefreshEndpoint(args) => {
                run_resource_contract_refresh_endpoint(&root, &args)
            }
            ResourceContractAction::Diff(args) => run_resource_contract_diff(&root, &args),
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

fn now_rfc3339() -> ToolResult<String> {
    Ok(time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|error| format!("failed to format timestamp: {error}"))?)
}

fn read_json(path: &Path) -> ToolResult<Value> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?)
}

fn write_json_pretty(path: &Path, value: &Value) -> ToolResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut rendered = serde_json::to_string_pretty(value)
        .map_err(|error| format!("json render failed: {error}"))?;
    rendered.push('\n');
    Ok(fs::write(path, rendered)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

fn normalize_string_list(value: &Value, context: &str) -> ToolResult<Vec<String>> {
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

fn collect_graphql_name_list(value: &Value, context: &str) -> ToolResult<Vec<String>> {
    let items = value
        .as_array()
        .ok_or_else(|| format!("{context} must be an array"))?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            return Err((format!("{context} should contain objects with string `name`")).into());
        };
        let normalized = name.trim();
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.to_string()) {
            result.push(normalized.to_string());
        }
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct GraphqlArg {
    name: String,
    required: bool,
    type_ref: GraphqlTypeRef,
    rendered_type: String,
}

#[derive(Debug, Clone)]
struct GraphqlFieldSpec {
    args: Vec<GraphqlArg>,
    return_type: GraphqlTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphqlTypeRef {
    kind: String,
    name: Option<String>,
    of_type: Option<Box<GraphqlTypeRef>>,
}

#[derive(Debug, Clone)]
struct GraphqlFieldDefinition {
    name: String,
    args: Vec<GraphqlArg>,
    type_ref: GraphqlTypeRef,
}

#[derive(Debug, Clone)]
struct GraphqlTypeDefinition {
    kind: String,
    fields: Vec<GraphqlFieldDefinition>,
    possible_types: Vec<String>,
    enum_values: Vec<String>,
}

fn fetch_introspection_payload(
    endpoint: &str,
    token: &str,
    timeout_secs: u64,
) -> ToolResult<Value> {
    fetch_graphql_payload(endpoint, token, INTROSPECTION_QUERY, timeout_secs)
}

fn fetch_graphql_payload(
    endpoint: &str,
    token: &str,
    query: &str,
    timeout_secs: u64,
) -> ToolResult<Value> {
    let payload = json!({
        "query": query,
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
        .set("Accept", GRAPHQL_ACCEPT_HEADER)
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
            return Err((format!("request failed: {err}")).into());
        }
    };

    let payload = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse response: {error}"))?;
    if let Some(message) = extract_graphql_error_message(&payload) {
        if let Some(code) = status_code {
            return Err((format!("graphql error (status {code}): {message}")).into());
        }
        return Err((format!("graphql error: {message}")).into());
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
) -> ToolResult<HashMap<String, GraphqlFieldSpec>> {
    let pointer = match kind {
        "query" => "/data/__schema/queryType/fields",
        "mutation" => "/data/__schema/mutationType/fields",
        _ => return Err((format!("unsupported graphql kind: {kind}")).into()),
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
        let return_type = parse_graphql_type_ref(
            object
                .get("type")
                .ok_or_else(|| format!("{context} missing type"))?,
            &format!("{context}.type"),
        )?;
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
            let type_ref = parse_graphql_type_ref(
                arg_object
                    .get("type")
                    .ok_or_else(|| format!("{arg_context} missing type"))?,
                &format!("{arg_context}.type"),
            )?;
            let rendered_type = render_graphql_type_ref(&type_ref);
            parsed_args.push(GraphqlArg {
                name: arg_name,
                required,
                type_ref,
                rendered_type,
            });
        }
        result.insert(
            name,
            GraphqlFieldSpec {
                args: parsed_args,
                return_type,
            },
        );
    }

    Ok(result)
}

fn arg_is_required(arg_object: &serde_json::Map<String, Value>, context: &str) -> ToolResult<bool> {
    let type_value = arg_object
        .get("type")
        .ok_or_else(|| format!("{context} missing type"))?;
    let kind = type_value.get("kind").and_then(Value::as_str).unwrap_or("");
    let default_value = arg_object.get("defaultValue");
    Ok(kind == "NON_NULL" && default_value.is_none_or(|value| value.is_null()))
}

fn parse_graphql_type_ref(value: &Value, context: &str) -> ToolResult<GraphqlTypeRef> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} missing kind"))?
        .trim()
        .to_string();
    if kind.is_empty() {
        return Err((format!("{context} kind is empty")).into());
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let of_type = object
        .get("ofType")
        .filter(|child| !child.is_null())
        .map(|child| parse_graphql_type_ref(child, &format!("{context}.ofType")))
        .transpose()?
        .map(Box::new);
    Ok(GraphqlTypeRef {
        kind,
        name,
        of_type,
    })
}

fn render_graphql_type_ref(type_ref: &GraphqlTypeRef) -> String {
    match type_ref.kind.as_str() {
        "NON_NULL" => type_ref
            .of_type
            .as_deref()
            .map(render_graphql_type_ref)
            .map(|inner| format!("{inner}!"))
            .unwrap_or_else(|| "JSON!".to_string()),
        "LIST" => type_ref
            .of_type
            .as_deref()
            .map(render_graphql_type_ref)
            .map(|inner| format!("[{inner}]"))
            .unwrap_or_else(|| "[JSON]".to_string()),
        _ => type_ref.name.clone().unwrap_or_else(|| "JSON".to_string()),
    }
}

fn parse_schema_types(payload: &Value) -> ToolResult<HashMap<String, GraphqlTypeDefinition>> {
    let Some(types) = payload.pointer("/data/__schema/types") else {
        return Ok(HashMap::new());
    };
    let items = types
        .as_array()
        .ok_or_else(|| "/data/__schema/types must be an array".to_string())?;
    let mut result = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if let Some((name, definition)) = parse_schema_type_entry(item, &format!("types[{index}]"))?
        {
            result.insert(name, definition);
        }
    }
    Ok(result)
}

fn parse_schema_type_entry(
    item: &Value,
    context: &str,
) -> ToolResult<Option<(String, GraphqlTypeDefinition)>> {
    let object = item
        .as_object()
        .ok_or_else(|| format!("{context} must be object"))?;
    let Some(name) = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("UNKNOWN")
        .to_string();
    let fields = parse_schema_field_definitions(object, context)?;
    let possible_types = collect_named_members(object, "possibleTypes");
    let enum_values = collect_named_members(object, "enumValues");
    Ok(Some((
        name.to_string(),
        GraphqlTypeDefinition {
            kind,
            fields,
            possible_types,
            enum_values,
        },
    )))
}

fn parse_schema_field_definitions(
    object: &serde_json::Map<String, Value>,
    context: &str,
) -> ToolResult<Vec<GraphqlFieldDefinition>> {
    let Some(field_items) = object.get("fields").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut fields = Vec::new();
    for (field_index, field_item) in field_items.iter().enumerate() {
        fields.push(parse_schema_field_definition(
            field_item,
            &format!("{context}.fields[{field_index}]"),
        )?);
    }
    Ok(fields)
}

fn parse_schema_field_definition(
    field_item: &Value,
    context: &str,
) -> ToolResult<GraphqlFieldDefinition> {
    let field_object = field_item
        .as_object()
        .ok_or_else(|| format!("{context} must be object"))?;
    let field_name = get_trimmed_string(field_object, "name", context)?;
    let field_type = parse_graphql_type_ref(
        field_object
            .get("type")
            .ok_or_else(|| format!("{context} missing type"))?,
        &format!("{context}.type"),
    )?;
    let field_args = parse_schema_field_args(field_object, context)?;
    Ok(GraphqlFieldDefinition {
        name: field_name,
        args: field_args,
        type_ref: field_type,
    })
}

fn parse_schema_field_args(
    field_object: &serde_json::Map<String, Value>,
    context: &str,
) -> ToolResult<Vec<GraphqlArg>> {
    let args = field_object
        .get("args")
        .and_then(Value::as_array)
        .map(|value| value.as_slice())
        .unwrap_or(&[]);
    let mut parsed_args = Vec::new();
    for (arg_index, arg_item) in args.iter().enumerate() {
        let arg_context = format!("{context}.args[{arg_index}]");
        let arg_object = arg_item
            .as_object()
            .ok_or_else(|| format!("{arg_context} must be object"))?;
        let arg_name = get_trimmed_string(arg_object, "name", &arg_context)?;
        let required = arg_is_required(arg_object, &arg_context)?;
        let arg_type = parse_graphql_type_ref(
            arg_object
                .get("type")
                .ok_or_else(|| format!("{arg_context} missing type"))?,
            &format!("{arg_context}.type"),
        )?;
        let rendered_type = render_graphql_type_ref(&arg_type);
        parsed_args.push(GraphqlArg {
            name: arg_name,
            required,
            type_ref: arg_type,
            rendered_type,
        });
    }
    Ok(parsed_args)
}

fn collect_named_members(object: &serde_json::Map<String, Value>, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn resolve_named_type(type_ref: &GraphqlTypeRef) -> Option<&str> {
    if let Some(name) = type_ref.name.as_deref() {
        return Some(name);
    }
    type_ref.of_type.as_deref().and_then(resolve_named_type)
}

fn unwrap_non_null(type_ref: &GraphqlTypeRef) -> &GraphqlTypeRef {
    if type_ref.kind == "NON_NULL" {
        return type_ref.of_type.as_deref().unwrap_or(type_ref);
    }
    type_ref
}

fn terminal_field_candidates() -> &'static [&'static str] {
    &[
        "id",
        "title",
        "name",
        "fullName",
        "fixedPath",
        "url",
        "content",
        "contentSummaryHtml",
        "path",
        "account",
        "realName",
        "date",
        "anchor",
        "createdAt",
        "updatedAt",
        "__typename",
    ]
}

fn render_terminal_fields(type_def: &GraphqlTypeDefinition) -> String {
    let mut fields = Vec::new();
    for candidate in terminal_field_candidates() {
        if type_def.fields.iter().any(|field| field.name == *candidate) {
            fields.push((*candidate).to_string());
        }
    }
    if fields.is_empty() && !type_def.fields.is_empty() {
        fields.push(type_def.fields[0].name.clone());
    }
    if fields.is_empty() {
        fields.push("__typename".to_string());
    }
    fields.join("\n")
}

fn required_arg_literal(
    type_ref: &GraphqlTypeRef,
    type_map: &HashMap<String, GraphqlTypeDefinition>,
) -> Option<String> {
    let type_ref = unwrap_non_null(type_ref);
    match type_ref.kind.as_str() {
        "LIST" => Some("[]".to_string()),
        "SCALAR" => match type_ref.name.as_deref().unwrap_or("") {
            "Int" => Some("16".to_string()),
            "Float" => Some("1.0".to_string()),
            "Boolean" => Some("false".to_string()),
            "ID" | "String" => Some("\"stub\"".to_string()),
            _ => None,
        },
        "ENUM" => type_ref.name.as_deref().and_then(|name| {
            type_map
                .get(name)
                .and_then(|value| value.enum_values.first())
                .cloned()
        }),
        _ => None,
    }
}

fn render_required_args(
    args: &[GraphqlArg],
    type_map: &HashMap<String, GraphqlTypeDefinition>,
) -> Option<String> {
    let mut rendered = Vec::new();
    for arg in args {
        if !arg.required {
            continue;
        }
        let literal = required_arg_literal(&arg.type_ref, type_map)?;
        rendered.push(format!("{}: {literal}", arg.name));
    }
    Some(rendered.join(", "))
}

fn render_selection_set(
    type_ref: &GraphqlTypeRef,
    type_map: &HashMap<String, GraphqlTypeDefinition>,
    stack: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) -> Option<String> {
    let named = resolve_named_type(type_ref)?;
    let type_def = type_map.get(named)?;
    match type_def.kind.as_str() {
        "SCALAR" | "ENUM" => None,
        "UNION" => {
            let mut fragments = vec!["__typename".to_string()];
            if depth >= max_depth {
                return Some(fragments.join("\n"));
            }
            for possible_type in &type_def.possible_types {
                if let Some(possible_def) = type_map.get(possible_type) {
                    let inner = render_terminal_fields(possible_def);
                    fragments.push(format!(
                        "... on {possible_type} {{\n{}\n}}",
                        indent_block(&inner, 2)
                    ));
                }
            }
            Some(fragments.join("\n"))
        }
        _ => {
            if depth >= max_depth || stack.iter().any(|entry| entry == named) {
                return Some(render_terminal_fields(type_def));
            }
            stack.push(named.to_string());
            let mut selected_fields = Vec::new();
            for field in &type_def.fields {
                if field.name.starts_with("__") {
                    continue;
                }
                let required_args = match render_required_args(&field.args, type_map) {
                    Some(value) => value,
                    None => continue,
                };
                let field_head = if required_args.is_empty() {
                    field.name.clone()
                } else {
                    format!("{}({required_args})", field.name)
                };
                if let Some(child_selection) =
                    render_selection_set(&field.type_ref, type_map, stack, depth + 1, max_depth)
                {
                    selected_fields.push(format!(
                        "{field_head} {{\n{}\n}}",
                        indent_block(&child_selection, 2)
                    ));
                } else {
                    selected_fields.push(field_head);
                }
            }
            stack.pop();
            if selected_fields.is_empty() {
                Some(render_terminal_fields(type_def))
            } else {
                Some(selected_fields.join("\n"))
            }
        }
    }
}

fn indent_block(value: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_operation_document(
    definition: &ResourceDefinition,
    field_spec: &GraphqlFieldSpec,
    type_map: &HashMap<String, GraphqlTypeDefinition>,
) -> Option<String> {
    let mut variable_defs = Vec::new();
    let mut call_args = Vec::new();
    for arg in &field_spec.args {
        variable_defs.push(format!("${}: {}", arg.name, arg.rendered_type));
        call_args.push(format!("{}: ${}", arg.name, arg.name));
    }
    let variable_defs_raw = variable_defs.join(", ");
    let call_args_raw = call_args.join(", ");
    let field_head = if call_args_raw.is_empty() {
        definition.field.to_string()
    } else {
        format!("{}({call_args_raw})", definition.field)
    };
    let mut stack = Vec::new();
    let selection = render_selection_set(&field_spec.return_type, type_map, &mut stack, 0, 8);
    let root_block = if let Some(selection) = selection {
        format!("{field_head} {{\n{}\n  }}", indent_block(&selection, 4))
    } else {
        field_head
    };
    let operation = to_pascal_case(definition.name);
    let operation_kind = if definition.kind == "mutation" {
        "mutation"
    } else {
        "query"
    };
    if variable_defs_raw.is_empty() {
        Some(format!(
            "{operation_kind} {operation} {{\n  {root_block}\n}}"
        ))
    } else {
        Some(format!(
            "{operation_kind} {operation}({variable_defs_raw}) {{\n  {root_block}\n}}"
        ))
    }
}

fn get_trimmed_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> ToolResult<String> {
    let value = object
        .get(key)
        .ok_or_else(|| format!("{context} missing `{key}`"))?;
    Ok(value_to_string(value).trim().to_string())
}

fn parse_schema_contract_version(
    object: &serde_json::Map<String, Value>,
    context: &str,
) -> ToolResult<u32> {
    let raw = object
        .get("schema_contract_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} must contain numeric `schema_contract_version`"))?;
    Ok(
        u32::try_from(raw)
            .map_err(|_| format!("{context} schema_contract_version out of range"))?,
    )
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

fn load_create_note_snapshot(path: &Path) -> ToolResult<CreateNoteSnapshot> {
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

    let input_set = input_fields
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let payload_set = payload_fields
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    let missing_input = required_input_fields
        .iter()
        .filter(|field| !input_set.contains(field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_input.is_empty() {
        return Err((format!("missing required input fields: {missing_input:?}")).into());
    }

    let missing_payload = required_payload_fields
        .iter()
        .filter(|field| !payload_set.contains(field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_payload.is_empty() {
        return Err((format!("missing required payload fields: {missing_payload:?}")).into());
    }

    if !note_projection_fields.iter().any(|field| field == "id") {
        return Err(("create_note_note_projection_fields must include `id`".to_string()).into());
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

fn run_create_note_contract_check(root: &Path, args: &CreateNoteContractArgs) -> ToolResult<()> {
    let snapshot_path = resolve_path(root, &args.snapshot);
    let generated_path = resolve_path(root, &args.generated);
    let snapshot = load_create_note_snapshot(&snapshot_path)?;
    let expected = render_create_note_module(&snapshot);
    let actual = fs::read_to_string(&generated_path)
        .map_err(|error| format!("failed to read {}: {error}", generated_path.display()))?;
    if actual != expected {
        return Err(("generated file is stale. run:\n\
             cargo run -p kibel-tools -- create-note-contract write"
            .to_string())
        .into());
    }
    println!("schema contract check: ok");
    Ok(())
}

fn run_create_note_contract_write(root: &Path, args: &CreateNoteContractArgs) -> ToolResult<()> {
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

fn run_create_note_contract_refresh_from_endpoint(
    root: &Path,
    args: &CreateNoteRefreshFromEndpointArgs,
) -> ToolResult<()> {
    let endpoint_snapshot_path = resolve_path(root, &args.endpoint_snapshot);
    let endpoint_snapshot_payload = read_json(&endpoint_snapshot_path)?;
    let snapshot_value =
        build_create_note_snapshot_from_endpoint_snapshot(&endpoint_snapshot_payload)?;

    let snapshot_path = resolve_path(root, &args.snapshot);
    write_json_pretty(&snapshot_path, &snapshot_value)?;
    println!("create-note contract refresh from endpoint snapshot: ok (written)");
    Ok(())
}

fn build_create_note_schema_from_endpoint_introspection(
    payload: &Value,
) -> ToolResult<CreateNoteSnapshot> {
    let input_fields = collect_schema_type_member_names(
        payload,
        "CreateNoteInput",
        "inputFields",
        "endpoint introspection",
    )?;
    let payload_fields = collect_schema_type_member_names(
        payload,
        "CreateNotePayload",
        "fields",
        "endpoint introspection",
    )?;
    let note_projection_fields =
        collect_schema_type_member_names(payload, "Note", "fields", "endpoint introspection")?;

    validate_create_note_schema_fields(
        &input_fields,
        &payload_fields,
        &note_projection_fields,
        "endpoint introspection",
    )?;

    Ok(CreateNoteSnapshot {
        input: input_fields,
        payload: payload_fields,
        note_projection: note_projection_fields,
    })
}

fn build_create_note_snapshot_from_endpoint_snapshot(payload: &Value) -> ToolResult<Value> {
    let context = "endpoint snapshot";
    let object = payload
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let create_note = object
        .get("create_note_schema")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{context} missing `create_note_schema`"))?;

    let input_fields = normalize_string_list(
        create_note
            .get("input_fields")
            .ok_or_else(|| format!("{context}.create_note_schema missing `input_fields`"))?,
        &format!("{context}.create_note_schema.input_fields"),
    )?;
    let payload_fields = normalize_string_list(
        create_note
            .get("payload_fields")
            .ok_or_else(|| format!("{context}.create_note_schema missing `payload_fields`"))?,
        &format!("{context}.create_note_schema.payload_fields"),
    )?;
    let note_projection_fields = normalize_string_list(
        create_note.get("note_projection_fields").ok_or_else(|| {
            format!("{context}.create_note_schema missing `note_projection_fields`")
        })?,
        &format!("{context}.create_note_schema.note_projection_fields"),
    )?;
    validate_create_note_schema_fields(
        &input_fields,
        &payload_fields,
        &note_projection_fields,
        context,
    )?;

    let captured_at = object
        .get("captured_at")
        .map(value_to_string)
        .unwrap_or_default();
    let origin = object
        .get("origin")
        .map(value_to_string)
        .unwrap_or_default();
    let endpoint = object
        .get("endpoint")
        .map(value_to_string)
        .unwrap_or_default();

    Ok(create_note_snapshot_value(
        &input_fields,
        &payload_fields,
        &note_projection_fields,
        &origin,
        &format!("endpoint-snapshot:{endpoint}"),
        &captured_at,
    ))
}

fn collect_schema_type_member_names(
    payload: &Value,
    type_name: &str,
    member_key: &str,
    context: &str,
) -> ToolResult<Vec<String>> {
    let types = payload
        .pointer("/data/__schema/types")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{context} missing /data/__schema/types array"))?;
    let type_object = types
        .iter()
        .filter_map(Value::as_object)
        .find(|item| {
            item.get("name")
                .and_then(Value::as_str)
                .is_some_and(|value| value.trim() == type_name)
        })
        .ok_or_else(|| format!("{context} missing type `{type_name}`"))?;
    let members = type_object
        .get(member_key)
        .ok_or_else(|| format!("{context} type `{type_name}` missing `{member_key}`"))?;
    collect_graphql_name_list(members, &format!("{type_name}.{member_key}"))
}

fn validate_create_note_schema_fields(
    input_fields: &[String],
    payload_fields: &[String],
    note_projection_fields: &[String],
    context: &str,
) -> ToolResult<()> {
    let input_set = input_fields
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let payload_set = payload_fields
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let missing_required_input = REQUIRED_CREATE_NOTE_INPUT_FIELDS
        .iter()
        .filter(|field| !input_set.contains(*field))
        .copied()
        .collect::<Vec<_>>();
    if !missing_required_input.is_empty() {
        return Err((format!(
            "{context} missing required create-note input fields: {}",
            missing_required_input.join(", ")
        ))
        .into());
    }
    let missing_required_payload = REQUIRED_CREATE_NOTE_PAYLOAD_FIELDS
        .iter()
        .filter(|field| !payload_set.contains(*field))
        .copied()
        .collect::<Vec<_>>();
    if !missing_required_payload.is_empty() {
        return Err((format!(
            "{context} missing required create-note payload fields: {}",
            missing_required_payload.join(", ")
        ))
        .into());
    }
    if !note_projection_fields.iter().any(|field| field == "id") {
        return Err((format!("{context} note projection must include `id`")).into());
    }
    Ok(())
}

fn create_note_snapshot_value(
    input_fields: &[String],
    payload_fields: &[String],
    note_projection_fields: &[String],
    origin: &str,
    artifact: &str,
    captured_at: &str,
) -> Value {
    json!({
        "schema_contract_version": 1,
        "captured_at": captured_at,
        "source": {
            "origin": origin,
            "artifact": artifact,
        },
        "create_note_input_fields": input_fields,
        "create_note_payload_fields": payload_fields,
        "create_note_note_projection_fields": note_projection_fields,
        "required_input_fields": REQUIRED_CREATE_NOTE_INPUT_FIELDS,
        "required_payload_fields": REQUIRED_CREATE_NOTE_PAYLOAD_FIELDS,
    })
}

fn build_endpoint_snapshot_from_introspection(
    definitions: &[ResourceDefinition],
    payload: &Value,
    origin: &str,
    endpoint: &str,
    captured_at: &str,
) -> ToolResult<Value> {
    let query_fields = parse_graphql_fields(payload, "query")?;
    let mutation_fields = parse_graphql_fields(payload, "mutation")?;
    let type_map = parse_schema_types(payload)?;
    let create_note_schema = build_create_note_schema_from_endpoint_introspection(payload)?;

    let mut resources = Vec::new();
    for definition in definitions {
        let fields = match definition.kind {
            "query" => &query_fields,
            "mutation" => &mutation_fields,
            other => return Err((format!("unsupported kind: {other}")).into()),
        };
        let field_spec = fields
            .get(definition.field)
            .ok_or_else(|| format!("missing graphql field: {}", definition.field))?;

        let mut all_variables = Vec::new();
        let mut required_variables = Vec::new();
        let mut seen = HashSet::new();
        for arg in &field_spec.args {
            if !seen.insert(arg.name.clone()) {
                continue;
            }
            all_variables.push(arg.name.clone());
            if arg.required {
                required_variables.push(arg.name.clone());
            }
        }
        let document =
            build_operation_document(definition, field_spec, &type_map).ok_or_else(|| {
                format!(
                    "failed to build operation document for `{}` from endpoint introspection",
                    definition.name
                )
            })?;

        resources.push(json!({
            "name": definition.name,
            "kind": definition.kind,
            "field": definition.field,
            "operation": to_pascal_case(definition.name),
            "client_method": definition.client_method,
            "all_variables": all_variables,
            "required_variables": required_variables,
            "document": document,
        }));
    }

    Ok(json!({
        "schema_contract_version": 1,
        "captured_at": captured_at,
        "origin": origin,
        "endpoint": endpoint,
        "resource_count": definitions.len(),
        "create_note_schema": {
            "input_fields": create_note_schema.input,
            "payload_fields": create_note_schema.payload,
            "note_projection_fields": create_note_schema.note_projection,
            "required_input_fields": REQUIRED_CREATE_NOTE_INPUT_FIELDS,
            "required_payload_fields": REQUIRED_CREATE_NOTE_PAYLOAD_FIELDS,
        },
        "resources": resources,
    }))
}

#[allow(clippy::too_many_lines)]
fn load_endpoint_snapshot(path: &Path) -> ToolResult<EndpointSnapshot> {
    let payload = read_json(path)?;
    parse_endpoint_snapshot(&payload)
}

fn parse_endpoint_snapshot(payload: &Value) -> ToolResult<EndpointSnapshot> {
    let object = payload
        .as_object()
        .ok_or_else(|| "endpoint snapshot must be an object".to_string())?;
    let resources_array = endpoint_snapshot_resources_array(object)?;
    let resources = parse_endpoint_resources(resources_array)?;
    validate_endpoint_resource_coverage(&resources)?;
    Ok(EndpointSnapshot {
        captured_at: endpoint_snapshot_meta_value(object, "captured_at"),
        origin: endpoint_snapshot_meta_value(object, "origin"),
        endpoint: endpoint_snapshot_meta_value(object, "endpoint"),
        resources,
    })
}

fn endpoint_snapshot_resources_array(
    object: &serde_json::Map<String, Value>,
) -> ToolResult<&[Value]> {
    let resources_value = object
        .get("resources")
        .ok_or_else(|| "endpoint snapshot must contain array `resources`".to_string())?;
    Ok(resources_value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "endpoint snapshot must contain array `resources`".to_string())?)
}

fn parse_endpoint_resources(
    resources_array: &[Value],
) -> ToolResult<HashMap<String, EndpointResource>> {
    let mut resources = HashMap::new();
    for (index, item) in resources_array.iter().enumerate() {
        let resource = parse_endpoint_resource(item, index)?;
        if resources.contains_key(&resource.name) {
            return Err((format!(
                "duplicate resource name in endpoint snapshot: {}",
                resource.name
            ))
            .into());
        }
        resources.insert(resource.name.clone(), resource);
    }
    Ok(resources)
}

fn parse_endpoint_resource(item: &Value, index: usize) -> ToolResult<EndpointResource> {
    let context = format!("resources[{index}]");
    let object = item
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;

    let name = get_trimmed_string(object, "name", &context)?;
    if name.is_empty() {
        return Err((format!("{context} has empty name")).into());
    }

    let kind = get_trimmed_string(object, "kind", &context)?;
    if kind != "query" && kind != "mutation" {
        return Err((format!("resource `{name}` has invalid kind: {kind}")).into());
    }
    let field = get_trimmed_string(object, "field", &context)?;
    let operation = get_trimmed_string(object, "operation", &context)?;
    let client_method = get_trimmed_string(object, "client_method", &context)?;
    if field.is_empty() || operation.is_empty() || client_method.is_empty() {
        return Err((format!(
            "resource `{name}` must have non-empty field/operation/client_method"
        ))
        .into());
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
    validate_required_subset(&name, &all_variables, &required_variables)?;

    let document = parse_endpoint_resource_document(object, &name)?;
    Ok(EndpointResource {
        name,
        kind,
        field,
        operation,
        client_method,
        all_variables,
        required_variables,
        document,
    })
}

fn validate_required_subset(
    resource_name: &str,
    all_variables: &[String],
    required_variables: &[String],
) -> ToolResult<()> {
    let all_set = all_variables.iter().collect::<HashSet<_>>();
    let missing_required = required_variables
        .iter()
        .filter(|value| !all_set.contains(*value))
        .cloned()
        .collect::<Vec<_>>();
    if missing_required.is_empty() {
        return Ok(());
    }
    Err((format!(
        "resource `{resource_name}` has required vars not in all_variables: {missing_required:?}"
    ))
    .into())
}

fn parse_endpoint_resource_document(
    object: &serde_json::Map<String, Value>,
    resource_name: &str,
) -> ToolResult<String> {
    Ok(object
        .get("document")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "resource `{resource_name}` missing `document`; re-run refresh-endpoint to regenerate endpoint snapshot"
            )
        })?)
}

fn validate_endpoint_resource_coverage(
    resources: &HashMap<String, EndpointResource>,
) -> ToolResult<()> {
    let missing = resource_definitions()
        .iter()
        .filter(|definition| !resources.contains_key(definition.name))
        .map(|definition| definition.name.to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err((format!("endpoint snapshot missing resources: {missing:?}")).into());
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
    if unexpected.is_empty() {
        return Ok(());
    }
    Err(("endpoint snapshot contains unknown resources. \
         update RESOURCE_ORDER/CLI/client/tests first: "
        .to_string()
        + &format!("{unexpected:?}"))
        .into())
}

fn endpoint_snapshot_meta_value(object: &serde_json::Map<String, Value>, key: &str) -> String {
    object.get(key).map(value_to_string).unwrap_or_default()
}

fn build_resource_snapshot_value(
    root: &Path,
    endpoint_snapshot_path: &Path,
    endpoint_payload: &EndpointSnapshot,
) -> ToolResult<Value> {
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
            "document": item.document,
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

fn normalize_resource_snapshot(payload: &Value) -> ToolResult<NormalizedSnapshot> {
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
            return Err((format!("duplicate resource name: {}", resource.name)).into());
        }
        seen_names.insert(resource.name.clone());
        resources.push(resource);
    }
    if resources.is_empty() {
        return Err(("snapshot resources cannot be empty".to_string()).into());
    }
    resources.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(NormalizedSnapshot {
        schema_contract_version: version,
        source,
        resources,
    })
}

fn parse_normalized_resource(item: &Value, context: &str) -> ToolResult<NormalizedResource> {
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
        "document",
    ] {
        if !object.contains_key(key) {
            return Err((format!("{context} is missing `{key}`")).into());
        }
    }

    let name = get_trimmed_string(object, "name", context)?;
    if name.is_empty() {
        return Err((format!("{context} name is empty")).into());
    }
    let kind = get_trimmed_string(object, "kind", context)?;
    if kind != "query" && kind != "mutation" {
        return Err((format!("resource `{name}` has invalid kind: {kind}")).into());
    }
    let operation = get_trimmed_string(object, "operation", context)?;
    let graphql_file = get_trimmed_string(object, "graphql_file", context)?;
    let client_method = get_trimmed_string(object, "client_method", context)?;
    let document = get_trimmed_string(object, "document", context)?;
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
        return Err((format!(
            "resource `{name}` has required vars not in all_variables: {missing_required:?}"
        ))
        .into());
    }

    Ok(NormalizedResource {
        name,
        kind,
        operation,
        all_variables,
        required_variables,
        graphql_file,
        client_method,
        document,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceContractDiffResult {
    breaking: Vec<String>,
    notes: Vec<String>,
}

fn graphql_root_field(graphql_file: &str) -> Option<&str> {
    graphql_file
        .rsplit('.')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn compute_resource_contract_diff(
    base: &NormalizedSnapshot,
    target: &NormalizedSnapshot,
) -> ResourceContractDiffResult {
    let base_by_name = base
        .resources
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect::<HashMap<_, _>>();
    let target_by_name = target
        .resources
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect::<HashMap<_, _>>();

    let mut breaking = Vec::new();
    let mut notes = Vec::new();

    for base_item in &base.resources {
        let Some(target_item) = target_by_name.get(base_item.name.as_str()) else {
            breaking.push(format!(
                "resource removed: `{}` ({})",
                base_item.name, base_item.kind
            ));
            continue;
        };

        if base_item.kind != target_item.kind {
            breaking.push(format!(
                "resource kind changed: `{}` {} -> {}",
                base_item.name, base_item.kind, target_item.kind
            ));
        }

        let base_root = graphql_root_field(&base_item.graphql_file).unwrap_or("");
        let target_root = graphql_root_field(&target_item.graphql_file).unwrap_or("");
        if !base_root.is_empty() && !target_root.is_empty() && base_root != target_root {
            breaking.push(format!(
                "resource root field changed: `{}` {} -> {}",
                base_item.name, base_root, target_root
            ));
        }

        let base_required = base_item
            .required_variables
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let base_all = base_item
            .all_variables
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let target_required = target_item
            .required_variables
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let target_all = target_item
            .all_variables
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();

        let mut removed_all = base_all
            .difference(&target_all)
            .copied()
            .collect::<Vec<_>>();
        removed_all.sort_unstable();
        if !removed_all.is_empty() {
            breaking.push(format!(
                "resource variable(s) removed: `{}` {}",
                base_item.name,
                removed_all.join(", ")
            ));
        }

        let mut added_all = target_all
            .difference(&base_all)
            .copied()
            .collect::<Vec<_>>();
        added_all.sort_unstable();
        if !added_all.is_empty() {
            notes.push(format!(
                "resource variable(s) added: `{}` {}",
                base_item.name,
                added_all.join(", ")
            ));
        }

        let mut relaxed_required = base_required
            .difference(&target_required)
            .filter(|name| target_all.contains(**name))
            .copied()
            .collect::<Vec<_>>();
        relaxed_required.sort_unstable();
        if !relaxed_required.is_empty() {
            notes.push(format!(
                "resource required variable(s) no longer mandatory: `{}` {}",
                base_item.name,
                relaxed_required.join(", ")
            ));
        }

        let mut added_required = target_required
            .difference(&base_required)
            .copied()
            .collect::<Vec<_>>();
        added_required.sort_unstable();
        if !added_required.is_empty() {
            breaking.push(format!(
                "resource required variable(s) added: `{}` {}",
                base_item.name,
                added_required.join(", ")
            ));
        }
    }

    for target_item in &target.resources {
        if !base_by_name.contains_key(target_item.name.as_str()) {
            notes.push(format!(
                "resource added: `{}` ({})",
                target_item.name, target_item.kind
            ));
        }
    }

    breaking.sort();
    notes.sort();
    ResourceContractDiffResult { breaking, notes }
}

fn resource_contract_diff_json(diff: &ResourceContractDiffResult) -> Value {
    json!({
        "breaking": diff.breaking,
        "notes": diff.notes,
        "breaking_count": diff.breaking.len(),
        "notes_count": diff.notes.len(),
    })
}

fn load_resource_module_snapshot(path: &Path) -> ToolResult<ResourceModuleSnapshot> {
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
        return Err(("snapshot resources cannot be empty".to_string()).into());
    }

    let mut resources = Vec::new();
    let mut seen_names = HashSet::new();
    for (index, item) in resources_array.iter().enumerate() {
        let context = format!("resource[{index}]");
        let resource = parse_normalized_resource(item, &context)?;
        if seen_names.contains(&resource.name) {
            return Err((format!("duplicate resource name: {}", resource.name)).into());
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
        format!("        document: {},", rust_string(&resource.document)),
        "    },".to_string(),
    ]
    .join("\n")
}

fn trusted_operation_variant(resource: &NormalizedResource) -> String {
    to_pascal_case(&resource.name)
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
        "    pub document: &'static str,".to_string(),
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
    lines.push("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]".to_string());
    lines.push("pub enum TrustedOperation {".to_string());
    for resource in &snapshot.resources {
        lines.push(format!("    {},", trusted_operation_variant(resource)));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("pub const TRUSTED_OPERATIONS: &[TrustedOperation] = &[".to_string());
    for resource in &snapshot.resources {
        lines.push(format!(
            "    TrustedOperation::{},",
            trusted_operation_variant(resource)
        ));
    }
    lines.push("];".to_string());
    lines.push(String::new());
    lines.push(
        "pub const fn trusted_operation_contract_index(operation: TrustedOperation) -> usize {"
            .to_string(),
    );
    lines.push("    match operation {".to_string());
    for (index, resource) in snapshot.resources.iter().enumerate() {
        lines.push(format!(
            "        TrustedOperation::{} => {},",
            trusted_operation_variant(resource),
            index
        ));
    }
    lines.push("    }".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(
        "pub fn trusted_operation_contract(operation: TrustedOperation) -> &'static ResourceContract {"
            .to_string(),
    );
    lines.push("    &RESOURCE_CONTRACTS[trusted_operation_contract_index(operation)]".to_string());
    lines.push("}".to_string());
    lines.push(String::new());

    let mut rendered = lines.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

fn rustfmt_source(source: &str) -> ToolResult<String> {
    let temp_dir =
        tempfile::tempdir().map_err(|error| format!("failed to create temp dir: {error}"))?;
    let path = temp_dir.path().join("generated.rs");
    fs::write(&path, source).map_err(|error| format!("failed to write temp rust file: {error}"))?;
    let status = Command::new("rustfmt")
        .arg(&path)
        .status()
        .map_err(|error| format!("failed to run rustfmt: {error}"))?;
    if !status.success() {
        return Err(("rustfmt failed for generated module".to_string()).into());
    }
    Ok(fs::read_to_string(&path)
        .map_err(|error| format!("failed to read rustfmt output: {error}"))?)
}

fn run_resource_contract_check(root: &Path, args: &ResourceContractArgs) -> ToolResult<()> {
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
        return Err(("resource snapshot is stale. run:\n\
             cargo run -p kibel-tools -- resource-contract write"
            .to_string())
        .into());
    }

    let module_snapshot = load_resource_module_snapshot(&snapshot_path)?;
    let expected_generated = rustfmt_source(&render_resource_module(&module_snapshot))?;
    let actual_generated = fs::read_to_string(&generated_path)
        .map_err(|error| format!("failed to read {}: {error}", generated_path.display()))?;
    if actual_generated != expected_generated {
        return Err(("generated resource contract module is stale. run:\n\
             cargo run -p kibel-tools -- resource-contract write"
            .to_string())
        .into());
    }

    println!("resource contract check: ok");
    Ok(())
}

fn run_resource_contract_write(root: &Path, args: &ResourceContractArgs) -> ToolResult<()> {
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
) -> ToolResult<()> {
    let origin = args.origin.trim();
    if origin.is_empty() {
        return Err(("origin is required (use --origin or KIBELA_ORIGIN)".to_string()).into());
    }
    let token = args.token.trim();
    if token.is_empty() {
        return Err(("token is required (use --token or KIBELA_ACCESS_TOKEN)".to_string()).into());
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

fn load_normalized_snapshot_from_path(path: &Path) -> ToolResult<NormalizedSnapshot> {
    let payload = read_json(path)?;
    normalize_resource_snapshot(&payload)
}

fn run_resource_contract_diff(root: &Path, args: &ResourceContractDiffArgs) -> ToolResult<()> {
    let base_path = resolve_path(root, &args.base);
    let target_path = resolve_path(root, &args.target);
    let base_snapshot = load_normalized_snapshot_from_path(&base_path)?;
    let target_snapshot = load_normalized_snapshot_from_path(&target_path)?;
    let diff = compute_resource_contract_diff(&base_snapshot, &target_snapshot);

    match args.format {
        DiffOutputFormat::Text => {
            if diff.breaking.is_empty() {
                println!("resource contract diff: no breaking changes");
            } else {
                println!(
                    "resource contract diff: {} breaking change(s)",
                    diff.breaking.len()
                );
                for item in &diff.breaking {
                    println!("  - {item}");
                }
            }

            if !diff.notes.is_empty() {
                println!("resource contract diff notes:");
                for item in &diff.notes {
                    println!("  - {item}");
                }
            }
        }
        DiffOutputFormat::Json => {
            let rendered = serde_json::to_string_pretty(&resource_contract_diff_json(&diff))
                .map_err(|error| {
                    ToolError::message(format!("failed to render diff json: {error}"))
                })?;
            println!("{rendered}");
        }
    }

    if args.fail_on_breaking && !diff.breaking.is_empty() {
        return Err((format!(
            "resource contract diff detected {} breaking change(s)",
            diff.breaking.len()
        ))
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
