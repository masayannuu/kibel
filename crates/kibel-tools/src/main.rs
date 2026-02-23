use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const RESOURCE_ORDER: &[&str] = &[
    "searchNote",
    "searchFolder",
    "getGroups",
    "getFolders",
    "getNotes",
    "getNote",
    "getNoteFromPath",
    "getFolder",
    "getFolderFromPath",
    "getFeedSections",
    "createNote",
    "createComment",
    "createCommentReply",
    "createFolder",
    "moveNoteToAnotherFolder",
    "attachNoteToFolder",
    "updateNoteContent",
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

    let missing = RESOURCE_ORDER
        .iter()
        .filter(|name| !resources.contains_key(**name))
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!("endpoint snapshot missing resources: {missing:?}"));
    }

    let expected = RESOURCE_ORDER.iter().copied().collect::<BTreeSet<_>>();
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
    for name in RESOURCE_ORDER {
        let item = endpoint_payload
            .resources
            .get(*name)
            .ok_or_else(|| format!("endpoint snapshot missing resource `{name}`"))?;
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
}
