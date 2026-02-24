use crate::error::KibelClientError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
#[cfg(any(test, feature = "test-hooks"))]
use std::fs;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[path = "generated_create_note_contract.rs"]
mod generated_create_note_contract;
#[path = "generated_resource_contracts.rs"]
mod generated_resource_contracts;

use self::generated_create_note_contract::{
    CREATE_NOTE_INPUT_FIELDS, CREATE_NOTE_NOTE_PROJECTION_FIELDS, CREATE_NOTE_PAYLOAD_FIELDS,
};
pub use self::generated_resource_contracts::{ResourceContract, TrustedOperation};

const DEFAULT_TIMEOUT_MS: u64 = 5000;
const DEFAULT_FIRST: u32 = 16;
const GRAPHQL_ACCEPT_HEADER: &str = "application/graphql-response+json, application/json;q=0.9";
const APQ_VERSION: u64 = 1;
const APQ_GET_VARIABLES_LIMIT_BYTES: usize = 1024;
const SEARCH_NOTE_RESOURCE_KINDS: [&str; 3] = ["NOTE", "COMMENT", "ATTACHMENT"];

const QUERY_NOTE_GET: &str = r"
query NoteGet($id: ID!) {
  note(id: $id) {
    id
    title
    content
  }
}
";

const QUERY_CREATE_NOTE_SCHEMA: &str = r#"
query CreateNoteSchema {
  createNoteInput: __type(name: "CreateNoteInput") {
    inputFields {
      name
    }
  }
  createNotePayload: __type(name: "CreateNotePayload") {
    fields {
      name
    }
  }
  noteType: __type(name: "Note") {
    fields {
      name
    }
  }
}
"#;

const QUERY_SEARCH_NOTE: &str = r"
query SearchNote(
  $query: String!
  $resources: [SearchResourceKind!]
  $coediting: Boolean
  $updated: SearchDate
  $groupIds: [ID!]
  $userIds: [ID!]
  $folderIds: [ID!]
  $likerIds: [ID!]
  $isArchived: Boolean
  $sortBy: SearchSortKind
  $first: Int!
) {
  search(
    query: $query
    resources: $resources
    coediting: $coediting
    updated: $updated
    groupIds: $groupIds
    userIds: $userIds
    folderIds: $folderIds
    likerIds: $likerIds
    isArchived: $isArchived
    sortBy: $sortBy
    first: $first
  ) {
    edges {
      node {
        document {
          ... on Node {
            id
          }
        }
        title
        url
        contentSummaryHtml
        path
        author {
          account
          realName
        }
      }
    }
  }
}
";

const QUERY_CURRENT_USER_ID: &str = r"
query GetCurrentUserId {
  currentUser {
    id
  }
}
";

const QUERY_CURRENT_USER_LATEST_NOTES: &str = r"
query GetCurrentUserLatestNotes($first: Int!) {
  currentUser {
    latestNotes(first: $first) {
      edges {
        node {
          id
          title
          url
          updatedAt
          contentSummaryHtml
          path
          author {
            account
            realName
          }
        }
      }
    }
  }
}
";

const QUERY_SEARCH_FOLDER: &str = r"
query SearchFolder($query: String!, $first: Int!) {
  searchFolder(query: $query, first: $first) {
    edges {
      node {
        name
        fixedPath
        group {
          name
          isPrivate
        }
      }
    }
  }
}
";

const QUERY_GET_GROUPS: &str = r"
query GetGroups($first: Int!) {
  groups(first: $first) {
    edges {
      node {
        id
        name
        isDefault
        isArchived
      }
    }
  }
}
";

const QUERY_GET_FOLDERS: &str = r"
query GetFolders($first: Int!) {
  folders(first: $first) {
    edges {
      node {
        id
        name
      }
    }
  }
}
";

const QUERY_GET_NOTES: &str = r"
query GetNotes($folderId: ID!, $first: Int!, $last: Int) {
  notes(folderId: $folderId, first: $first, last: $last) {
    edges {
      node {
        id
        title
        url
      }
    }
  }
}
";

const QUERY_GET_NOTE_FROM_PATH: &str = r"
query GetNoteFromPath($path: String!, $first: Int!) {
  noteFromPath(path: $path) {
    id
    title
    content
    url
    author {
      account
      realName
    }
    folders(first: $first) {
      edges {
        node {
          id
          name
          fullName
          fixedPath
          group {
            id
            name
          }
        }
      }
    }
    comments(first: $first) {
      edges {
        node {
          id
          anchor
          content
          author {
            account
            realName
          }
          replies(first: $first) {
            edges {
              node {
                id
                anchor
                content
                author {
                  account
                  realName
                }
              }
            }
          }
        }
      }
    }
    inlineComments(first: $first) {
      edges {
        node {
          id
          anchor
          content
          author {
            account
            realName
          }
          replies(first: $first) {
            edges {
              node {
                id
                anchor
                content
                author {
                  account
                  realName
                }
              }
            }
          }
        }
      }
    }
  }
}
";

const QUERY_GET_FOLDER: &str = r"
query GetFolder($id: ID!, $first: Int!) {
  folder(id: $id) {
    name
    fullName
    fixedPath
    createdAt
    updatedAt
    group {
      id
      name
    }
    folders(first: $first) {
      edges {
        node {
          id
          name
        }
      }
    }
    notes(first: $first) {
      edges {
        node {
          id
          title
        }
      }
    }
  }
}
";

const QUERY_GET_FOLDER_FROM_PATH: &str = r"
query GetFolderFromPath($path: String!, $first: Int!) {
  folderFromPath(path: $path) {
    name
    fullName
    fixedPath
    createdAt
    updatedAt
    group {
      id
      name
    }
    folders(first: $first) {
      edges {
        node {
          id
          name
        }
      }
    }
    notes(first: $first) {
      edges {
        node {
          id
          title
        }
      }
    }
  }
}
";

const QUERY_GET_FEED_SECTIONS: &str = r"
query GetFeedSections($kind: FeedKind!, $groupId: ID!, $first: Int!) {
  feedSections(kind: $kind, groupId: $groupId, first: $first) {
    edges {
      node {
        ... on FeedNote {
          date
          note {
            id
            title
            contentSummaryHtml
          }
        }
        ... on FeedFolderParcel {
          date
          folder {
            id
            name
          }
          notes(first: $first) {
            edges {
              node {
                id
                title
                contentSummaryHtml
              }
            }
          }
        }
        ... on FeedUserParcel {
          date
          user {
            account
            realName
          }
          notes(first: $first) {
            edges {
              node {
                id
                title
                contentSummaryHtml
              }
            }
          }
        }
      }
    }
  }
}
";

const MUTATION_CREATE_COMMENT: &str = r"
mutation CreateComment($input: CreateCommentInput!) {
  createComment(input: $input) {
    comment {
      id
    }
  }
}
";

const MUTATION_CREATE_COMMENT_REPLY: &str = r"
mutation CreateCommentReply($input: CreateCommentReplyInput!) {
  createCommentReply(input: $input) {
    reply {
      id
    }
  }
}
";

const MUTATION_CREATE_FOLDER: &str = r"
mutation CreateFolder($input: CreateFolderInput!) {
  createFolder(input: $input) {
    folder {
      id
    }
  }
}
";

const MUTATION_MOVE_NOTE_TO_ANOTHER_FOLDER: &str = r"
mutation MoveNoteToAnotherFolder($input: MoveNoteToAnotherFolderInput!) {
  moveNoteToAnotherFolder(input: $input) {
    note {
      id
    }
  }
}
";

const MUTATION_ATTACH_NOTE_TO_FOLDER: &str = r"
mutation AttachNoteToFolder($input: AttachNoteToFolderInput!) {
  attachNoteToFolder(input: $input) {
    note {
      id
    }
  }
}
";

const MUTATION_UPDATE_NOTE_CONTENT: &str = r"
mutation UpdateNoteContent($input: UpdateNoteContentInput!) {
  updateNoteContent(input: $input) {
    note {
      id
      title
      content
    }
  }
}
";

#[must_use]
pub fn resource_contracts() -> &'static [ResourceContract] {
    generated_resource_contracts::RESOURCE_CONTRACTS
}

#[must_use]
pub fn resource_contract_version() -> u32 {
    generated_resource_contracts::RESOURCE_CONTRACT_VERSION
}

#[must_use]
pub fn resource_contract_upstream_commit() -> &'static str {
    generated_resource_contracts::RESOURCE_CONTRACT_UPSTREAM_COMMIT
}

