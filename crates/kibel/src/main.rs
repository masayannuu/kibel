mod cli;
mod error;

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use error::{CliError, ErrorCode};
use kibel_client::{
    default_config_path, require_team, resolve_access_token, resource_contracts,
    token_source_label, token_store_subject, AttachNoteToFolderInput, Config, CreateCommentInput,
    CreateCommentReplyInput, CreateFolderInput, CreateNoteFolderInput, CreateNoteInput,
    FeedSectionsInput, FolderLookupInput, GetNotesInput, KeychainTokenStore, KibelClient,
    MoveNoteToAnotherFolderInput, PageInput, PathLookupInput, ResolveTokenInput, SearchFolderInput,
    SearchNoteInput, TokenStore, UpdateNoteInput,
};
use rpassword::prompt_password;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
struct CommandOutput {
    data: Value,
    message: String,
}

#[derive(Debug)]
struct ClientContext {
    team: Option<String>,
    token_source: String,
    client: KibelClient,
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = cli::Cli::parse();
    let json_mode = cli.json
        || matches!(
            &cli.command,
            cli::Command::Version(cli::VersionArgs { json: true })
        );

    if let cli::Command::Completion(args) = &cli.command {
        let mut command = cli::Cli::command();
        generate(args.shell, &mut command, "kibel", &mut io::stdout());
        return 0;
    }

    let request_id = generated_request_id();
    let started = Instant::now();

    let result = execute(&cli);
    let elapsed_ms = started.elapsed().as_millis();

    match result {
        Ok(output) => {
            if json_mode {
                let envelope = json!({
                    "ok": true,
                    "data": output.data,
                    "error": Value::Null,
                    "meta": {
                        "request_id": request_id,
                        "elapsed_ms": elapsed_ms,
                    }
                });
                println!("{envelope}");
            } else {
                println!("{}", output.message);
            }
            0
        }
        Err(err) => {
            if json_mode {
                let envelope = json!({
                    "ok": false,
                    "data": Value::Null,
                    "error": {
                        "code": err.code.as_str(),
                        "message": err.message,
                        "retryable": err.code.retryable(),
                        "details": err.details,
                    },
                    "meta": {
                        "request_id": request_id,
                        "elapsed_ms": elapsed_ms,
                    }
                });
                println!("{envelope}");
            } else {
                eprintln!("[{}] {}", err.code.as_str(), err.message);
            }

            err.code.exit_code()
        }
    }
}

fn execute(cli: &cli::Cli) -> Result<CommandOutput, CliError> {
    let token_inputs_required = command_uses_token_inputs(&cli.command);
    let stdin_token = if token_inputs_required {
        read_stdin_token(cli.with_token)?
    } else {
        None
    };
    let env_token = if token_inputs_required {
        std::env::var(&cli.token_env).ok()
    } else {
        None
    };

    match &cli.command {
        cli::Command::Auth(args) => execute_auth(cli, args, stdin_token, env_token),
        cli::Command::Config(args) => execute_config(cli, args),
        cli::Command::Search(args) => execute_search(cli, args, stdin_token, env_token),
        cli::Command::Group(args) => execute_group(cli, args, stdin_token, env_token),
        cli::Command::Folder(args) => execute_folder(cli, args, stdin_token, env_token),
        cli::Command::Feed(args) => execute_feed(cli, args, stdin_token, env_token),
        cli::Command::Comment(args) => execute_comment(cli, args, stdin_token, env_token),
        cli::Command::Note(args) => execute_note(cli, args, stdin_token, env_token),
        cli::Command::Graphql(args) => execute_graphql(cli, args, stdin_token, env_token),
        cli::Command::Version(args) => Ok(execute_version(args)),
        cli::Command::Completion(_) => unreachable!("completion is handled before execute"),
    }
}

fn command_uses_token_inputs(command: &cli::Command) -> bool {
    match command {
        cli::Command::Auth(auth) => {
            matches!(
                &auth.command,
                cli::AuthCommand::Login(_) | cli::AuthCommand::Status(_)
            )
        }
        cli::Command::Search(_)
        | cli::Command::Group(_)
        | cli::Command::Folder(_)
        | cli::Command::Feed(_)
        | cli::Command::Comment(_)
        | cli::Command::Note(_)
        | cli::Command::Graphql(_) => true,
        cli::Command::Config(_) | cli::Command::Completion(_) | cli::Command::Version(_) => false,
    }
}

