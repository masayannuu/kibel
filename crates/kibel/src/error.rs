use kibel_client::KibelClientError;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InputInvalid,
    AuthFailed,
    NotFound,
    PreconditionFailed,
    IdempotencyConflict,
    ThrottledRetryable,
    ThrottledRewriteRequired,
    TransportError,
    UnknownError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputInvalid => "INPUT_INVALID",
            Self::AuthFailed => "AUTH_FAILED",
            Self::NotFound => "NOT_FOUND",
            Self::PreconditionFailed => "PRECONDITION_FAILED",
            Self::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
            Self::ThrottledRetryable => "THROTTLED_RETRYABLE",
            Self::ThrottledRewriteRequired => "THROTTLED_REWRITE_REQUIRED",
            Self::TransportError => "TRANSPORT_ERROR",
            Self::UnknownError => "UNKNOWN_ERROR",
        }
    }

    pub fn exit_code(self) -> i32 {
        match self {
            Self::InputInvalid => 2,
            Self::AuthFailed => 3,
            Self::NotFound => 4,
            Self::PreconditionFailed | Self::IdempotencyConflict => 5,
            Self::ThrottledRetryable | Self::TransportError => 6,
            Self::ThrottledRewriteRequired => 7,
            Self::UnknownError => 10,
        }
    }

    pub fn retryable(self) -> bool {
        matches!(self, Self::ThrottledRetryable | Self::TransportError)
    }
}

pub fn map_graphql_error(raw_code: &str) -> ErrorCode {
    match raw_code {
        "PRECONDITION_FAILED" => ErrorCode::PreconditionFailed,
        "IDEMPOTENCY_CONFLICT" => ErrorCode::IdempotencyConflict,
        "NOT_FOUND" => ErrorCode::NotFound,
        "UNAUTHENTICATED" | "FORBIDDEN" => ErrorCode::AuthFailed,
        "REQUEST_LIMIT_EXCEEDED" => ErrorCode::ThrottledRewriteRequired,
        "TOKEN_BUDGET_EXHAUSTED" | "TEAM_BUDGET_EXHAUSTED" => ErrorCode::ThrottledRetryable,
        _ => ErrorCode::UnknownError,
    }
}

#[derive(Debug, Clone)]
pub struct CliError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Value,
}

impl CliError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: json!({}),
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }
}

impl From<KibelClientError> for CliError {
    fn from(value: KibelClientError) -> Self {
        match value {
            KibelClientError::InputInvalid(message) => Self::new(ErrorCode::InputInvalid, message),
            KibelClientError::Api { code, message } => {
                let mapped = map_graphql_error(&code);
                Self::new(mapped, message).with_details(json!({ "graphql_code": code }))
            }
            KibelClientError::Transport(message) => Self::new(ErrorCode::TransportError, message),
            KibelClientError::Keychain(message) => Self::new(
                ErrorCode::AuthFailed,
                "failed to access OS credential store",
            )
            .with_details(json!({ "cause": message })),
            other => Self::new(ErrorCode::UnknownError, other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{map_graphql_error, ErrorCode};

    #[test]
    fn graphql_error_mapping_is_stable() {
        let cases = [
            ("UNAUTHENTICATED", ErrorCode::AuthFailed),
            ("FORBIDDEN", ErrorCode::AuthFailed),
            ("NOT_FOUND", ErrorCode::NotFound),
            ("PRECONDITION_FAILED", ErrorCode::PreconditionFailed),
            ("IDEMPOTENCY_CONFLICT", ErrorCode::IdempotencyConflict),
            (
                "REQUEST_LIMIT_EXCEEDED",
                ErrorCode::ThrottledRewriteRequired,
            ),
            ("TOKEN_BUDGET_EXHAUSTED", ErrorCode::ThrottledRetryable),
            ("TEAM_BUDGET_EXHAUSTED", ErrorCode::ThrottledRetryable),
            ("SOMETHING_ELSE", ErrorCode::UnknownError),
        ];

        for (raw, expected) in cases {
            assert_eq!(map_graphql_error(raw), expected, "raw code: {raw}");
        }
    }
}