#[must_use]
pub fn trusted_operations() -> &'static [TrustedOperation] {
    generated_resource_contracts::TRUSTED_OPERATIONS
}

#[must_use]
pub fn trusted_operation_contract(operation: TrustedOperation) -> &'static ResourceContract {
    generated_resource_contracts::trusted_operation_contract(operation)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNoteInput {
    pub title: String,
    pub content: String,
    pub group_ids: Vec<String>,
    pub draft: Option<bool>,
    pub coediting: bool,
    pub folders: Vec<CreateNoteFolderInput>,
    pub author_id: Option<String>,
    pub published_at: Option<String>,
    pub client_mutation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNoteFolderInput {
    pub group_id: String,
    pub folder_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNoteResult {
    pub note: Note,
    pub client_mutation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateNoteInput {
    pub id: String,
    pub base_content: String,
    pub new_content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchNoteInput {
    pub query: String,
    pub resources: Vec<String>,
    pub coediting: Option<bool>,
    pub updated: Option<String>,
    pub group_ids: Vec<String>,
    pub user_ids: Vec<String>,
    pub folder_ids: Vec<String>,
    pub liker_ids: Vec<String>,
    pub is_archived: Option<bool>,
    pub sort_by: Option<String>,
    pub first: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFolderInput {
    pub query: String,
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInput {
    pub first: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNotesInput {
    pub folder_id: String,
    pub first: Option<u32>,
    pub last: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathLookupInput {
    pub path: String,
    pub first: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderLookupInput {
    pub id: String,
    pub first: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedSectionsInput {
    pub kind: String,
    pub group_id: String,
    pub first: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateCommentInput {
    pub content: String,
    pub note_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateCommentReplyInput {
    pub content: String,
    pub comment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFolderInput {
    pub group_id: String,
    pub full_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveNoteToAnotherFolderInput {
    pub id: String,
    pub from_folder: CreateNoteFolderInput,
    pub to_folder: CreateNoteFolderInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachNoteToFolderInput {
    pub id: String,
    pub folder: CreateNoteFolderInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdOnlyResult {
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct KibelClient {
    origin: String,
    endpoint: String,
    token: String,
    timeout_ms: u64,
    create_note_schema: Arc<Mutex<Option<CreateNoteSchema>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryTransportMode {
    PostOnly,
    TrustedQueryApqGet,
}

#[derive(Debug, Clone)]
struct ParsedGraphqlResponse {
    payload: Value,
    status_code: Option<u16>,
}

impl KibelClient {
    /// Builds a client for a Kibela origin and access token.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when `origin` or `token` is
    /// empty after trimming.
    pub fn new(
        origin: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, KibelClientError> {
        let origin = origin.into().trim().trim_end_matches('/').to_string();
        let token = token.into().trim().to_string();

        if origin.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "origin is required".to_string(),
            ));
        }
        if token.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "access token is required".to_string(),
            ));
        }

        let endpoint = endpoint_from_origin(&origin);

        Ok(Self {
            origin,
            endpoint,
            token,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            create_note_schema: Arc::new(Mutex::new(None)),
        })
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Executes an ad-hoc GraphQL request outside the trusted operation registry.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when query or limits are invalid,
    /// or transport/API errors from GraphQL.
    pub fn run_untrusted_graphql(
        &self,
        query: &str,
        variables: Value,
        timeout_ms: u64,
        max_response_bytes: usize,
    ) -> Result<Value, KibelClientError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "query is required".to_string(),
            ));
        }
        if max_response_bytes == 0 {
            return Err(KibelClientError::InputInvalid(
                "max response bytes must be greater than 0".to_string(),
            ));
        }
        self.request_graphql_raw_with_limits(
            query,
            variables,
            timeout_ms.max(100),
            Some(max_response_bytes),
            QueryTransportMode::PostOnly,
        )
    }

    /// Fetches a note by id.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when `id` is empty, or
    /// transport/API errors from GraphQL.
    pub fn get_note(&self, id: &str) -> Result<Note, KibelClientError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "note id is required".to_string(),
            ));
        }

        let payload = self.request_trusted_graphql(
            TrustedOperation::GetNote,
            QUERY_NOTE_GET,
            json!({ "id": id }),
        )?;
        parse_note_at(&payload, "/data/note")
    }

    /// Creates a note from input while adapting to runtime schema.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] for invalid required fields,
    /// or transport/API errors from GraphQL.
    pub fn create_note(
        &self,
        input: &CreateNoteInput,
    ) -> Result<CreateNoteResult, KibelClientError> {
        let title = input.title.trim();
        let content = input.content.trim();

        if title.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "title is required".to_string(),
            ));
        }
        if content.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "content is required".to_string(),
            ));
        }
        let group_ids = normalize_vec(&input.group_ids);
        if group_ids.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "at least one group id is required".to_string(),
            ));
        }

        let schema = self.resolve_create_note_schema();
        let mut gql_input = serde_json::Map::new();

        if schema.supports_input("title") {
            gql_input.insert("title".to_string(), Value::String(title.to_string()));
        }
        if schema.supports_input("content") {
            gql_input.insert("content".to_string(), Value::String(content.to_string()));
        }
        if schema.supports_input("groupIds") {
            gql_input.insert("groupIds".to_string(), json!(group_ids));
        }
        if schema.supports_input("coediting") {
            gql_input.insert("coediting".to_string(), Value::Bool(input.coediting));
        }
        if schema.supports_input("draft") {
            if let Some(draft) = input.draft {
                gql_input.insert("draft".to_string(), Value::Bool(draft));
            }
        }
        if schema.supports_input("folders") && !input.folders.is_empty() {
            let mut folders = Vec::with_capacity(input.folders.len());
            for folder in &input.folders {
                let group_id = folder.group_id.trim();
                let folder_name = folder.folder_name.trim();
                if group_id.is_empty() || folder_name.is_empty() {
                    return Err(KibelClientError::InputInvalid(
                        "folder requires non-empty group_id and folder_name".to_string(),
                    ));
                }
                folders.push(json!({
                    "groupId": group_id,
                    "folderName": folder_name,
                }));
            }
            gql_input.insert("folders".to_string(), Value::Array(folders));
        }
        if schema.supports_input("authorId") {
            if let Some(author_id) = input.author_id.as_deref().and_then(normalize_optional) {
                gql_input.insert("authorId".to_string(), Value::String(author_id));
            }
        }
        if schema.supports_input("publishedAt") {
            if let Some(published_at) = input.published_at.as_deref().and_then(normalize_optional) {
                gql_input.insert("publishedAt".to_string(), Value::String(published_at));
            }
        }
        if schema.supports_input("clientMutationId") {
            if let Some(client_mutation_id) = input
                .client_mutation_id
                .as_deref()
                .and_then(normalize_optional)
            {
                gql_input.insert(
                    "clientMutationId".to_string(),
                    Value::String(client_mutation_id),
                );
            }
        }

        let mutation = schema.create_note_mutation();
        let payload = self.request_trusted_graphql(
            TrustedOperation::CreateNote,
            &mutation,
            json!({ "input": Value::Object(gql_input) }),
        )?;
        let note = parse_create_note_at(&payload, "/data/createNote/note")?;
        let client_mutation_id = payload
            .pointer("/data/createNote/clientMutationId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        Ok(CreateNoteResult {
            note,
            client_mutation_id,
        })
    }

    /// Updates note content using optimistic locking (`base_content`).
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when required fields are empty,
    /// or transport/API errors from GraphQL.
    pub fn update_note(&self, input: &UpdateNoteInput) -> Result<Note, KibelClientError> {
        let id = input.id.trim();
        let base_content = input.base_content.trim();
        let new_content = input.new_content.trim();

        if id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "note id is required".to_string(),
            ));
        }
        if base_content.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "base content is required".to_string(),
            ));
        }
        if new_content.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "new content is required".to_string(),
            ));
        }

        let payload = self.request_trusted_graphql(
            TrustedOperation::UpdateNoteContent,
            MUTATION_UPDATE_NOTE_CONTENT,
            json!({
                "input": {
                    "id": id,
                    "baseContent": base_content,
                    "newContent": new_content,
                }
            }),
        )?;
        parse_note_at(&payload, "/data/updateNoteContent/note")
    }

    /// Searches notes.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when paging is invalid,
    /// or transport/API errors from GraphQL.
    pub fn search_note(&self, input: &SearchNoteInput) -> Result<Value, KibelClientError> {
        let first = normalize_first(input.first)?;
        let variables = build_search_note_variables(input, first)?;

        let payload = self.request_trusted_graphql(
            TrustedOperation::SearchNote,
            QUERY_SEARCH_NOTE,
            Value::Object(variables),
        )?;
        let edges = require_array_at(&payload, "/data/search/edges", "search result not found")?;
        let mut items = Vec::with_capacity(edges.len());
        for edge in edges {
            let node = edge.get("node").unwrap_or(&Value::Null);
            items.push(json!({
                "id": node.pointer("/document/id").cloned().unwrap_or(Value::Null),
                "title": node.get("title").cloned().unwrap_or(Value::Null),
                "url": node.get("url").cloned().unwrap_or(Value::Null),
                "contentSummaryHtml": node.get("contentSummaryHtml").cloned().unwrap_or(Value::Null),
                "path": node.get("path").cloned().unwrap_or(Value::Null),
                "author": {
                    "account": node.pointer("/author/account").cloned().unwrap_or(Value::Null),
                    "realName": node.pointer("/author/realName").cloned().unwrap_or(Value::Null),
                }
            }));
        }
        Ok(Value::Array(items))
    }

    /// Returns the current authenticated user id.
    ///
    /// # Errors
    /// Returns transport/API errors from GraphQL, or [`KibelClientError::Api`]
    /// when the current user payload is unavailable.
    pub fn get_current_user_id(&self) -> Result<String, KibelClientError> {
        let payload = self.run_untrusted_graphql(
            QUERY_CURRENT_USER_ID,
            json!({}),
            self.timeout_ms,
            128 * 1024,
        )?;
        parse_current_user_id(&payload)
    }

    /// Returns latest notes for the current authenticated user.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when paging is invalid,
    /// or transport/API errors from GraphQL.
    pub fn get_current_user_latest_notes(
        &self,
        input: PageInput,
    ) -> Result<Value, KibelClientError> {
        let first = normalize_first(input.first)?;
        let payload = self.run_untrusted_graphql(
            QUERY_CURRENT_USER_LATEST_NOTES,
            json!({ "first": first }),
            self.timeout_ms,
            2 * 1024 * 1024,
        )?;
        let edges = require_array_at(
            &payload,
            "/data/currentUser/latestNotes/edges",
            "current user latest notes not found",
        )?;
        let mut items = Vec::with_capacity(edges.len());
        for edge in edges {
            let node = edge.get("node").unwrap_or(&Value::Null);
            items.push(json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "title": node.get("title").cloned().unwrap_or(Value::Null),
                "url": node.get("url").cloned().unwrap_or(Value::Null),
                "updatedAt": node.get("updatedAt").cloned().unwrap_or(Value::Null),
                "contentSummaryHtml": node.get("contentSummaryHtml").cloned().unwrap_or(Value::Null),
                "path": node.get("path").cloned().unwrap_or(Value::Null),
                "author": {
                    "account": node.pointer("/author/account").cloned().unwrap_or(Value::Null),
                    "realName": node.pointer("/author/realName").cloned().unwrap_or(Value::Null),
                }
            }));
        }
        Ok(Value::Array(items))
    }

    /// Searches folders.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when query/paging is invalid,
    /// or transport/API errors from GraphQL.
    pub fn search_folder(&self, input: &SearchFolderInput) -> Result<Value, KibelClientError> {
        let query = input.query.trim();
        if query.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "query is required".to_string(),
            ));
        }
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::SearchFolder,
            QUERY_SEARCH_FOLDER,
            json!({
                "query": query,
                "first": first,
            }),
        )?;
        let edges = require_array_at(
            &payload,
            "/data/searchFolder/edges",
            "folder search result not found",
        )?;
        let mut items = Vec::with_capacity(edges.len());
        for edge in edges {
            let node = edge.get("node").unwrap_or(&Value::Null);
            items.push(json!({
                "name": node.get("name").cloned().unwrap_or(Value::Null),
                "fixedPath": node.get("fixedPath").cloned().unwrap_or(Value::Null),
                "group": {
                    "name": node.pointer("/group/name").cloned().unwrap_or(Value::Null),
                    "isPrivate": node.pointer("/group/isPrivate").cloned().unwrap_or(Value::Null),
                }
            }));
        }
        Ok(Value::Array(items))
    }

    /// Lists groups.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when paging is invalid, or
    /// transport/API errors from GraphQL.
    pub fn get_groups(&self, input: PageInput) -> Result<Value, KibelClientError> {
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetGroups,
            QUERY_GET_GROUPS,
            json!({ "first": first }),
        )?;
        let edges = require_array_at(&payload, "/data/groups/edges", "groups not found")?;
        let mut items = Vec::with_capacity(edges.len());
        for edge in edges {
            let node = edge.get("node").unwrap_or(&Value::Null);
            items.push(json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "name": node.get("name").cloned().unwrap_or(Value::Null),
                "isDefault": node.get("isDefault").cloned().unwrap_or(Value::Null),
                "isArchived": node.get("isArchived").cloned().unwrap_or(Value::Null),
            }));
        }
        Ok(Value::Array(items))
    }

    /// Lists folders.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when paging is invalid, or
    /// transport/API errors from GraphQL.
    pub fn get_folders(&self, input: PageInput) -> Result<Value, KibelClientError> {
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetFolders,
            QUERY_GET_FOLDERS,
            json!({ "first": first }),
        )?;
        let edges = require_array_at(&payload, "/data/folders/edges", "folders not found")?;
        let mut items = Vec::with_capacity(edges.len());
        for edge in edges {
            let node = edge.get("node").unwrap_or(&Value::Null);
            items.push(json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "name": node.get("name").cloned().unwrap_or(Value::Null),
            }));
        }
        Ok(Value::Array(items))
    }

    /// Lists notes under a folder.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when folder/paging is invalid,
    /// or transport/API errors from GraphQL.
    pub fn get_notes(&self, input: &GetNotesInput) -> Result<Value, KibelClientError> {
        let folder_id = input.folder_id.trim();
        if folder_id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "folder id is required".to_string(),
            ));
        }
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetNotes,
            QUERY_GET_NOTES,
            json!({
                "folderId": folder_id,
                "first": first,
                "last": input.last,
            }),
        )?;
        let edges = require_array_at(&payload, "/data/notes/edges", "notes not found")?;
        let mut items = Vec::with_capacity(edges.len());
        for edge in edges {
            let node = edge.get("node").unwrap_or(&Value::Null);
            items.push(json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "title": node.get("title").cloned().unwrap_or(Value::Null),
                "url": node.get("url").cloned().unwrap_or(Value::Null),
            }));
        }
        Ok(Value::Array(items))
    }

    /// Gets a note by Kibela path.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when path/paging is invalid,
    /// or transport/API errors from GraphQL.
    pub fn get_note_from_path(&self, input: &PathLookupInput) -> Result<Value, KibelClientError> {
        let path = input.path.trim();
        if path.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "path is required".to_string(),
            ));
        }
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetNoteFromPath,
            QUERY_GET_NOTE_FROM_PATH,
            json!({
                "path": path,
                "first": first,
            }),
        )?;
        require_value_at(&payload, "/data/noteFromPath", "note not found")
    }

    /// Gets folder details by id.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when id/paging is invalid, or
    /// transport/API errors from GraphQL.
    pub fn get_folder(&self, input: &FolderLookupInput) -> Result<Value, KibelClientError> {
        let id = input.id.trim();
        if id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "folder id is required".to_string(),
            ));
        }
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetFolder,
            QUERY_GET_FOLDER,
            json!({
                "id": id,
                "first": first,
            }),
        )?;
        require_value_at(&payload, "/data/folder", "folder not found")
    }

    /// Gets folder details by path.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when path/paging is invalid,
    /// or transport/API errors from GraphQL.
    pub fn get_folder_from_path(&self, input: &PathLookupInput) -> Result<Value, KibelClientError> {
        let path = input.path.trim();
        if path.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "path is required".to_string(),
            ));
        }
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetFolderFromPath,
            QUERY_GET_FOLDER_FROM_PATH,
            json!({
                "path": path,
                "first": first,
            }),
        )?;
        require_value_at(&payload, "/data/folderFromPath", "folder not found")
    }

    /// Lists feed section entries.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when kind/group/paging is
    /// invalid, or transport/API errors from GraphQL.
    pub fn get_feed_sections(&self, input: &FeedSectionsInput) -> Result<Value, KibelClientError> {
        let kind = input.kind.trim();
        let group_id = input.group_id.trim();
        if kind.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "kind is required".to_string(),
            ));
        }
        if group_id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "group id is required".to_string(),
            ));
        }
        let first = normalize_first(input.first)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::GetFeedSections,
            QUERY_GET_FEED_SECTIONS,
            json!({
                "kind": kind,
                "groupId": group_id,
                "first": first,
            }),
        )?;
        require_value_at(
            &payload,
            "/data/feedSections/edges",
            "feed sections not found",
        )
    }

    /// Creates a top-level comment on a note.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when required fields are empty,
    /// or transport/API errors from GraphQL.
    pub fn create_comment(
        &self,
        input: &CreateCommentInput,
    ) -> Result<IdOnlyResult, KibelClientError> {
        let content = input.content.trim();
        let note_id = input.note_id.trim();
        if content.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "content is required".to_string(),
            ));
        }
        if note_id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "note id is required".to_string(),
            ));
        }
        let payload = self.request_trusted_graphql(
            TrustedOperation::CreateComment,
            MUTATION_CREATE_COMMENT,
            json!({
                "input": {
                    "content": content,
                    "commentableId": note_id,
                }
            }),
        )?;
        parse_id_only_at(
            &payload,
            "/data/createComment/comment/id",
            "createComment response",
        )
    }

    /// Creates a reply for an existing comment.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when required fields are empty,
    /// or transport/API errors from GraphQL.
    pub fn create_comment_reply(
        &self,
        input: &CreateCommentReplyInput,
    ) -> Result<IdOnlyResult, KibelClientError> {
        let content = input.content.trim();
        let comment_id = input.comment_id.trim();
        if content.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "content is required".to_string(),
            ));
        }
        if comment_id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "comment id is required".to_string(),
            ));
        }
        let payload = self.request_trusted_graphql(
            TrustedOperation::CreateCommentReply,
            MUTATION_CREATE_COMMENT_REPLY,
            json!({
                "input": {
                    "content": content,
                    "commentId": comment_id,
                }
            }),
        )?;
        parse_id_only_at(
            &payload,
            "/data/createCommentReply/reply/id",
            "createCommentReply response",
        )
    }

    /// Creates a folder in a group.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when required fields are empty,
    /// or transport/API errors from GraphQL.
    pub fn create_folder(
        &self,
        input: &CreateFolderInput,
    ) -> Result<IdOnlyResult, KibelClientError> {
        let group_id = input.group_id.trim();
        let full_name = input.full_name.trim();
        if group_id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "group id is required".to_string(),
            ));
        }
        if full_name.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "full name is required".to_string(),
            ));
        }
        let payload = self.request_trusted_graphql(
            TrustedOperation::CreateFolder,
            MUTATION_CREATE_FOLDER,
            json!({
                "input": {
                    "folder": {
                        "groupId": group_id,
                        "folderName": full_name,
                    }
                }
            }),
        )?;
        parse_id_only_at(
            &payload,
            "/data/createFolder/folder/id",
            "createFolder response",
        )
    }

    /// Moves a note from one folder to another.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when required fields are empty,
    /// or transport/API errors from GraphQL.
    pub fn move_note_to_another_folder(
        &self,
        input: &MoveNoteToAnotherFolderInput,
    ) -> Result<IdOnlyResult, KibelClientError> {
        let id = input.id.trim();
        if id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "note id is required".to_string(),
            ));
        }
        let from_folder = normalize_folder(&input.from_folder)?;
        let to_folder = normalize_folder(&input.to_folder)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::MoveNoteToAnotherFolder,
            MUTATION_MOVE_NOTE_TO_ANOTHER_FOLDER,
            json!({
                "input": {
                    "noteId": id,
                    "fromFolder": from_folder,
                    "toFolder": to_folder,
                }
            }),
        )?;
        parse_id_only_at(
            &payload,
            "/data/moveNoteToAnotherFolder/note/id",
            "moveNoteToAnotherFolder response",
        )
    }

    /// Attaches a note to a folder.
    ///
    /// # Errors
    /// Returns [`KibelClientError::InputInvalid`] when required fields are empty,
    /// or transport/API errors from GraphQL.
    pub fn attach_note_to_folder(
        &self,
        input: &AttachNoteToFolderInput,
    ) -> Result<IdOnlyResult, KibelClientError> {
        let id = input.id.trim();
        if id.is_empty() {
            return Err(KibelClientError::InputInvalid(
                "note id is required".to_string(),
            ));
        }
        let folder = normalize_folder(&input.folder)?;
        let payload = self.request_trusted_graphql(
            TrustedOperation::AttachNoteToFolder,
            MUTATION_ATTACH_NOTE_TO_FOLDER,
            json!({
                "input": {
                    "noteId": id,
                    "folder": folder,
                }
            }),
        )?;
        parse_id_only_at(
            &payload,
            "/data/attachNoteToFolder/note/id",
            "attachNoteToFolder response",
        )
    }

    fn request_trusted_graphql(
        &self,
        operation: TrustedOperation,
        query: &str,
        variables: Value,
    ) -> Result<Value, KibelClientError> {
        validate_trusted_operation_request(operation, query, &variables)?;
        let mode = match trusted_operation_contract(operation).kind {
            "query" => QueryTransportMode::TrustedQueryApqGet,
            _ => QueryTransportMode::PostOnly,
        };
        self.request_graphql_raw_with_limits(query, variables, self.timeout_ms, None, mode)
    }

    fn request_graphql_raw(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<Value, KibelClientError> {
        self.request_graphql_raw_with_limits(
            query,
            variables,
            self.timeout_ms,
            None,
            QueryTransportMode::PostOnly,
        )
    }

    fn request_graphql_raw_with_limits(
        &self,
        query: &str,
        variables: Value,
        timeout_ms: u64,
        max_response_bytes: Option<usize>,
        mode: QueryTransportMode,
    ) -> Result<Value, KibelClientError> {
        let timeout = Duration::from_millis(timeout_ms.max(100));
        let payload = json!({
            "query": query,
            "variables": variables.clone(),
        });
        let payload_raw = payload.to_string();

        test_capture_request_payload(&payload_raw)?;

        if let Some(message) = test_transport_error_message() {
            return Err(KibelClientError::Transport(message));
        }

        if let Some(parsed) = load_graphql_response_fixture()? {
            if let Some((code, message)) = extract_graphql_error(&parsed) {
                return Err(KibelClientError::Api { code, message });
            }
            return Ok(parsed);
        }

        let parsed = match mode {
            QueryTransportMode::PostOnly => {
                self.request_graphql_post(timeout, max_response_bytes, query, &variables, None)?
            }
            QueryTransportMode::TrustedQueryApqGet => {
                self.request_trusted_query_with_apq(timeout, max_response_bytes, query, &variables)?
            }
        };

        finalize_graphql_response(parsed)
    }

    fn request_trusted_query_with_apq(
        &self,
        timeout: Duration,
        max_response_bytes: Option<usize>,
        query: &str,
        variables: &Value,
    ) -> Result<ParsedGraphqlResponse, KibelClientError> {
        let persisted_hash = sha256_hex(query);
        let extensions = json!({
            "persistedQuery": {
                "version": APQ_VERSION,
                "sha256Hash": persisted_hash,
            }
        });

        let variables_raw = serde_json::to_string(variables)
            .map_err(|error| KibelClientError::Transport(format!("json render failed: {error}")))?;

        if variables_raw.len() > APQ_GET_VARIABLES_LIMIT_BYTES {
            return self.request_graphql_post(
                timeout,
                max_response_bytes,
                query,
                variables,
                Some(&extensions),
            );
        }

        let get_response = self.request_graphql_get_hash_only(
            timeout,
            max_response_bytes,
            variables,
            &extensions,
        )?;
        if should_fallback_apq_status(get_response.status_code) {
            return self.request_graphql_post(timeout, max_response_bytes, query, variables, None);
        }

        let Some((error_code, message)) = extract_graphql_error(&get_response.payload) else {
            return Ok(get_response);
        };

        if is_persisted_query_not_found(&error_code, &message) {
            return self.request_graphql_post(
                timeout,
                max_response_bytes,
                query,
                variables,
                Some(&extensions),
            );
        }
        if is_persisted_query_not_supported(&error_code, &message) {
            return self.request_graphql_post(timeout, max_response_bytes, query, variables, None);
        }

        Ok(get_response)
    }

    fn request_graphql_post(
        &self,
        timeout: Duration,
        max_response_bytes: Option<usize>,
        query: &str,
        variables: &Value,
        extensions: Option<&Value>,
    ) -> Result<ParsedGraphqlResponse, KibelClientError> {
        let mut payload_object = serde_json::Map::new();
        payload_object.insert("query".to_string(), Value::String(query.to_string()));
        payload_object.insert("variables".to_string(), variables.clone());
        if let Some(extensions) = extensions {
            payload_object.insert("extensions".to_string(), extensions.clone());
        }
        let payload_raw = Value::Object(payload_object).to_string();

        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        let request = agent
            .post(&self.endpoint)
            .set("Content-Type", "application/json")
            .set("Accept", GRAPHQL_ACCEPT_HEADER)
            .set("Authorization", &format!("Bearer {}", self.token));

        let (raw, status_code) = match request.send_string(&payload_raw) {
            Ok(response) => {
                let body = read_response_body(response, max_response_bytes)?;
                (body, None)
            }
            Err(ureq::Error::Status(code, response)) => {
                let body = read_response_body(response, max_response_bytes)?;
                (body, Some(code))
            }
            Err(err) => {
                return Err(KibelClientError::Transport(err.to_string()));
            }
        };

        parse_http_response(raw, status_code)
    }

    fn request_graphql_get_hash_only(
        &self,
        timeout: Duration,
        max_response_bytes: Option<usize>,
        variables: &Value,
        extensions: &Value,
    ) -> Result<ParsedGraphqlResponse, KibelClientError> {
        let variables_raw = serde_json::to_string(variables)
            .map_err(|error| KibelClientError::Transport(format!("json render failed: {error}")))?;
        let extensions_raw = serde_json::to_string(extensions)
            .map_err(|error| KibelClientError::Transport(format!("json render failed: {error}")))?;

        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        let request = agent
            .get(&self.endpoint)
            .query("variables", &variables_raw)
            .query("extensions", &extensions_raw)
            .set("Accept", GRAPHQL_ACCEPT_HEADER)
            .set("Authorization", &format!("Bearer {}", self.token));

        let (raw, status_code) = match request.call() {
            Ok(response) => {
                let body = read_response_body(response, max_response_bytes)?;
                (body, None)
            }
            Err(ureq::Error::Status(code, response)) => {
                let body = read_response_body(response, max_response_bytes)?;
                (body, Some(code))
            }
            Err(err) => {
                return Err(KibelClientError::Transport(err.to_string()));
            }
        };

        parse_http_response(raw, status_code)
    }

    fn resolve_create_note_schema(&self) -> CreateNoteSchema {
        if let Some(schema) = load_schema_fixture_from_env() {
            return schema;
        }
        if should_skip_runtime_introspection() {
            return CreateNoteSchema::default();
        }
        if let Ok(guard) = self.create_note_schema.lock() {
            if let Some(schema) = guard.as_ref() {
                return schema.clone();
            }
        }

        if let Ok(payload) = self.request_graphql_raw(QUERY_CREATE_NOTE_SCHEMA, json!({})) {
            if let Some(schema) = CreateNoteSchema::from_introspection(&payload) {
                if let Ok(mut guard) = self.create_note_schema.lock() {
                    *guard = Some(schema.clone());
                }
                return schema;
            }
        }
        CreateNoteSchema::default()
    }
}