fn execute_auth(
    cli: &cli::Cli,
    args: &cli::AuthArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    match &args.command {
        cli::AuthCommand::Login(command) => {
            let (config_path, mut config) = load_config(cli.config_path.clone())?;
            let interactive = is_interactive_terminal();
            let requested_team = command
                .team
                .clone()
                .or_else(|| requested_team_from_cli(cli));
            let requested_origin = requested_origin_from_cli(cli);
            let origin = resolve_login_origin(
                requested_origin.as_deref(),
                requested_team.as_deref(),
                &config,
                interactive,
            )?;
            let team = resolve_login_team(
                requested_team.as_deref(),
                Some(&origin),
                &config,
                interactive,
            )?;
            let (token, token_source) = resolve_login_token(
                cli.with_token,
                stdin_token.as_deref(),
                env_token.as_deref(),
                interactive,
            )?;

            let store = KeychainTokenStore::default();
            let subject = token_store_subject(&team, Some(&origin));
            let mut stored_in = Vec::new();
            let mut keychain_error = None;

            match store.set_token(&subject, &token) {
                Ok(()) => stored_in.push("keychain"),
                Err(err) => keychain_error = Some(err.to_string()),
            }

            config.set_profile_token(&team, &token);
            config.set_profile_origin(&team, &origin);
            config.set_default_team_if_missing(&team);
            config.save(&config_path)?;
            stored_in.push("config");
            let access_token_settings_url = kibela_access_token_settings_url(&origin);

            Ok(CommandOutput {
                data: json!({
                    "team": team,
                    "origin": origin,
                    "token_source": token_source,
                    "stored_in": stored_in,
                    "token_store_subject": subject,
                    "keychain_available": keychain_error.is_none(),
                    "keychain_error": keychain_error,
                    "access_token_settings_url": access_token_settings_url,
                    "config_path": config_path,
                }),
                message: "auth login completed".to_string(),
            })
        }
        cli::AuthCommand::Logout(command) => {
            let (config_path, mut config) = load_config(cli.config_path.clone())?;
            let requested_team = command
                .team
                .clone()
                .or_else(|| requested_team_from_cli(cli));
            let team = require_team(requested_team.as_deref(), &config)?;
            let requested_origin = requested_origin_from_cli(cli);
            let resolved_origin = config.resolve_origin(requested_origin.as_deref(), Some(&team));

            let store = KeychainTokenStore::default();
            let mut keychain_deleted = false;
            let mut keychain_error = None;
            let mut subjects = token_store_lookup_subjects(&team, resolved_origin.as_deref());
            if subjects.is_empty() {
                subjects.push(team.clone());
            }
            for subject in subjects {
                match store.delete_token(&subject) {
                    Ok(()) => keychain_deleted = true,
                    Err(err) => keychain_error = Some(err.to_string()),
                }
            }
            let config_token_removed = config.clear_profile_token(&team);
            config.save(&config_path)?;

            Ok(CommandOutput {
                data: json!({
                    "team": team,
                    "origin": resolved_origin,
                    "keychain_deleted": keychain_deleted,
                    "keychain_error": keychain_error,
                    "config_token_removed": config_token_removed,
                    "config_path": config_path,
                }),
                message: "auth logout completed".to_string(),
            })
        }
        cli::AuthCommand::Status(command) => {
            let (_, config) = load_config(cli.config_path.clone())?;
            let requested_team = command
                .team
                .clone()
                .or_else(|| requested_team_from_cli(cli));
            let requested_origin = requested_origin_from_cli(cli);
            let resolved = resolve_access_token(
                &ResolveTokenInput {
                    requested_team,
                    requested_origin,
                    stdin_token,
                    env_token,
                },
                &config,
                &KeychainTokenStore::default(),
            )?;

            let data = if let Some(token) = resolved {
                json!({
                    "logged_in": true,
                    "team": token.team,
                    "token_source": token_source_label(token.source),
                })
            } else {
                json!({
                    "logged_in": false,
                    "team": config.default_team,
                    "token_source": Value::Null,
                })
            };

            let message = if data
                .get("logged_in")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "auth status: logged in"
            } else {
                "auth status: not logged in"
            }
            .to_string();

            Ok(CommandOutput { data, message })
        }
    }
}

fn execute_config(cli: &cli::Cli, args: &cli::ConfigArgs) -> Result<CommandOutput, CliError> {
    let (config_path, mut config) = load_config(cli.config_path.clone())?;
    match &args.command {
        cli::ConfigCommand::Set(command) => match &command.command {
            cli::ConfigSetCommand::Team(set_team) => {
                let team = normalize_owned(&set_team.team).ok_or_else(|| {
                    CliError::new(
                        ErrorCode::InputInvalid,
                        "team is required for `config set team`",
                    )
                })?;
                config.set_default_team(&team);
                config.save(&config_path)?;

                Ok(CommandOutput {
                    data: json!({
                        "default_team": team,
                        "config_path": config_path,
                    }),
                    message: "config set team completed".to_string(),
                })
            }
        },
        cli::ConfigCommand::Profiles(_) => {
            let mut profiles = Vec::new();
            for (team, profile) in &config.profiles {
                let has_token = profile
                    .token
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|token| !token.is_empty());
                let origin = profile
                    .origin
                    .as_deref()
                    .map(str::trim)
                    .filter(|origin| !origin.is_empty())
                    .map(ToOwned::to_owned);

                profiles.push(json!({
                    "team": team,
                    "has_token": has_token,
                    "has_origin": origin.is_some(),
                    "origin": origin,
                }));
            }

            Ok(CommandOutput {
                data: json!({
                    "default_team": config.default_team,
                    "profiles": profiles,
                    "config_path": config_path,
                }),
                message: "config profiles listed".to_string(),
            })
        }
    }
}

fn execute_search(
    cli: &cli::Cli,
    args: &cli::SearchArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::SearchCommand::Note(command) => {
            if command.mine {
                if search_note_mine_has_unsupported_filters(command) {
                    return Err(CliError::new(
                        ErrorCode::InputInvalid,
                        "--mine cannot be combined with other search filters",
                    ));
                }
                let results = ctx.client.get_current_user_latest_notes(PageInput {
                    first: command.first,
                })?;
                return Ok(CommandOutput {
                    data: json!({
                        "results": results,
                        "meta": context_meta(&ctx),
                    }),
                    message: "search note completed".to_string(),
                });
            }

            let user_ids = command.user_ids.clone();
            let results = ctx.client.search_note(&SearchNoteInput {
                query: command.query.clone(),
                resources: command.resources.clone(),
                coediting: command.coediting,
                updated: command.updated.clone(),
                group_ids: command.group_ids.clone(),
                user_ids,
                folder_ids: command.folder_ids.clone(),
                liker_ids: command.liker_ids.clone(),
                is_archived: command.is_archived,
                sort_by: command.sort_by.clone(),
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "results": results,
                    "meta": context_meta(&ctx),
                }),
                message: "search note completed".to_string(),
            })
        }
        cli::SearchCommand::Folder(command) => {
            let results = ctx.client.search_folder(&SearchFolderInput {
                query: command.query.clone(),
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "results": results,
                    "meta": context_meta(&ctx),
                }),
                message: "search folder completed".to_string(),
            })
        }
    }
}

fn search_note_mine_has_unsupported_filters(command: &cli::SearchNoteArgs) -> bool {
    !command.query.trim().is_empty()
        || !command.resources.is_empty()
        || command.coediting.is_some()
        || command.updated.is_some()
        || !command.group_ids.is_empty()
        || !command.user_ids.is_empty()
        || !command.folder_ids.is_empty()
        || !command.liker_ids.is_empty()
        || command.is_archived.is_some()
        || command.sort_by.is_some()
}

