mod cli;
mod error;

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use error::{CliError, ErrorCode};
use kibel_client::{
    default_config_path, require_team, resolve_access_token, token_source_label,
    AttachNoteToFolderInput, Config, CreateCommentInput, CreateCommentReplyInput,
    CreateFolderInput, CreateNoteFolderInput, CreateNoteInput, FeedSectionsInput,
    FolderLookupInput, GetNotesInput, KeychainTokenStore, KibelClient,
    MoveNoteToAnotherFolderInput, PageInput, PathLookupInput, ResolveTokenInput, SearchFolderInput,
    SearchNoteInput, TokenStore, UpdateNoteInput,
};
use serde_json::{json, Value};
use std::io::{self, Read};
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
        | cli::Command::Note(_) => true,
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
            let team = require_team(command.team.as_deref().or(cli.team.as_deref()), &config)?;

            let Some(token) = stdin_token
                .as_deref()
                .and_then(normalize_owned)
                .or_else(|| env_token.as_deref().and_then(normalize_owned))
            else {
                return Err(CliError::new(
                    ErrorCode::InputInvalid,
                    "token is required (--with-token stdin or token env)",
                ));
            };

            let token_source = if cli.with_token { "stdin" } else { "env" };
            let store = KeychainTokenStore::default();
            store.set_token(&team, &token)?;
            config.set_profile_token(&team, &token);
            if let Some(origin) = normalize_owned(&cli.origin) {
                config.set_profile_origin(&team, &origin);
            }
            config.set_default_team_if_missing(&team);
            config.save(&config_path)?;

            Ok(CommandOutput {
                data: json!({
                    "team": team,
                    "token_source": token_source,
                    "stored_in": ["keychain", "config"],
                    "config_path": config_path,
                }),
                message: "auth login completed".to_string(),
            })
        }
        cli::AuthCommand::Logout(command) => {
            let (config_path, mut config) = load_config(cli.config_path.clone())?;
            let team = require_team(command.team.as_deref().or(cli.team.as_deref()), &config)?;

            let store = KeychainTokenStore::default();
            store.delete_token(&team)?;
            let config_token_removed = config.clear_profile_token(&team);
            config.save(&config_path)?;

            Ok(CommandOutput {
                data: json!({
                    "team": team,
                    "keychain_deleted": true,
                    "config_token_removed": config_token_removed,
                    "config_path": config_path,
                }),
                message: "auth logout completed".to_string(),
            })
        }
        cli::AuthCommand::Status(command) => {
            let (_, config) = load_config(cli.config_path.clone())?;
            let requested_team = command.team.clone().or_else(|| cli.team.clone());
            let resolved = resolve_access_token(
                &ResolveTokenInput {
                    requested_team,
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
            let results = ctx.client.search_note(&SearchNoteInput {
                query: command.query.clone(),
                resources: command.resources.clone(),
                coediting: command.coediting,
                updated: command.updated.clone(),
                group_ids: command.group_ids.clone(),
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
    let requested_origin = normalize_owned(&cli.origin);

    let resolved = resolve_access_token(
        &ResolveTokenInput {
            requested_team: cli.team.clone(),
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
        .or_else(|| config.resolve_team(cli.team.as_deref()));
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

fn generated_request_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let bytes = now.to_le_bytes();
    let lower_32_bits = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    format!("req-{lower_32_bits:08x}")
}