fn endpoint_from_origin(origin: &str) -> String {
    let normalized = origin.trim().trim_end_matches('/');
    if normalized.ends_with("/api/v1") {
        normalized.to_string()
    } else {
        format!("{normalized}/api/v1")
    }
}

fn read_response_body(
    response: ureq::Response,
    max_response_bytes: Option<usize>,
) -> Result<String, KibelClientError> {
    match max_response_bytes {
        Some(limit) => {
            let mut buffer = Vec::new();
            response
                .into_reader()
                .take((limit.saturating_add(1)) as u64)
                .read_to_end(&mut buffer)
                .map_err(|error| KibelClientError::Transport(error.to_string()))?;
            if buffer.len() > limit {
                return Err(KibelClientError::Transport(format!(
                    "response body exceeds limit: {limit} bytes"
                )));
            }
            String::from_utf8(buffer)
                .map_err(|error| KibelClientError::Transport(error.to_string()))
        }
        None => response
            .into_string()
            .map_err(|error| KibelClientError::Transport(error.to_string())),
    }
}

fn parse_http_response(
    raw: String,
    status_code: Option<u16>,
) -> Result<ParsedGraphqlResponse, KibelClientError> {
    let payload = serde_json::from_str::<Value>(&raw)
        .map_err(|error| KibelClientError::Transport(format!("invalid JSON response: {error}")))?;
    Ok(ParsedGraphqlResponse {
        payload,
        status_code,
    })
}