fn execute_group(
    cli: &cli::Cli,
    args: &cli::GroupArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::GroupCommand::List(command) => {
            let groups = ctx.client.get_groups(PageInput {
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "groups": groups,
                    "meta": context_meta(&ctx),
                }),
                message: "group list completed".to_string(),
            })
        }
    }
}

fn execute_folder(
    cli: &cli::Cli,
    args: &cli::FolderArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::FolderCommand::List(command) => {
            let folders = ctx.client.get_folders(PageInput {
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "folders": folders,
                    "meta": context_meta(&ctx),
                }),
                message: "folder list completed".to_string(),
            })
        }
        cli::FolderCommand::Get(command) => {
            let folder = ctx.client.get_folder(&FolderLookupInput {
                id: command.id.clone(),
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "folder": folder,
                    "meta": context_meta(&ctx),
                }),
                message: "folder get completed".to_string(),
            })
        }
        cli::FolderCommand::GetFromPath(command) => {
            let folder = ctx.client.get_folder_from_path(&PathLookupInput {
                path: command.path.clone(),
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "folder": folder,
                    "meta": context_meta(&ctx),
                }),
                message: "folder get-from-path completed".to_string(),
            })
        }
        cli::FolderCommand::Notes(command) => {
            let notes = ctx.client.get_notes(&GetNotesInput {
                folder_id: command.folder_id.clone(),
                first: command.first,
                last: command.last,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "notes": notes,
                    "meta": context_meta(&ctx),
                }),
                message: "folder notes completed".to_string(),
            })
        }
        cli::FolderCommand::Create(command) => {
            let folder = ctx.client.create_folder(&CreateFolderInput {
                group_id: command.group_id.clone(),
                full_name: command.full_name.clone(),
            })?;
            Ok(CommandOutput {
                data: json!({
                    "folder": folder,
                    "meta": context_meta(&ctx),
                }),
                message: "folder create completed".to_string(),
            })
        }
    }
}

fn execute_feed(
    cli: &cli::Cli,
    args: &cli::FeedArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::FeedCommand::Sections(command) => {
            let sections = ctx.client.get_feed_sections(&FeedSectionsInput {
                kind: command.kind.clone(),
                group_id: command.group_id.clone(),
                first: command.first,
            })?;
            Ok(CommandOutput {
                data: json!({
                    "sections": sections,
                    "meta": context_meta(&ctx),
                }),
                message: "feed sections completed".to_string(),
            })
        }
    }
}

fn execute_comment(
    cli: &cli::Cli,
    args: &cli::CommentArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::CommentCommand::Create(command) => {
            let comment = ctx.client.create_comment(&CreateCommentInput {
                content: command.content.clone(),
                note_id: command.note_id.clone(),
            })?;
            Ok(CommandOutput {
                data: json!({
                    "comment": comment,
                    "meta": context_meta(&ctx),
                }),
                message: "comment create completed".to_string(),
            })
        }
        cli::CommentCommand::Reply(command) => {
            let reply = ctx.client.create_comment_reply(&CreateCommentReplyInput {
                content: command.content.clone(),
                comment_id: command.comment_id.clone(),
            })?;
            Ok(CommandOutput {
                data: json!({
                    "reply": reply,
                    "meta": context_meta(&ctx),
                }),
                message: "comment reply completed".to_string(),
            })
        }
    }
}

