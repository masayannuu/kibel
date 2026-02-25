use clap::{ArgAction, Args, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(name = "kibel", about = "Kibela CLI", version)]
pub struct Cli {
    #[arg(long, global = true, action = ArgAction::SetTrue, help = "Output machine-readable JSON")]
    pub json: bool,
    #[arg(long, global = true, action = ArgAction::SetTrue, help = "Read access token from stdin")]
    pub with_token: bool,
    #[arg(
        long,
        global = true,
        default_value = "KIBELA_ACCESS_TOKEN",
        help = "Token env var name"
    )]
    pub token_env: String,
    #[arg(
        long,
        global = true,
        env = "KIBELA_ORIGIN",
        default_value = "",
        help = "Kibela origin URL"
    )]
    pub origin: String,
    #[arg(
        long,
        visible_alias = "tenant",
        global = true,
        env = "KIBELA_TEAM",
        help = "Team name (tenant); env alias KIBELA_TENANT is also supported"
    )]
    pub team: Option<String>,
    #[arg(long, global = true, value_name = "PATH", help = "Config file path")]
    pub config_path: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Auth(AuthArgs),
    Config(ConfigArgs),
    Search(SearchArgs),
    Group(GroupArgs),
    Folder(FolderArgs),
    Feed(FeedArgs),
    Comment(CommentArgs),
    Note(NoteArgs),
    Graphql(GraphqlArgs),
    Completion(CompletionArgs),
    Version(VersionArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommand {
    Login(AuthLoginArgs),
    Logout(AuthLogoutArgs),
    Status(AuthStatusArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AuthLoginArgs {
    #[arg(long, visible_alias = "tenant", help = "Team name (tenant)")]
    pub team: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct AuthLogoutArgs {
    #[arg(long, help = "Team name")]
    pub team: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct AuthStatusArgs {
    #[arg(long, help = "Team name")]
    pub team: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    Set(ConfigSetArgs),
    Profiles(ConfigProfilesArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ConfigSetArgs {
    #[command(subcommand)]
    pub command: ConfigSetCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigSetCommand {
    Team(ConfigSetTeamArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ConfigSetTeamArgs {
    pub team: String,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigProfilesArgs {}

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    #[command(subcommand)]
    pub command: SearchCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SearchCommand {
    Note(SearchNoteArgs),
    Folder(SearchFolderArgs),
    User(SearchUserArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SearchNoteArgs {
    #[arg(long, default_value = "")]
    pub query: String,
    #[arg(long)]
    pub after: Option<String>,
    #[arg(long = "resource")]
    pub resources: Vec<String>,
    #[arg(long)]
    pub coediting: Option<bool>,
    #[arg(long)]
    pub updated: Option<String>,
    #[arg(long = "group-id")]
    pub group_ids: Vec<String>,
    #[arg(long = "user-id")]
    pub user_ids: Vec<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub mine: bool,
    #[arg(long = "folder-id")]
    pub folder_ids: Vec<String>,
    #[arg(long = "liker-id")]
    pub liker_ids: Vec<String>,
    #[arg(long = "is-archived")]
    pub is_archived: Option<bool>,
    #[arg(long = "sort-by")]
    pub sort_by: Option<String>,
    #[arg(long)]
    pub first: Option<u32>,
    #[arg(long)]
    pub preset: Option<String>,
    #[arg(long = "save-preset")]
    pub save_preset: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct SearchFolderArgs {
    #[arg(long)]
    pub query: String,
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct SearchUserArgs {
    #[arg(long, default_value = "")]
    pub query: String,
    #[arg(long)]
    pub first: Option<u32>,
    #[arg(long = "group-id")]
    pub group_ids: Vec<String>,
    #[arg(long = "folder-id")]
    pub folder_ids: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct GroupArgs {
    #[command(subcommand)]
    pub command: GroupCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum GroupCommand {
    List(GroupListArgs),
}

#[derive(Debug, Clone, Args)]
pub struct GroupListArgs {
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct FolderArgs {
    #[command(subcommand)]
    pub command: FolderCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum FolderCommand {
    List(FolderListArgs),
    Get(FolderGetArgs),
    GetFromPath(FolderGetFromPathArgs),
    Notes(FolderNotesArgs),
    Create(FolderCreateArgs),
}

#[derive(Debug, Clone, Args)]
pub struct FolderListArgs {
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct FolderGetArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct FolderGetFromPathArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct FolderNotesArgs {
    #[arg(long = "folder-id")]
    pub folder_id: String,
    #[arg(long)]
    pub first: Option<u32>,
    #[arg(long)]
    pub last: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct FolderCreateArgs {
    #[arg(long = "group-id")]
    pub group_id: String,
    #[arg(long = "full-name")]
    pub full_name: String,
}

#[derive(Debug, Clone, Args)]
pub struct FeedArgs {
    #[command(subcommand)]
    pub command: FeedCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum FeedCommand {
    Sections(FeedSectionsArgs),
}

#[derive(Debug, Clone, Args)]
pub struct FeedSectionsArgs {
    #[arg(long)]
    pub kind: String,
    #[arg(long = "group-id")]
    pub group_id: String,
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct CommentArgs {
    #[command(subcommand)]
    pub command: CommentCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum CommentCommand {
    Create(CommentCreateArgs),
    Reply(CommentReplyArgs),
}

#[derive(Debug, Clone, Args)]
pub struct CommentCreateArgs {
    #[arg(long)]
    pub content: String,
    #[arg(long = "note-id")]
    pub note_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct CommentReplyArgs {
    #[arg(long)]
    pub content: String,
    #[arg(long = "comment-id")]
    pub comment_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct NoteArgs {
    #[command(subcommand)]
    pub command: NoteCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum NoteCommand {
    Create(NoteCreateArgs),
    Get(NoteGetArgs),
    GetMany(NoteGetManyArgs),
    GetFromPath(NoteGetFromPathArgs),
    Update(NoteUpdateArgs),
    MoveToFolder(NoteMoveToFolderArgs),
    AttachToFolder(NoteAttachToFolderArgs),
}

#[derive(Debug, Clone, Args)]
pub struct NoteCreateArgs {
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub content: String,
    #[arg(long = "group-id")]
    pub group_ids: Vec<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub draft: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub coediting: bool,
    #[arg(long = "folder", value_parser = parse_folder_arg)]
    pub folders: Vec<NoteFolderArg>,
    #[arg(long = "author-id")]
    pub author_id: Option<String>,
    #[arg(long = "published-at")]
    pub published_at: Option<String>,
    #[arg(long = "client-mutation-id")]
    pub client_mutation_id: Option<String>,
    #[arg(long = "idempotency-key", hide = true)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFolderArg {
    pub group_id: String,
    pub folder_name: String,
}

#[derive(Debug, Clone, Args)]
pub struct NoteGetArgs {
    #[arg(long)]
    pub id: String,
}

#[derive(Debug, Clone, Args)]
pub struct NoteGetManyArgs {
    #[arg(long = "id", required = true)]
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct NoteGetFromPathArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct NoteUpdateArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long = "base-content")]
    pub base_content: String,
    #[arg(long = "new-content")]
    pub new_content: String,
}

#[derive(Debug, Clone, Args)]
pub struct NoteMoveToFolderArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long = "from-folder", value_parser = parse_folder_arg)]
    pub from_folder: NoteFolderArg,
    #[arg(long = "to-folder", value_parser = parse_folder_arg)]
    pub to_folder: NoteFolderArg,
}

#[derive(Debug, Clone, Args)]
pub struct NoteAttachToFolderArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long = "folder", value_parser = parse_folder_arg)]
    pub folder: NoteFolderArg,
}

#[derive(Debug, Clone, Args)]
pub struct GraphqlArgs {
    #[command(subcommand)]
    pub command: GraphqlCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum GraphqlCommand {
    Run(GraphqlRunArgs),
}

#[derive(Debug, Clone, Args)]
pub struct GraphqlRunArgs {
    #[arg(long, conflicts_with = "query_file", help = "GraphQL query text")]
    pub query: Option<String>,
    #[arg(
        long = "query-file",
        value_name = "PATH",
        conflicts_with = "query",
        help = "Path to a file containing GraphQL query text"
    )]
    pub query_file: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with = "variables_file",
        help = "JSON object for GraphQL variables"
    )]
    pub variables: Option<String>,
    #[arg(
        long = "variables-file",
        value_name = "PATH",
        conflicts_with = "variables",
        help = "Path to a file containing GraphQL variables JSON object"
    )]
    pub variables_file: Option<PathBuf>,
    #[arg(long, default_value_t = 15, help = "Request timeout (seconds)")]
    pub timeout_secs: u64,
    #[arg(
        long = "response-limit-mib",
        default_value_t = 2,
        help = "Response size limit (MiB)"
    )]
    pub response_limit_mib: u64,
    #[arg(long = "max-depth", default_value_t = 8, help = "Maximum query depth")]
    pub max_depth: u32,
    #[arg(
        long = "max-complexity",
        default_value_t = 1000,
        help = "Maximum static complexity score"
    )]
    pub max_complexity: u32,
    #[arg(
        long = "allow-mutation",
        action = ArgAction::SetTrue,
        help = "Allow mutation execution in graphql run mode"
    )]
    pub allow_mutation: bool,
    #[arg(
        long = "unsafe-no-cost-check",
        action = ArgAction::SetTrue,
        help = "Allow execution when query shape analysis fails"
    )]
    pub unsafe_no_cost_check: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CompletionArgs {
    pub shell: Shell,
}

#[derive(Debug, Clone, Args)]
pub struct VersionArgs {
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

fn parse_folder_arg(raw: &str) -> Result<NoteFolderArg, String> {
    let value = raw.trim();
    let (group_id, folder_name) = value
        .split_once(':')
        .ok_or_else(|| "folder must be `GROUP_ID:FOLDER_NAME`".to_string())?;
    let group_id = group_id.trim();
    let folder_name = folder_name.trim();
    if group_id.is_empty() || folder_name.is_empty() {
        return Err("folder must contain non-empty group id and folder name".to_string());
    }

    Ok(NoteFolderArg {
        group_id: group_id.to_string(),
        folder_name: folder_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        parse_folder_arg, AuthCommand, Cli, Command, ConfigCommand, ConfigSetCommand,
        GraphqlCommand, GroupCommand, NoteCommand, SearchCommand,
    };
    use clap::Parser;

    #[test]
    fn parse_auth_login_with_stdin_flag() {
        let cli = Cli::try_parse_from(["kibel", "auth", "login", "--team", "acme", "--with-token"])
            .expect("parse should succeed");

        assert!(cli.with_token);
        match cli.command {
            Command::Auth(args) => match args.command {
                AuthCommand::Login(login) => assert_eq!(login.team.as_deref(), Some("acme")),
                _ => panic!("expected login command"),
            },
            _ => panic!("expected auth command"),
        }
    }

    #[test]
    fn parse_note_update() {
        let cli = Cli::try_parse_from([
            "kibel",
            "note",
            "update",
            "--id",
            "N1",
            "--base-content",
            "old",
            "--new-content",
            "new",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Note(args) => match args.command {
                NoteCommand::Update(update) => {
                    assert_eq!(update.id, "N1");
                    assert_eq!(update.base_content, "old");
                    assert_eq!(update.new_content, "new");
                }
                _ => panic!("expected update command"),
            },
            _ => panic!("expected note command"),
        }
    }

    #[test]
    fn parse_note_create_with_json() {
        let cli = Cli::try_parse_from([
            "kibel",
            "--json",
            "note",
            "create",
            "--title",
            "hello",
            "--content",
            "world",
            "--group-id",
            "G1",
        ])
        .expect("parse should succeed");

        assert!(cli.json);
        match cli.command {
            Command::Note(args) => match args.command {
                NoteCommand::Create(create) => {
                    assert_eq!(create.title, "hello");
                    assert_eq!(create.content, "world");
                    assert_eq!(create.group_ids, vec!["G1"]);
                    assert!(!create.draft);
                    assert!(!create.coediting);
                    assert!(create.folders.is_empty());
                    assert!(create.client_mutation_id.is_none());
                }
                _ => panic!("expected create command"),
            },
            _ => panic!("expected note command"),
        }
    }

    #[test]
    fn parse_note_create_with_extended_schema_options() {
        let cli = Cli::try_parse_from([
            "kibel",
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
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Note(args) => match args.command {
                NoteCommand::Create(create) => {
                    assert!(create.draft);
                    assert!(create.coediting);
                    assert_eq!(create.folders.len(), 1);
                    assert_eq!(create.folders[0].group_id, "G1");
                    assert_eq!(create.folders[0].folder_name, "Engineering");
                    assert_eq!(create.author_id.as_deref(), Some("U1"));
                    assert_eq!(create.published_at.as_deref(), Some("2026-02-23T00:00:00Z"));
                    assert_eq!(create.client_mutation_id.as_deref(), Some("cmid-1"));
                }
                _ => panic!("expected create command"),
            },
            _ => panic!("expected note command"),
        }
    }

    #[test]
    fn parse_note_create_accepts_legacy_idempotency_key_alias() {
        let cli = Cli::try_parse_from([
            "kibel",
            "note",
            "create",
            "--title",
            "hello",
            "--content",
            "world",
            "--group-id",
            "G1",
            "--idempotency-key",
            "legacy-key",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Note(args) => match args.command {
                NoteCommand::Create(create) => {
                    assert_eq!(create.idempotency_key.as_deref(), Some("legacy-key"));
                }
                _ => panic!("expected create command"),
            },
            _ => panic!("expected note command"),
        }
    }

    #[test]
    fn parse_search_note_args() {
        let cli = Cli::try_parse_from([
            "kibel",
            "search",
            "note",
            "--query",
            "rust",
            "--after",
            "cursor-1",
            "--resource",
            "note",
            "--group-id",
            "G1",
            "--first",
            "8",
            "--preset",
            "daily",
            "--save-preset",
            "daily",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Search(args) => match args.command {
                SearchCommand::Note(note) => {
                    assert_eq!(note.query, "rust");
                    assert_eq!(note.after.as_deref(), Some("cursor-1"));
                    assert_eq!(note.resources, vec!["note"]);
                    assert_eq!(note.group_ids, vec!["G1"]);
                    assert_eq!(note.first, Some(8));
                    assert_eq!(note.preset.as_deref(), Some("daily"));
                    assert_eq!(note.save_preset.as_deref(), Some("daily"));
                    assert!(note.user_ids.is_empty());
                    assert!(!note.mine);
                }
                SearchCommand::Folder(_) => panic!("expected search note command"),
                SearchCommand::User(_) => panic!("expected search note command"),
            },
            _ => panic!("expected search command"),
        }
    }

    #[test]
    fn parse_search_note_without_query_with_mine() {
        let cli = Cli::try_parse_from(["kibel", "search", "note", "--mine"])
            .expect("parse should succeed");

        match cli.command {
            Command::Search(args) => match args.command {
                SearchCommand::Note(note) => {
                    assert_eq!(note.query, "");
                    assert!(note.mine);
                    assert!(note.after.is_none());
                    assert!(note.user_ids.is_empty());
                }
                SearchCommand::Folder(_) => panic!("expected search note command"),
                SearchCommand::User(_) => panic!("expected search note command"),
            },
            _ => panic!("expected search command"),
        }
    }

    #[test]
    fn parse_search_user_args() {
        let cli = Cli::try_parse_from([
            "kibel",
            "search",
            "user",
            "--query",
            "alice",
            "--first",
            "5",
            "--group-id",
            "G1",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Search(args) => match args.command {
                SearchCommand::User(user) => {
                    assert_eq!(user.query, "alice");
                    assert_eq!(user.first, Some(5));
                    assert_eq!(user.group_ids, vec!["G1"]);
                }
                _ => panic!("expected search user command"),
            },
            _ => panic!("expected search command"),
        }
    }

    #[test]
    fn parse_group_list_args() {
        let cli = Cli::try_parse_from(["kibel", "group", "list", "--first", "10"])
            .expect("parse should succeed");

        match cli.command {
            Command::Group(args) => match args.command {
                GroupCommand::List(list) => assert_eq!(list.first, Some(10)),
            },
            _ => panic!("expected group command"),
        }
    }

    #[test]
    fn parse_note_move_to_folder_args() {
        let cli = Cli::try_parse_from([
            "kibel",
            "note",
            "move-to-folder",
            "--id",
            "N1",
            "--from-folder",
            "G1:Old",
            "--to-folder",
            "G1:New",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Note(args) => match args.command {
                NoteCommand::MoveToFolder(move_args) => {
                    assert_eq!(move_args.id, "N1");
                    assert_eq!(move_args.from_folder.group_id, "G1");
                    assert_eq!(move_args.from_folder.folder_name, "Old");
                    assert_eq!(move_args.to_folder.folder_name, "New");
                }
                _ => panic!("expected move-to-folder command"),
            },
            _ => panic!("expected note command"),
        }
    }

    #[test]
    fn parse_note_get_many_args() {
        let cli = Cli::try_parse_from(["kibel", "note", "get-many", "--id", "N1", "--id", "N2"])
            .expect("parse should succeed");

        match cli.command {
            Command::Note(args) => match args.command {
                NoteCommand::GetMany(get_many) => {
                    assert_eq!(get_many.ids, vec!["N1", "N2"]);
                }
                _ => panic!("expected get-many command"),
            },
            _ => panic!("expected note command"),
        }
    }

    #[test]
    fn parse_folder_arg_rejects_invalid_value() {
        assert!(parse_folder_arg("just-group").is_err());
        assert!(parse_folder_arg(" : ").is_err());
    }

    #[test]
    fn parse_config_set_team() {
        let cli = Cli::try_parse_from(["kibel", "config", "set", "team", "acme"])
            .expect("parse should succeed");

        match cli.command {
            Command::Config(args) => match args.command {
                ConfigCommand::Set(set) => match set.command {
                    ConfigSetCommand::Team(team) => assert_eq!(team.team, "acme"),
                },
                ConfigCommand::Profiles(_) => panic!("expected set command"),
            },
            _ => panic!("expected config command"),
        }
    }

    #[test]
    fn parse_graphql_run_defaults() {
        let cli = Cli::try_parse_from([
            "kibel",
            "graphql",
            "run",
            "--query",
            "query Q { groups { edges { node { id } } } }",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Graphql(args) => match args.command {
                GraphqlCommand::Run(run) => {
                    assert!(run.query.is_some());
                    assert!(run.variables.is_none());
                    assert_eq!(run.timeout_secs, 15);
                    assert_eq!(run.response_limit_mib, 2);
                    assert_eq!(run.max_depth, 8);
                    assert_eq!(run.max_complexity, 1000);
                    assert!(!run.allow_mutation);
                    assert!(!run.unsafe_no_cost_check);
                }
            },
            _ => panic!("expected graphql command"),
        }
    }
}