fn finalize_graphql_response(response: ParsedGraphqlResponse) -> Result<Value, KibelClientError> {
    if let Some((code, message)) = extract_graphql_error(&response.payload) {
        return Err(KibelClientError::Api { code, message });
    }
    if let Some(code) = response.status_code {
        return Err(KibelClientError::Transport(format!(
            "http status {code} without graphql errors"
        )));
    }
    Ok(response.payload)
}

fn should_fallback_apq_status(status_code: Option<u16>) -> bool {
    matches!(
        status_code,
        Some(400 | 404 | 405 | 406 | 414 | 415 | 422 | 501)
    )
}

fn is_persisted_query_not_found(code: &str, message: &str) -> bool {
    code.eq_ignore_ascii_case("PERSISTED_QUERY_NOT_FOUND")
        || message
            .to_ascii_uppercase()
            .contains("PERSISTED_QUERY_NOT_FOUND")
}

fn is_persisted_query_not_supported(code: &str, message: &str) -> bool {
    code.eq_ignore_ascii_case("PERSISTED_QUERY_NOT_SUPPORTED")
        || message
            .to_ascii_uppercase()
            .contains("PERSISTED_QUERY_NOT_SUPPORTED")
}

fn sha256_hex(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn validate_trusted_operation_request(
    operation: TrustedOperation,
    query: &str,
    variables: &Value,
) -> Result<(), KibelClientError> {
    let contract = trusted_operation_contract(operation);
    let expected_root = contract
        .graphql_file
        .rsplit('.')
        .next()
        .ok_or_else(|| {
            KibelClientError::Transport(format!(
                "invalid graphql_file format in contract: {}",
                contract.graphql_file
            ))
        })?
        .trim();
    if expected_root.is_empty() {
        return Err(KibelClientError::Transport(
            "empty root field in trusted contract".to_string(),
        ));
    }

    let actual_root = extract_root_field(query).ok_or_else(|| {
        KibelClientError::Transport(format!(
            "failed to extract root field for trusted operation `{}`",
            contract.name
        ))
    })?;
    if actual_root != expected_root {
        return Err(KibelClientError::Transport(format!(
            "trusted operation `{}` root field mismatch: expected `{}`, got `{}`",
            contract.name, expected_root, actual_root
        )));
    }

    let declared_variables = extract_declared_variables(query);
    let missing_declarations = contract
        .required_variables
        .iter()
        .filter(|name| !declared_variables.contains(**name))
        .copied()
        .collect::<Vec<_>>();
    if !missing_declarations.is_empty() {
        return Err(KibelClientError::Transport(format!(
            "trusted operation `{}` required variable(s) are not declared in query: {}",
            contract.name,
            missing_declarations.join(", ")
        )));
    }

    let object = variables.as_object().ok_or_else(|| {
        KibelClientError::Transport(format!(
            "trusted operation `{}` requires JSON object variables",
            contract.name
        ))
    })?;

    let missing_required = contract
        .required_variables
        .iter()
        .filter(|name| object.get(**name).is_none_or(Value::is_null))
        .copied()
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        return Err(KibelClientError::Transport(format!(
            "trusted operation `{}` missing required variable(s): {}",
            contract.name,
            missing_required.join(", ")
        )));
    }

    let unsupported = object
        .keys()
        .filter(|name| !declared_variables.contains(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(KibelClientError::Transport(format!(
            "trusted operation `{}` has undeclared variable(s): {}",
            contract.name,
            unsupported.join(", ")
        )));
    }

    Ok(())
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

fn extract_declared_variables(query: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let header_end = query.find('{').unwrap_or(query.len());
    let bytes = query.as_bytes();
    let mut index = 0;

    while index < header_end {
        if bytes[index] != b'$' {
            index += 1;
            continue;
        }
        index += 1;
        let start = index;
        while index < header_end {
            let c = bytes[index];
            if c.is_ascii_alphanumeric() || c == b'_' {
                index += 1;
            } else {
                break;
            }
        }
        if index > start {
            if let Ok(name) = std::str::from_utf8(&bytes[start..index]) {
                set.insert(name.to_string());
            }
        }
    }

    set
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

fn parse_note_at(payload: &Value, pointer: &str) -> Result<Note, KibelClientError> {
    let value = payload.pointer(pointer).ok_or_else(|| {
        KibelClientError::Transport(format!("missing `{pointer}` field in GraphQL response"))
    })?;

    serde_json::from_value::<Note>(value.clone())
        .map_err(|err| KibelClientError::Transport(format!("invalid note payload: {err}")))
}

fn parse_create_note_at(payload: &Value, pointer: &str) -> Result<Note, KibelClientError> {
    let value = payload.pointer(pointer).ok_or_else(|| {
        KibelClientError::Transport(format!("missing `{pointer}` field in GraphQL response"))
    })?;
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            KibelClientError::Transport("missing `id` in createNote response".to_string())
        })?
        .to_string();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let content = value
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Ok(Note { id, title, content })
}

fn parse_id_only_at(
    payload: &Value,
    pointer: &str,
    context: &str,
) -> Result<IdOnlyResult, KibelClientError> {
    let id = payload
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| KibelClientError::Transport(format!("missing `id` in {context}")))?
        .to_string();
    Ok(IdOnlyResult { id })
}