#[allow(clippy::too_many_lines)]
fn execute_note(
    cli: &cli::Cli,
    args: &cli::NoteArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::NoteCommand::Create(command) => {
            let client_mutation_id = command
                .client_mutation_id
                .clone()
                .or_else(|| command.idempotency_key.clone());
            let folders = command
                .folders
                .iter()
                .map(note_folder_arg_to_input)
                .collect::<Vec<_>>();
            let created = ctx.client.create_note(&CreateNoteInput {
                title: command.title.clone(),
                content: command.content.clone(),
                group_ids: command.group_ids.clone(),
                draft: if command.draft { Some(true) } else { None },
                coediting: command.coediting,
                folders,
                author_id: command.author_id.clone(),
                published_at: command.published_at.clone(),
                client_mutation_id: client_mutation_id.clone(),
            })?;

            Ok(CommandOutput {
                data: json!({
                    "note": created.note,
                    "meta": {
                        "team": ctx.team,
                        "origin": ctx.client.origin(),
                        "token_source": ctx.token_source,
                        "client_mutation_id": created.client_mutation_id.or(client_mutation_id),
                    }
                }),
                message: "note create completed".to_string(),
            })
        }
        cli::NoteCommand::Get(command) => {
            let note = ctx.client.get_note(&command.id)?;

            Ok(CommandOutput {
                data: json!({
                    "note": note,
                    "meta": context_meta(&ctx),
                }),
                message: "note get completed".to_string(),
            })
        }
        cli::NoteCommand::GetFromPath(command) => {
            let note = ctx.client.get_note_from_path(&PathLookupInput {
                path: command.path.clone(),
                first: command.first,
            })?;

            Ok(CommandOutput {
                data: json!({
                    "note": note,
                    "meta": context_meta(&ctx),
                }),
                message: "note get-from-path completed".to_string(),
            })
        }
        cli::NoteCommand::Update(command) => {
            let note = ctx.client.update_note(&UpdateNoteInput {
                id: command.id.clone(),
                base_content: command.base_content.clone(),
                new_content: command.new_content.clone(),
            })?;

            Ok(CommandOutput {
                data: json!({
                    "note": note,
                    "meta": context_meta(&ctx),
                }),
                message: "note update completed".to_string(),
            })
        }
        cli::NoteCommand::MoveToFolder(command) => {
            let note = ctx
                .client
                .move_note_to_another_folder(&MoveNoteToAnotherFolderInput {
                    id: command.id.clone(),
                    from_folder: note_folder_arg_to_input(&command.from_folder),
                    to_folder: note_folder_arg_to_input(&command.to_folder),
                })?;

            Ok(CommandOutput {
                data: json!({
                    "note": note,
                    "meta": context_meta(&ctx),
                }),
                message: "note move-to-folder completed".to_string(),
            })
        }
        cli::NoteCommand::AttachToFolder(command) => {
            let note = ctx.client.attach_note_to_folder(&AttachNoteToFolderInput {
                id: command.id.clone(),
                folder: note_folder_arg_to_input(&command.folder),
            })?;

            Ok(CommandOutput {
                data: json!({
                    "note": note,
                    "meta": context_meta(&ctx),
                }),
                message: "note attach-to-folder completed".to_string(),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphqlOperationKind {
    Query,
    Mutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueryShape {
    max_depth: u32,
    complexity: u32,
}

#[derive(Debug, Clone, Copy)]
struct GraphqlGuardrails {
    timeout_secs: u64,
    response_limit_bytes: usize,
    max_depth: u32,
    max_complexity: u32,
    allow_mutation: bool,
    unsafe_no_cost_check: bool,
}

fn execute_graphql(
    cli: &cli::Cli,
    args: &cli::GraphqlArgs,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<CommandOutput, CliError> {
    let ctx = resolve_client_context(cli, stdin_token, env_token)?;

    match &args.command {
        cli::GraphqlCommand::Run(command) => {
            let query = resolve_graphql_query(command)?;
            let variables = resolve_graphql_variables(command)?;
            let guardrails = build_graphql_guardrails(command)?;
            enforce_graphql_guardrails(&query, &variables, guardrails)?;

            let response = ctx.client.run_untrusted_graphql(
                &query,
                variables,
                guardrails.timeout_secs.saturating_mul(1000),
                guardrails.response_limit_bytes,
            )?;

            Ok(CommandOutput {
                data: json!({
                    "response": response,
                    "meta": {
                        "team": ctx.team,
                        "origin": ctx.client.origin(),
                        "token_source": ctx.token_source,
                        "guardrails": {
                            "timeout_secs": guardrails.timeout_secs,
                            "response_limit_bytes": guardrails.response_limit_bytes,
                            "max_depth": guardrails.max_depth,
                            "max_complexity": guardrails.max_complexity,
                            "allow_mutation": guardrails.allow_mutation,
                            "unsafe_no_cost_check": guardrails.unsafe_no_cost_check,
                        }
                    }
                }),
                message: "graphql run completed".to_string(),
            })
        }
    }
}

fn resolve_graphql_query(command: &cli::GraphqlRunArgs) -> Result<String, CliError> {
    if let Some(raw) = command.query.as_deref() {
        return normalize_owned(raw).ok_or_else(|| {
            CliError::new(
                ErrorCode::InputInvalid,
                "--query is empty; provide GraphQL query text",
            )
        });
    }

    if let Some(path) = &command.query_file {
        let raw = fs::read_to_string(path).map_err(|error| {
            CliError::new(
                ErrorCode::TransportError,
                format!("failed to read query file {}: {error}", path.display()),
            )
        })?;
        return normalize_owned(&raw).ok_or_else(|| {
            CliError::new(
                ErrorCode::InputInvalid,
                "query file is empty; provide GraphQL query text",
            )
        });
    }

    Err(CliError::new(
        ErrorCode::InputInvalid,
        "either --query or --query-file is required",
    ))
}

fn resolve_graphql_variables(command: &cli::GraphqlRunArgs) -> Result<Value, CliError> {
    let raw = if let Some(path) = &command.variables_file {
        fs::read_to_string(path).map_err(|error| {
            CliError::new(
                ErrorCode::TransportError,
                format!("failed to read variables file {}: {error}", path.display()),
            )
        })?
    } else {
        command
            .variables
            .clone()
            .unwrap_or_else(|| "{}".to_string())
    };

    let parsed = serde_json::from_str::<Value>(&raw).map_err(|error| {
        CliError::new(
            ErrorCode::InputInvalid,
            format!("variables must be valid JSON object: {error}"),
        )
    })?;

    if !parsed.is_object() {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "variables must be a JSON object",
        ));
    }
    Ok(parsed)
}

fn build_graphql_guardrails(command: &cli::GraphqlRunArgs) -> Result<GraphqlGuardrails, CliError> {
    if command.timeout_secs == 0 || command.timeout_secs > 60 {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "timeout-secs must be in range 1..=60",
        ));
    }
    if command.response_limit_mib == 0 || command.response_limit_mib > 8 {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "response-limit-mib must be in range 1..=8",
        ));
    }
    if command.max_depth == 0 {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "max-depth must be greater than 0",
        ));
    }
    if command.max_complexity == 0 {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "max-complexity must be greater than 0",
        ));
    }

    let mib = usize::try_from(command.response_limit_mib).map_err(|_| {
        CliError::new(
            ErrorCode::InputInvalid,
            "response-limit-mib is out of supported range",
        )
    })?;
    let response_limit_bytes = mib.checked_mul(1024 * 1024).ok_or_else(|| {
        CliError::new(
            ErrorCode::InputInvalid,
            "response-limit-mib overflowed byte conversion",
        )
    })?;

    Ok(GraphqlGuardrails {
        timeout_secs: command.timeout_secs,
        response_limit_bytes,
        max_depth: command.max_depth,
        max_complexity: command.max_complexity,
        allow_mutation: command.allow_mutation,
        unsafe_no_cost_check: command.unsafe_no_cost_check,
    })
}