fn require_array_at<'a>(
    payload: &'a Value,
    pointer: &str,
    not_found_message: &str,
) -> Result<&'a Vec<Value>, KibelClientError> {
    payload
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| KibelClientError::Api {
            code: "NOT_FOUND".to_string(),
            message: not_found_message.to_string(),
        })
}

fn require_value_at(
    payload: &Value,
    pointer: &str,
    not_found_message: &str,
) -> Result<Value, KibelClientError> {
    let value = payload
        .pointer(pointer)
        .cloned()
        .ok_or_else(|| KibelClientError::Api {
            code: "NOT_FOUND".to_string(),
            message: not_found_message.to_string(),
        })?;

    if value.is_null() {
        return Err(KibelClientError::Api {
            code: "NOT_FOUND".to_string(),
            message: not_found_message.to_string(),
        });
    }
    Ok(value)
}

fn parse_current_user_id(payload: &Value) -> Result<String, KibelClientError> {
    let user = require_value_at(payload, "/data/currentUser", "current user not found")?;
    let id = user
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| KibelClientError::Api {
            code: "NOT_FOUND".to_string(),
            message: "current user id not found".to_string(),
        })?;
    Ok(id.to_string())
}

fn build_search_note_variables(
    input: &SearchNoteInput,
    first: u32,
) -> Result<serde_json::Map<String, Value>, KibelClientError> {
    let mut variables = serde_json::Map::new();
    variables.insert(
        "query".to_string(),
        Value::String(input.query.trim().to_string()),
    );
    variables.insert("first".to_string(), json!(first));

    let resources = normalize_search_note_resources(&input.resources)?;
    if resources.is_empty() {
        variables.insert("resources".to_string(), json!(["NOTE"]));
    } else {
        variables.insert("resources".to_string(), json!(resources));
    }
    if let Some(value) = input.coediting {
        variables.insert("coediting".to_string(), Value::Bool(value));
    }
    if let Some(value) = input.updated.as_deref().and_then(normalize_optional) {
        variables.insert("updated".to_string(), Value::String(value));
    }
    let group_ids = normalize_vec(&input.group_ids);
    if !group_ids.is_empty() {
        variables.insert("groupIds".to_string(), json!(group_ids));
    }
    let user_ids = normalize_vec(&input.user_ids);
    if !user_ids.is_empty() {
        variables.insert("userIds".to_string(), json!(user_ids));
    }
    let folder_ids = normalize_vec(&input.folder_ids);
    if !folder_ids.is_empty() {
        variables.insert("folderIds".to_string(), json!(folder_ids));
    }
    let liker_ids = normalize_vec(&input.liker_ids);
    if !liker_ids.is_empty() {
        variables.insert("likerIds".to_string(), json!(liker_ids));
    }
    if let Some(value) = input.is_archived {
        variables.insert("isArchived".to_string(), Value::Bool(value));
    }
    if let Some(value) = input.sort_by.as_deref().and_then(normalize_optional) {
        variables.insert("sortBy".to_string(), Value::String(value));
    }
    Ok(variables)
}

fn normalize_search_note_resources(resources: &[String]) -> Result<Vec<String>, KibelClientError> {
    let normalized = normalize_vec(resources)
        .into_iter()
        .map(|value| value.to_ascii_uppercase())
        .collect::<Vec<_>>();
    for value in &normalized {
        if !SEARCH_NOTE_RESOURCE_KINDS.contains(&value.as_str()) {
            return Err(KibelClientError::InputInvalid(format!(
                "resource must be one of: {}",
                SEARCH_NOTE_RESOURCE_KINDS.join(", ")
            )));
        }
    }
    Ok(normalized)
}

fn extract_graphql_error(payload: &Value) -> Option<(String, String)> {
    let first = payload
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|errors| errors.first())?;

    let code = first
        .pointer("/extensions/code")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN_ERROR")
        .to_string();

    let message = first
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("GraphQL request failed")
        .to_string();

    Some((code, message))
}

fn normalize_optional(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn normalize_vec(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| normalize_optional(value))
        .collect()
}

fn normalize_first(first: Option<u32>) -> Result<u32, KibelClientError> {
    let value = first.unwrap_or(DEFAULT_FIRST);
    if value == 0 {
        return Err(KibelClientError::InputInvalid(
            "first must be greater than 0".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_folder(folder: &CreateNoteFolderInput) -> Result<Value, KibelClientError> {
    let group_id = folder.group_id.trim();
    let folder_name = folder.folder_name.trim();
    if group_id.is_empty() || folder_name.is_empty() {
        return Err(KibelClientError::InputInvalid(
            "folder requires non-empty group_id and folder_name".to_string(),
        ));
    }
    Ok(json!({
        "groupId": group_id,
        "folderName": folder_name,
    }))
}

#[cfg(any(test, feature = "test-hooks"))]
fn test_capture_request_payload(payload_raw: &str) -> Result<(), KibelClientError> {
    if let Ok(path) = std::env::var("KIBEL_TEST_CAPTURE_REQUEST_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            fs::write(path, payload_raw)
                .map_err(|err| KibelClientError::Transport(err.to_string()))?;
        }
    }
    Ok(())
}

#[cfg(not(any(test, feature = "test-hooks")))]
fn test_capture_request_payload(_payload_raw: &str) -> Result<(), KibelClientError> {
    Ok(())
}

#[cfg(any(test, feature = "test-hooks"))]
fn test_transport_error_message() -> Option<String> {
    std::env::var("KIBEL_TEST_TRANSPORT_ERROR")
        .ok()
        .map(|message| message.trim().to_string())
        .filter(|message| !message.is_empty())
}

#[cfg(not(any(test, feature = "test-hooks")))]
fn test_transport_error_message() -> Option<String> {
    None
}

#[cfg(any(test, feature = "test-hooks"))]
fn load_graphql_response_fixture() -> Result<Option<Value>, KibelClientError> {
    let Some(fixture) = std::env::var("KIBEL_TEST_GRAPHQL_RESPONSE").ok() else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<Value>(&fixture)
        .map_err(|err| KibelClientError::Transport(format!("invalid test fixture JSON: {err}")))?;
    Ok(Some(parsed))
}

#[cfg(not(any(test, feature = "test-hooks")))]
fn load_graphql_response_fixture() -> Result<Option<Value>, KibelClientError> {
    Ok(None)
}

#[cfg(any(test, feature = "test-hooks"))]
fn fixture_response_env_set() -> bool {
    std::env::var("KIBEL_TEST_GRAPHQL_RESPONSE").is_ok()
}

#[cfg(not(any(test, feature = "test-hooks")))]
fn fixture_response_env_set() -> bool {
    false
}

fn should_skip_runtime_introspection() -> bool {
    if fixture_response_env_set() {
        return true;
    }
    if env_flag_is_true("KIBEL_ENABLE_RUNTIME_INTROSPECTION") {
        return false;
    }
    if env_flag_is_true("KIBEL_DISABLE_RUNTIME_INTROSPECTION") {
        return true;
    }
    true
}

fn env_flag_is_true(name: &str) -> bool {
    let Ok(raw) = std::env::var(name) else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

#[cfg(any(test, feature = "test-hooks"))]
fn load_schema_fixture_from_env() -> Option<CreateNoteSchema> {
    let raw = std::env::var("KIBEL_TEST_CREATE_NOTE_SCHEMA_RESPONSE").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let payload = serde_json::from_str::<Value>(trimmed).ok()?;
    CreateNoteSchema::from_introspection(&payload)
}

#[cfg(not(any(test, feature = "test-hooks")))]
fn load_schema_fixture_from_env() -> Option<CreateNoteSchema> {
    None
}

#[derive(Debug, Clone)]
struct CreateNoteSchema {
    input: BTreeSet<String>,
    payload: BTreeSet<String>,
    note: BTreeSet<String>,
}

impl Default for CreateNoteSchema {
    fn default() -> Self {
        Self {
            input: CREATE_NOTE_INPUT_FIELDS
                .iter()
                .copied()
                .map(str::to_string)
                .collect(),
            payload: CREATE_NOTE_PAYLOAD_FIELDS
                .iter()
                .copied()
                .map(str::to_string)
                .collect(),
            note: CREATE_NOTE_NOTE_PROJECTION_FIELDS
                .iter()
                .copied()
                .map(str::to_string)
                .collect(),
        }
    }
}

impl CreateNoteSchema {
    fn from_introspection(payload: &Value) -> Option<Self> {
        let input = collect_name_set(payload.pointer("/data/createNoteInput/inputFields")?);
        let payload_fields = collect_name_set(payload.pointer("/data/createNotePayload/fields")?);
        let note = collect_name_set(payload.pointer("/data/noteType/fields")?);

        if input.is_empty() || payload_fields.is_empty() || note.is_empty() {
            return None;
        }
        if !input.contains("title")
            || !input.contains("content")
            || !input.contains("groupIds")
            || !input.contains("coediting")
        {
            return None;
        }
        if !payload_fields.contains("note") {
            return None;
        }
        if !note.contains("id") {
            return None;
        }

        Some(Self {
            input,
            payload: payload_fields,
            note,
        })
    }

    fn supports_input(&self, field: &str) -> bool {
        self.input.contains(field)
    }

    fn create_note_mutation(&self) -> String {
        let mut payload_lines = Vec::new();
        if self.payload.contains("clientMutationId") {
            payload_lines.push("clientMutationId".to_string());
        }

        let note_fields = self.selected_note_fields();
        let note_block = format!("note {{\n      {}\n    }}", note_fields.join("\n      "));
        payload_lines.push(note_block);

        format!(
            "mutation CreateNote($input: CreateNoteInput!) {{\n  createNote(input: $input) {{\n    {}\n  }}\n}}",
            payload_lines.join("\n    ")
        )
    }

    fn selected_note_fields(&self) -> Vec<String> {
        let mut fields = Vec::new();
        for field in ["id", "title", "content", "url"] {
            if self.note.contains(field) {
                fields.push(field.to_string());
            }
        }
        if fields.is_empty() {
            fields.push("id".to_string());
        }
        fields
    }
}

fn collect_name_set(value: &Value) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if let Some(name) = item.get("name").and_then(Value::as_str) {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    set.insert(trimmed.to_string());
                }
            }
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::{
        build_search_note_variables, collect_name_set, endpoint_from_origin, extract_graphql_error,
        extract_root_field, is_persisted_query_not_found, is_persisted_query_not_supported,
        load_schema_fixture_from_env, parse_create_note_at, parse_current_user_id,
        resource_contract_upstream_commit, resource_contract_version, resource_contracts,
        should_fallback_apq_status, should_skip_runtime_introspection, trusted_operation_contract,
        trusted_operations, validate_trusted_operation_request, CreateNoteInput, CreateNoteSchema,
        KibelClient, SearchNoteInput, TrustedOperation, QUERY_GET_FOLDER, QUERY_NOTE_GET,
    };
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[test]
    fn endpoint_keeps_api_path_when_present() {
        assert_eq!(
            endpoint_from_origin("https://example.kibe.la/api/v1"),
            "https://example.kibe.la/api/v1"
        );
    }

    #[test]
    fn endpoint_appends_api_path_when_missing() {
        assert_eq!(
            endpoint_from_origin("https://example.kibe.la"),
            "https://example.kibe.la/api/v1"
        );
    }

    #[test]
    fn extract_graphql_error_reads_extensions_code() {
        let payload = json!({
            "errors": [{
                "message": "no note",
                "extensions": {
                    "code": "NOT_FOUND"
                }
            }]
        });

        let (code, message) = extract_graphql_error(&payload).expect("error should exist");
        assert_eq!(code, "NOT_FOUND");
        assert_eq!(message, "no note");
    }

    #[test]
    fn create_note_schema_parses_introspection_payload() {
        let payload = json!({
            "data": {
                "createNoteInput": {
                    "inputFields": [
                        { "name": "title" },
                        { "name": "content" },
                        { "name": "groupIds" },
                        { "name": "coediting" }
                    ]
                },
                "createNotePayload": { "fields": [ { "name": "note" }, { "name": "clientMutationId" } ] },
                "noteType": { "fields": [ { "name": "id" }, { "name": "title" }, { "name": "content" } ] },
            }
        });

        let schema = CreateNoteSchema::from_introspection(&payload).expect("schema should parse");
        assert!(schema.supports_input("title"));
        assert!(schema.supports_input("coediting"));

        let mutation = schema.create_note_mutation();
        assert!(mutation.contains("clientMutationId"));
        assert!(mutation.contains("note {"));
    }

    #[test]
    fn create_note_schema_rejects_missing_required_input_fields() {
        let payload = json!({
            "data": {
                "createNoteInput": { "inputFields": [ { "name": "title" }, { "name": "content" }, { "name": "groupIds" } ] },
                "createNotePayload": { "fields": [ { "name": "note" } ] },
                "noteType": { "fields": [ { "name": "id" } ] },
            }
        });
        assert!(CreateNoteSchema::from_introspection(&payload).is_none());
    }

    #[test]
    fn create_note_schema_rejects_missing_required_response_fields() {
        let payload_missing_note = json!({
            "data": {
                "createNoteInput": { "inputFields": [ { "name": "title" }, { "name": "content" }, { "name": "groupIds" }, { "name": "coediting" } ] },
                "createNotePayload": { "fields": [ { "name": "clientMutationId" } ] },
                "noteType": { "fields": [ { "name": "id" } ] },
            }
        });
        assert!(CreateNoteSchema::from_introspection(&payload_missing_note).is_none());

        let payload_missing_note_id = json!({
            "data": {
                "createNoteInput": { "inputFields": [ { "name": "title" }, { "name": "content" }, { "name": "groupIds" }, { "name": "coediting" } ] },
                "createNotePayload": { "fields": [ { "name": "note" } ] },
                "noteType": { "fields": [ { "name": "title" } ] },
            }
        });
        assert!(CreateNoteSchema::from_introspection(&payload_missing_note_id).is_none());
    }

    #[test]
    fn parse_create_note_tolerates_missing_title_and_content() {
        let payload = json!({
            "data": {
                "createNote": {
                    "note": {
                        "id": "N1"
                    }
                }
            }
        });

        let note =
            parse_create_note_at(&payload, "/data/createNote/note").expect("note should parse");
        assert_eq!(note.id, "N1");
        assert_eq!(note.title, "");
        assert_eq!(note.content, "");
    }

    #[test]
    fn should_skip_runtime_introspection_when_fixture_env_exists() {
        std::env::set_var("KIBEL_TEST_GRAPHQL_RESPONSE", "{\"data\":{}}");
        assert!(should_skip_runtime_introspection());
        std::env::remove_var("KIBEL_TEST_GRAPHQL_RESPONSE");
    }

    #[test]
    fn should_skip_runtime_introspection_by_default() {
        std::env::remove_var("KIBEL_ENABLE_RUNTIME_INTROSPECTION");
        std::env::remove_var("KIBEL_DISABLE_RUNTIME_INTROSPECTION");
        std::env::remove_var("KIBEL_TEST_GRAPHQL_RESPONSE");
        assert!(should_skip_runtime_introspection());
    }

    #[test]
    fn should_allow_runtime_introspection_when_explicitly_enabled() {
        std::env::set_var("KIBEL_ENABLE_RUNTIME_INTROSPECTION", "1");
        std::env::remove_var("KIBEL_DISABLE_RUNTIME_INTROSPECTION");
        std::env::remove_var("KIBEL_TEST_GRAPHQL_RESPONSE");
        assert!(!should_skip_runtime_introspection());
        std::env::remove_var("KIBEL_ENABLE_RUNTIME_INTROSPECTION");
    }

    #[test]
    fn apq_error_helpers_detect_known_errors() {
        assert!(is_persisted_query_not_found(
            "PERSISTED_QUERY_NOT_FOUND",
            "PersistedQueryNotFound"
        ));
        assert!(is_persisted_query_not_supported(
            "PERSISTED_QUERY_NOT_SUPPORTED",
            "PersistedQueryNotSupported"
        ));
        assert!(should_fallback_apq_status(Some(405)));
        assert!(!should_fallback_apq_status(Some(200)));
    }

    #[test]
    fn collect_name_set_ignores_empty_values() {
        let names = collect_name_set(&json!([
            { "name": "title" },
            { "name": " " },
            { "other": "ignored" }
        ]));
        assert!(names.contains("title"));
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn load_schema_fixture_from_env_parses_valid_payload() {
        std::env::set_var(
            "KIBEL_TEST_CREATE_NOTE_SCHEMA_RESPONSE",
            json!({
                "data": {
                    "createNoteInput": {
                        "inputFields": [
                            { "name": "title" },
                            { "name": "content" },
                            { "name": "groupIds" },
                            { "name": "coediting" }
                        ]
                    },
                    "createNotePayload": { "fields": [ { "name": "note" } ] },
                    "noteType": { "fields": [ { "name": "id" } ] },
                }
            })
            .to_string(),
        );
        let schema = load_schema_fixture_from_env().expect("schema fixture should parse");
        assert!(schema.supports_input("title"));
        assert!(schema.supports_input("coediting"));
        std::env::remove_var("KIBEL_TEST_CREATE_NOTE_SCHEMA_RESPONSE");
    }

    #[test]
    fn generated_resource_contracts_cover_all_resources() {
        let contracts = resource_contracts();
        assert!(contracts.len() >= 17);
        assert!(contracts.iter().any(|item| item.name == "createNote"));
        assert!(contracts
            .iter()
            .any(|item| item.name == "updateNoteContent"));
    }

    #[test]
    fn generated_resource_contract_metadata_is_set() {
        assert_eq!(resource_contract_version(), 1);
        let contracts = resource_contracts();
        assert!(contracts
            .iter()
            .all(|item| item.graphql_file.starts_with("endpoint:")));
        assert_eq!(resource_contract_upstream_commit(), "");
    }

    #[test]
    fn trusted_operations_cover_all_resource_contracts() {
        let operations = trusted_operations();
        let contracts = resource_contracts();
        assert_eq!(operations.len(), contracts.len());
        for operation in operations {
            let contract = trusted_operation_contract(*operation);
            assert!(contracts.iter().any(|item| item.name == contract.name));
        }
    }

    #[test]
    fn extract_root_field_supports_alias() {
        let query = r#"
query AliasQuery($id: ID!) {
  item: note(id: $id) {
    id
  }
}
"#;
        let root = extract_root_field(query).expect("root should be parsed");
        assert_eq!(root, "note");
    }

    #[test]
    fn trusted_operation_validation_rejects_missing_required_variable() {
        let error = validate_trusted_operation_request(
            TrustedOperation::GetNote,
            QUERY_NOTE_GET,
            &json!({}),
        )
        .expect_err("validation should fail");
        match error {
            super::KibelClientError::Transport(message) => {
                assert!(message.contains("missing required variable(s): id"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn trusted_operation_validation_rejects_unsupported_variable() {
        let error = validate_trusted_operation_request(
            TrustedOperation::GetNote,
            QUERY_NOTE_GET,
            &json!({
                "id": "N1",
                "first": 16,
            }),
        )
        .expect_err("validation should fail");
        match error {
            super::KibelClientError::Transport(message) => {
                assert!(message.contains("undeclared variable(s): first"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn trusted_operation_validation_allows_declared_non_root_variables() {
        validate_trusted_operation_request(
            TrustedOperation::GetFolder,
            QUERY_GET_FOLDER,
            &json!({
                "id": "F1",
                "first": 1,
            }),
        )
        .expect("validation should accept declared variable");
    }

    #[test]
    fn trusted_operation_validation_rejects_root_field_mismatch() {
        let error = validate_trusted_operation_request(
            TrustedOperation::GetNote,
            QUERY_GET_FOLDER,
            &json!({
                "id": "F1",
                "first": 1,
            }),
        )
        .expect_err("validation should fail");
        match error {
            super::KibelClientError::Transport(message) => {
                assert!(message.contains("root field mismatch"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn create_note_rejects_blank_group_ids() {
        let client = KibelClient::new("https://example.kibe.la", "test-token")
            .expect("client should be created");
        let error = client
            .create_note(&CreateNoteInput {
                title: "Title".to_string(),
                content: "Content".to_string(),
                group_ids: vec!["   ".to_string()],
                draft: None,
                coediting: false,
                folders: vec![],
                author_id: None,
                published_at: None,
                client_mutation_id: None,
            })
            .expect_err("blank group id should be rejected");
        match error {
            super::KibelClientError::InputInvalid(message) => {
                assert_eq!(message, "at least one group id is required");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn create_note_normalizes_group_ids_before_request() {
        let capture = NamedTempFile::new().expect("capture temp file should be created");
        let capture_path = capture.path().to_string_lossy().to_string();
        std::env::set_var("KIBEL_TEST_CAPTURE_REQUEST_PATH", &capture_path);

        let client =
            KibelClient::new("http://127.0.0.1:9", "test-token").expect("client should be created");
        let _ = client.create_note(&CreateNoteInput {
            title: "Title".to_string(),
            content: "Content".to_string(),
            group_ids: vec![" G1 ".to_string(), " ".to_string(), "G2".to_string()],
            draft: None,
            coediting: false,
            folders: vec![],
            author_id: None,
            published_at: None,
            client_mutation_id: None,
        });

        let payload =
            std::fs::read_to_string(&capture_path).expect("captured payload should exist");
        let parsed: serde_json::Value =
            serde_json::from_str(&payload).expect("captured payload should be valid json");
        assert_eq!(
            parsed["variables"]["input"]["groupIds"],
            json!(["G1", "G2"])
        );
        std::env::remove_var("KIBEL_TEST_CAPTURE_REQUEST_PATH");
    }

    #[test]
    fn build_search_note_variables_defaults_resource_to_note() {
        let variables = build_search_note_variables(
            &SearchNoteInput {
                query: "".to_string(),
                resources: vec![],
                coediting: None,
                updated: None,
                group_ids: vec![],
                user_ids: vec![],
                folder_ids: vec![],
                liker_ids: vec![],
                is_archived: None,
                sort_by: None,
                first: Some(10),
            },
            10,
        )
        .expect("variables should build");
        assert_eq!(variables.get("query"), Some(&json!("")));
        assert_eq!(variables.get("resources"), Some(&json!(["NOTE"])));
    }

    #[test]
    fn build_search_note_variables_normalizes_user_ids() {
        let variables = build_search_note_variables(
            &SearchNoteInput {
                query: "x".to_string(),
                resources: vec!["note".to_string()],
                coediting: None,
                updated: None,
                group_ids: vec![],
                user_ids: vec![" U1 ".to_string(), " ".to_string(), "U2".to_string()],
                folder_ids: vec![],
                liker_ids: vec![],
                is_archived: None,
                sort_by: None,
                first: Some(10),
            },
            10,
        )
        .expect("variables should build");
        assert_eq!(variables.get("resources"), Some(&json!(["NOTE"])));
        assert_eq!(variables.get("userIds"), Some(&json!(["U1", "U2"])));
    }

    #[test]
    fn build_search_note_variables_rejects_unknown_resource() {
        let error = build_search_note_variables(
            &SearchNoteInput {
                query: "x".to_string(),
                resources: vec!["unknown".to_string()],
                coediting: None,
                updated: None,
                group_ids: vec![],
                user_ids: vec![],
                folder_ids: vec![],
                liker_ids: vec![],
                is_archived: None,
                sort_by: None,
                first: Some(10),
            },
            10,
        )
        .expect_err("unknown resource must fail");
        match error {
            super::KibelClientError::InputInvalid(message) => {
                assert_eq!(
                    message,
                    "resource must be one of: NOTE, COMMENT, ATTACHMENT"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_current_user_id_accepts_valid_payload() {
        let id = parse_current_user_id(&json!({
            "data": {
                "currentUser": {
                    "id": " U1 "
                }
            }
        }))
        .expect("current user id should parse");
        assert_eq!(id, "U1");
    }

    #[test]
    fn parse_current_user_id_rejects_missing_id() {
        let error = parse_current_user_id(&json!({
            "data": {
                "currentUser": {}
            }
        }))
        .expect_err("missing current user id should fail");
        match error {
            super::KibelClientError::Api { code, message } => {
                assert_eq!(code, "NOT_FOUND");
                assert_eq!(message, "current user id not found");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