fn enforce_graphql_guardrails(
    query: &str,
    variables: &Value,
    guardrails: GraphqlGuardrails,
) -> Result<(), CliError> {
    if !variables.is_object() {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "variables must be a JSON object",
        ));
    }

    if detect_graphql_operation_kind(query) == Some(GraphqlOperationKind::Mutation) {
        if !guardrails.allow_mutation {
            return Err(CliError::new(
                ErrorCode::InputInvalid,
                "mutation is blocked in graphql run mode; pass --allow-mutation to execute",
            ));
        }
        enforce_mutation_allowlist(query)?;
    }

    match analyze_query_shape(query) {
        Ok(shape) => {
            if shape.max_depth > guardrails.max_depth {
                return Err(CliError::new(
                    ErrorCode::InputInvalid,
                    format!(
                        "query depth {} exceeds max-depth {}",
                        shape.max_depth, guardrails.max_depth
                    ),
                ));
            }
            if shape.complexity > guardrails.max_complexity {
                return Err(CliError::new(
                    ErrorCode::InputInvalid,
                    format!(
                        "query complexity {} exceeds max-complexity {}",
                        shape.complexity, guardrails.max_complexity
                    ),
                ));
            }
        }
        Err(error) => {
            if !guardrails.unsafe_no_cost_check {
                return Err(CliError::new(
                    ErrorCode::InputInvalid,
                    format!(
                        "query shape analysis failed: {error}; rerun with --unsafe-no-cost-check to bypass"
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn enforce_mutation_allowlist(query: &str) -> Result<(), CliError> {
    let mutation_root_fields = extract_mutation_root_fields(query)
        .map_err(|error| CliError::new(ErrorCode::InputInvalid, error))?;
    let allowed_roots = trusted_mutation_root_fields();
    let mut blocked_roots = mutation_root_fields
        .iter()
        .filter(|field| !allowed_roots.contains(field.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    if blocked_roots.is_empty() {
        return Ok(());
    }

    blocked_roots.sort();
    blocked_roots.dedup();

    let mut allowed_sorted = allowed_roots.iter().copied().collect::<Vec<_>>();
    allowed_sorted.sort_unstable();
    Err(CliError::new(
        ErrorCode::InputInvalid,
        format!(
            "mutation root field(s) are not allowlisted for graphql run: {}; allowed fields: {}",
            blocked_roots.join(", "),
            allowed_sorted.join(", ")
        ),
    ))
}

fn trusted_mutation_root_fields() -> HashSet<&'static str> {
    resource_contracts()
        .iter()
        .filter(|contract| contract.kind == "mutation")
        .filter_map(|contract| {
            contract
                .graphql_file
                .strip_prefix("endpoint:mutation.")
                .filter(|field| !field.is_empty())
        })
        .collect()
}

fn extract_mutation_root_fields(query: &str) -> Result<Vec<String>, String> {
    let bytes = query.as_bytes();
    let mut index = 0usize;
    let mut depth = 0u32;
    let mut paren_depth = 0u32;
    let mut in_comment = false;
    let mut in_string = false;
    let mut escaped = false;
    let mut roots = Vec::new();

    while index < bytes.len() {
        let c = bytes[index];
        if in_comment {
            if c == b'\n' {
                in_comment = false;
            }
            index += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match c {
            b'#' => {
                in_comment = true;
                index += 1;
            }
            b'.' if depth == 1
                && paren_depth == 0
                && index + 2 < bytes.len()
                && bytes[index + 1] == b'.'
                && bytes[index + 2] == b'.' =>
            {
                return Err(
                    "fragment syntax is not supported in mutation root selection set".to_string(),
                );
            }
            b'"' => {
                in_string = true;
                index += 1;
            }
            b'(' => {
                paren_depth = paren_depth.saturating_add(1);
                index += 1;
            }
            b')' => {
                paren_depth = paren_depth.saturating_sub(1);
                index += 1;
            }
            b'{' => {
                depth = depth.saturating_add(1);
                index += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            _ if depth == 1 && paren_depth == 0 && is_identifier_start(c) => {
                let field_or_alias = read_identifier_token(bytes, &mut index)?;
                skip_graphql_whitespace_and_comments(bytes, &mut index);
                let resolved_field = if index < bytes.len() && bytes[index] == b':' {
                    index += 1;
                    skip_graphql_whitespace_and_comments(bytes, &mut index);
                    if index >= bytes.len() || !is_identifier_start(bytes[index]) {
                        return Err("invalid alias syntax in mutation selection set".to_string());
                    }
                    read_identifier_token(bytes, &mut index)?
                } else {
                    field_or_alias
                };
                if !is_graphql_keyword(&resolved_field) {
                    roots.push(resolved_field);
                }
            }
            _ => {
                index += 1;
            }
        }
    }

    if roots.is_empty() {
        return Err("mutation must include at least one top-level root field".to_string());
    }
    Ok(roots)
}

fn read_identifier_token(bytes: &[u8], index: &mut usize) -> Result<String, String> {
    let start = *index;
    while *index < bytes.len() && is_identifier_continue(bytes[*index]) {
        *index += 1;
    }
    std::str::from_utf8(&bytes[start..*index])
        .map(str::to_string)
        .map_err(|_| "query contains non-utf8 identifier token".to_string())
}

fn skip_graphql_whitespace_and_comments(bytes: &[u8], index: &mut usize) {
    while *index < bytes.len() {
        if bytes[*index].is_ascii_whitespace() {
            *index += 1;
            continue;
        }
        if bytes[*index] == b'#' {
            *index += 1;
            while *index < bytes.len() && bytes[*index] != b'\n' {
                *index += 1;
            }
            continue;
        }
        break;
    }
}

fn detect_graphql_operation_kind(query: &str) -> Option<GraphqlOperationKind> {
    let bytes = query.as_bytes();
    let mut index = 0usize;
    skip_graphql_whitespace_and_comments(bytes, &mut index);
    if index >= bytes.len() {
        return None;
    }

    if bytes[index] == b'{' {
        return Some(GraphqlOperationKind::Query);
    }
    if !is_identifier_start(bytes[index]) {
        return None;
    }

    let operation = read_identifier_token(bytes, &mut index).ok()?;
    match operation.as_str() {
        "mutation" => Some(GraphqlOperationKind::Mutation),
        "query" | "subscription" => Some(GraphqlOperationKind::Query),
        _ => None,
    }
}

fn analyze_query_shape(query: &str) -> Result<QueryShape, String> {
    let bytes = query.as_bytes();
    let mut index = 0usize;
    let mut depth = 0u32;
    let mut max_depth = 0u32;
    let mut complexity = 0u32;
    let mut paren_depth = 0u32;
    let mut in_comment = false;
    let mut in_string = false;
    let mut escaped = false;
    let mut has_selection = false;

    while index < bytes.len() {
        let c = bytes[index];

        if in_comment {
            if c == b'\n' {
                in_comment = false;
            }
            index += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match c {
            b'#' => {
                in_comment = true;
                index += 1;
            }
            b'"' => {
                in_string = true;
                index += 1;
            }
            b'(' => {
                paren_depth = paren_depth.saturating_add(1);
                index += 1;
            }
            b')' => {
                paren_depth = paren_depth.saturating_sub(1);
                index += 1;
            }
            b'{' => {
                depth = depth.saturating_add(1);
                has_selection = true;
                if depth > max_depth {
                    max_depth = depth;
                }
                index += 1;
            }
            b'}' => {
                if depth == 0 {
                    return Err("query has unmatched closing brace".to_string());
                }
                depth -= 1;
                index += 1;
            }
            _ if is_identifier_start(c) => {
                let start = index;
                index += 1;
                while index < bytes.len() && is_identifier_continue(bytes[index]) {
                    index += 1;
                }
                if depth > 0 && paren_depth == 0 {
                    let token = std::str::from_utf8(&bytes[start..index])
                        .map_err(|_| "query contains non-utf8 token".to_string())?;
                    if !is_graphql_keyword(token) {
                        complexity = complexity.saturating_add(1);
                    }
                }
            }
            _ => {
                index += 1;
            }
        }
    }

    if depth != 0 {
        return Err("query has unmatched opening brace".to_string());
    }
    if !has_selection {
        return Err("query must include a selection set".to_string());
    }
    if complexity == 0 {
        return Err("query complexity is zero (no selectable fields found)".to_string());
    }

    Ok(QueryShape {
        max_depth,
        complexity,
    })
}

fn is_identifier_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_identifier_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn is_graphql_keyword(token: &str) -> bool {
    matches!(
        token,
        "query" | "mutation" | "subscription" | "fragment" | "on" | "true" | "false" | "null"
    )
}

fn execute_version(_command: &cli::VersionArgs) -> CommandOutput {
    let version = env!("CARGO_PKG_VERSION");
    CommandOutput {
        data: json!({ "version": version }),
        message: version.to_string(),
    }
}

fn resolve_client_context(
    cli: &cli::Cli,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> Result<ClientContext, CliError> {
    let (_, config) = load_config(cli.config_path.clone())?;
    let requested_team = requested_team_from_cli(cli);
    let requested_origin = requested_origin_from_cli(cli);

    let resolved = resolve_access_token(
        &ResolveTokenInput {
            requested_team: requested_team.clone(),
            requested_origin: requested_origin.clone(),
            stdin_token,
            env_token,
        },
        &config,
        &KeychainTokenStore::default(),
    )?
    .ok_or_else(|| {
        CliError::new(
            ErrorCode::AuthFailed,
            "no access token found (stdin/env/keychain/config)",
        )
    })?;

    let team = resolved
        .team
        .clone()
        .or_else(|| config.resolve_team(requested_team.as_deref()));
    let origin = config
        .resolve_origin(requested_origin.as_deref(), team.as_deref())
        .ok_or_else(|| {
            CliError::new(
                ErrorCode::InputInvalid,
                "origin is required (--origin/KIBELA_ORIGIN or profile origin)",
            )
        })?;
    let token_source = token_source_label(resolved.source).to_string();
    let client = KibelClient::new(origin, resolved.token)?;

    Ok(ClientContext {
        team,
        token_source,
        client,
    })
}

fn context_meta(ctx: &ClientContext) -> Value {
    json!({
        "team": ctx.team,
        "origin": ctx.client.origin(),
        "token_source": ctx.token_source,
    })
}

fn kibela_access_token_settings_url(origin: &str) -> String {
    let base = origin.trim_end_matches('/');
    format!("{base}/settings/access_tokens")
}

fn requested_team_from_cli(cli: &cli::Cli) -> Option<String> {
    cli.team.as_deref().and_then(normalize_owned).or_else(|| {
        std::env::var("KIBELA_TENANT")
            .ok()
            .and_then(|v| normalize_owned(&v))
    })
}

fn requested_origin_from_cli(cli: &cli::Cli) -> Option<String> {
    normalize_origin_owned(&cli.origin).or_else(|| {
        std::env::var("KIBELA_TENANT_ORIGIN")
            .ok()
            .and_then(|v| normalize_origin_owned(&v))
    })
}

fn resolve_login_origin(
    requested_origin: Option<&str>,
    requested_team: Option<&str>,
    config: &Config,
    interactive: bool,
) -> Result<String, CliError> {
    if let Some(origin) = requested_origin.and_then(normalize_origin_owned) {
        return Ok(origin);
    }

    if let Some(origin) = config.resolve_origin(None, requested_team) {
        if let Some(normalized) = normalize_origin_owned(&origin) {
            return Ok(normalized);
        }
    }

    if interactive {
        return prompt_origin_input();
    }

    Err(CliError::new(
        ErrorCode::InputInvalid,
        "origin is required (--origin/KIBELA_ORIGIN or profile origin)",
    ))
}

fn resolve_login_team(
    requested_team: Option<&str>,
    resolved_origin: Option<&str>,
    config: &Config,
    interactive: bool,
) -> Result<String, CliError> {
    if let Some(team) = requested_team.and_then(normalize_owned) {
        return Ok(team);
    }

    if let Some(origin) = resolved_origin {
        if let Some(inferred_team) = infer_team_from_origin(origin) {
            return Ok(inferred_team);
        }
    }

    if let Some(team) = config.resolve_team(None) {
        if let Some(normalized) = normalize_owned(&team) {
            return Ok(normalized);
        }
    }

    if interactive {
        return prompt_text_input("Kibela team (tenant)");
    }

    Err(CliError::new(
        ErrorCode::InputInvalid,
        "team is required (--team/KIBELA_TEAM/KIBELA_TENANT or config.default_team)",
    ))
}

fn resolve_login_token(
    with_token: bool,
    stdin_token: Option<&str>,
    env_token: Option<&str>,
    interactive: bool,
) -> Result<(String, &'static str), CliError> {
    if let Some(token) = stdin_token.and_then(normalize_owned) {
        return Ok((token, "stdin"));
    }
    if let Some(token) = env_token.and_then(normalize_owned) {
        return Ok((token, "env"));
    }
    if with_token {
        return Err(CliError::new(
            ErrorCode::InputInvalid,
            "stdin token is empty",
        ));
    }
    if interactive {
        let token = prompt_secret_input("Kibela access token")?;
        return Ok((token, "prompt"));
    }
    Err(CliError::new(
        ErrorCode::InputInvalid,
        "token is required (--with-token stdin or token env)",
    ))
}

fn token_store_lookup_subjects(team: &str, origin: Option<&str>) -> Vec<String> {
    let mut subjects = Vec::new();
    if let Some(origin) = origin.and_then(normalize_origin_owned) {
        subjects.push(token_store_subject(team, Some(&origin)));
    }
    if !subjects.iter().any(|subject| subject == team) {
        subjects.push(team.to_string());
    }
    subjects
}

fn prompt_text_input(label: &str) -> Result<String, CliError> {
    let mut stdout = io::stdout();
    write!(stdout, "{label}: ").map_err(|err| {
        CliError::new(
            ErrorCode::TransportError,
            format!("failed to write prompt: {err}"),
        )
    })?;
    stdout.flush().map_err(|err| {
        CliError::new(
            ErrorCode::TransportError,
            format!("failed to flush prompt: {err}"),
        )
    })?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|err| {
        CliError::new(
            ErrorCode::TransportError,
            format!("failed to read prompt input: {err}"),
        )
    })?;

    normalize_owned(&input)
        .ok_or_else(|| CliError::new(ErrorCode::InputInvalid, format!("{label} is empty")))
}

fn prompt_secret_input(label: &str) -> Result<String, CliError> {
    let value = prompt_password(format!("{label}: ")).map_err(|err| {
        CliError::new(
            ErrorCode::TransportError,
            format!("failed to read secret input: {err}"),
        )
    })?;
    normalize_owned(&value)
        .ok_or_else(|| CliError::new(ErrorCode::InputInvalid, format!("{label} is empty")))
}

fn prompt_origin_input() -> Result<String, CliError> {
    let origin = prompt_text_input("Kibela origin (https://<tenant>.kibe.la)")?;
    normalize_origin_owned(&origin).ok_or_else(|| {
        CliError::new(
            ErrorCode::InputInvalid,
            "origin must start with http:// or https://",
        )
    })
}

fn is_interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn note_folder_arg_to_input(folder: &cli::NoteFolderArg) -> CreateNoteFolderInput {
    CreateNoteFolderInput {
        group_id: folder.group_id.clone(),
        folder_name: folder.folder_name.clone(),
    }
}

fn load_config(config_path: Option<PathBuf>) -> Result<(PathBuf, Config), CliError> {
    let config_path = match config_path {
        Some(path) => path,
        None => default_config_path()?,
    };

    let config = Config::load(&config_path)?;
    Ok((config_path, config))
}

fn read_stdin_token(is_enabled: bool) -> Result<Option<String>, CliError> {
    if !is_enabled {
        return Ok(None);
    }

    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer).map_err(|err| {
        CliError::new(
            ErrorCode::TransportError,
            format!("failed to read token from stdin: {err}"),
        )
    })?;

    normalize_owned(&buffer)
        .map(Some)
        .ok_or_else(|| CliError::new(ErrorCode::InputInvalid, "stdin token is empty"))
}

fn normalize_owned(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_origin_owned(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.trim_end_matches('/');
    if normalized.is_empty() {
        return None;
    }
    if !(normalized.starts_with("https://") || normalized.starts_with("http://")) {
        return None;
    }
    Some(normalized.to_ascii_lowercase())
}

fn infer_team_from_origin(origin: &str) -> Option<String> {
    let normalized = normalize_origin_owned(origin)?;
    let host_and_path = normalized.split_once("://")?.1;
    let host = host_and_path.split('/').next()?;
    let host = host.split(':').next()?;
    let tenant = host.strip_suffix(".kibe.la")?;
    if tenant.is_empty() {
        None
    } else {
        Some(tenant.to_string())
    }
}

fn generated_request_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let bytes = now.to_le_bytes();
    let lower_32_bits = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    format!("req-{lower_32_bits:08x}")
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_query_shape, build_graphql_guardrails, detect_graphql_operation_kind,
        enforce_graphql_guardrails, extract_mutation_root_fields, infer_team_from_origin,
        kibela_access_token_settings_url, normalize_origin_owned, resolve_graphql_variables,
        search_note_mine_has_unsupported_filters, token_store_lookup_subjects,
        trusted_mutation_root_fields, GraphqlGuardrails, GraphqlOperationKind,
    };
    use crate::cli;
    use serde_json::json;

    fn graphql_run_args(query: &str) -> cli::GraphqlRunArgs {
        cli::GraphqlRunArgs {
            query: Some(query.to_string()),
            query_file: None,
            variables: Some("{}".to_string()),
            variables_file: None,
            timeout_secs: 15,
            response_limit_mib: 2,
            max_depth: 8,
            max_complexity: 1000,
            allow_mutation: false,
            unsafe_no_cost_check: false,
        }
    }

    #[test]
    fn detect_operation_kind_handles_query_and_mutation() {
        assert_eq!(
            detect_graphql_operation_kind("query Q { groups { edges { node { id } } } }"),
            Some(GraphqlOperationKind::Query)
        );
        assert_eq!(
            detect_graphql_operation_kind(
                "mutation M { createFolder(input: {}) { folder { id } } }"
            ),
            Some(GraphqlOperationKind::Mutation)
        );
    }

    #[test]
    fn detect_operation_kind_skips_leading_graphql_comments() {
        assert_eq!(
            detect_graphql_operation_kind(
                "# generated by agent\n   # keep for tracing\nmutation M { createFolder(input: {}) { folder { id } } }"
            ),
            Some(GraphqlOperationKind::Mutation)
        );
    }

    #[test]
    fn analyze_query_shape_computes_depth_and_complexity() {
        let shape = analyze_query_shape("query Q { groups { edges { node { id } } } }")
            .expect("shape analysis should succeed");
        assert!(shape.max_depth >= 4);
        assert!(shape.complexity >= 4);
    }

    #[test]
    fn build_graphql_guardrails_rejects_invalid_ranges() {
        let mut args = graphql_run_args("query Q { groups { edges { node { id } } } }");
        args.timeout_secs = 0;
        assert!(build_graphql_guardrails(&args).is_err());
        args.timeout_secs = 15;
        args.response_limit_mib = 9;
        assert!(build_graphql_guardrails(&args).is_err());
    }

    #[test]
    fn resolve_graphql_variables_requires_object() {
        let mut args = graphql_run_args("query Q { groups { edges { node { id } } } }");
        args.variables = Some("[1,2,3]".to_string());
        assert!(resolve_graphql_variables(&args).is_err());
    }

    #[test]
    fn enforce_graphql_guardrails_blocks_mutation_without_opt_in() {
        let guardrails = GraphqlGuardrails {
            timeout_secs: 15,
            response_limit_bytes: 2 * 1024 * 1024,
            max_depth: 8,
            max_complexity: 1000,
            allow_mutation: false,
            unsafe_no_cost_check: false,
        };
        let result = enforce_graphql_guardrails(
            "mutation M($input: CreateFolderInput!) { createFolder(input: $input) { folder { id } } }",
            &json!({ "input": { "folder": { "groupId": "G1", "folderName": "Engineering" } } }),
            guardrails,
        );
        assert!(result.is_err());
    }

    #[test]
    fn extract_mutation_root_fields_supports_alias() {
        let fields = extract_mutation_root_fields(
            "mutation M($input: CreateFolderInput!) { aliasCreate: createFolder(input: $input) { folder { id } } }",
        )
        .expect("alias mutation should parse");
        assert_eq!(fields, vec!["createFolder".to_string()]);
    }

    #[test]
    fn extract_mutation_root_fields_rejects_fragment_spread() {
        let result = extract_mutation_root_fields(
            "mutation M { ...DangerousFragment createFolder(input: {}) { folder { id } } }",
        );
        assert!(result
            .expect_err("fragment spread should be rejected")
            .contains("fragment syntax is not supported"),);
    }

    #[test]
    fn enforce_graphql_guardrails_blocks_untrusted_mutation_field() {
        let guardrails = GraphqlGuardrails {
            timeout_secs: 15,
            response_limit_bytes: 2 * 1024 * 1024,
            max_depth: 8,
            max_complexity: 1000,
            allow_mutation: true,
            unsafe_no_cost_check: false,
        };
        let result = enforce_graphql_guardrails(
            "mutation Dangerous($id: ID!) { deleteNote(input: { id: $id }) { clientMutationId } }",
            &json!({ "id": "N1" }),
            guardrails,
        );
        let error = result.expect_err("untrusted mutation root should be blocked");
        assert!(
            error
                .message
                .contains("not allowlisted for graphql run: deleteNote"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn trusted_mutation_root_fields_include_create_folder() {
        let allowed = trusted_mutation_root_fields();
        assert!(allowed.contains("createFolder"));
    }

    #[test]
    fn mine_search_accepts_default_shape_only() {
        let command = cli::SearchNoteArgs {
            query: String::new(),
            resources: vec![],
            coediting: None,
            updated: None,
            group_ids: vec![],
            user_ids: vec![],
            mine: true,
            folder_ids: vec![],
            liker_ids: vec![],
            is_archived: None,
            sort_by: None,
            first: Some(10),
        };
        assert!(!search_note_mine_has_unsupported_filters(&command));
    }

    #[test]
    fn mine_search_rejects_additional_filters() {
        let command = cli::SearchNoteArgs {
            query: String::new(),
            resources: vec!["note".to_string()],
            coediting: None,
            updated: None,
            group_ids: vec![],
            user_ids: vec![],
            mine: true,
            folder_ids: vec![],
            liker_ids: vec![],
            is_archived: None,
            sort_by: None,
            first: Some(10),
        };
        assert!(search_note_mine_has_unsupported_filters(&command));
    }

    #[test]
    fn infer_team_from_kibela_origin() {
        assert_eq!(
            infer_team_from_origin("https://example-team.kibe.la/"),
            Some("example-team".to_string())
        );
        assert_eq!(infer_team_from_origin("https://example.com"), None);
    }

    #[test]
    fn normalize_origin_requires_scheme_and_removes_trailing_slash() {
        assert_eq!(
            normalize_origin_owned("https://EXAMPLE-TEAM.kibe.la/"),
            Some("https://example-team.kibe.la".to_string())
        );
        assert_eq!(normalize_origin_owned("example-team.kibe.la"), None);
    }

    #[test]
    fn token_store_lookup_subjects_include_origin_specific_and_legacy_team() {
        let subjects =
            token_store_lookup_subjects("example-team", Some("https://example-team.kibe.la"));
        assert_eq!(
            subjects,
            vec![
                "origin::https://example-team.kibe.la::team::example-team".to_string(),
                "example-team".to_string(),
            ]
        );
    }

    #[test]
    fn token_settings_url_uses_tenant_origin() {
        assert_eq!(
            kibela_access_token_settings_url("https://example-team.kibe.la"),
            "https://example-team.kibe.la/settings/access_tokens".to_string()
        );
        assert_eq!(
            kibela_access_token_settings_url("https://example-team.kibe.la/"),
            "https://example-team.kibe.la/settings/access_tokens".to_string()
        );
    }
}
