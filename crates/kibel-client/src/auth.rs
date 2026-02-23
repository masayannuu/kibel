use crate::config::Config;
use crate::error::KibelClientError;
use crate::store::TokenStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Stdin,
    Env,
    Keychain,
    Config,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenResolution {
    pub token: String,
    pub source: TokenSource,
    pub team: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolveTokenInput {
    pub requested_team: Option<String>,
    pub stdin_token: Option<String>,
    pub env_token: Option<String>,
}

/// Returns a stable label string for token source reporting.
#[must_use]
pub fn token_source_label(source: TokenSource) -> &'static str {
    match source {
        TokenSource::Stdin => "stdin",
        TokenSource::Env => "env",
        TokenSource::Keychain => "keychain",
        TokenSource::Config => "config",
    }
}

/// Resolves effective team from request/config and fails when absent.
///
/// # Errors
/// Returns [`KibelClientError::InputInvalid`] when neither `requested_team` nor
/// `config.default_team` is available.
pub fn require_team(
    requested_team: Option<&str>,
    config: &Config,
) -> Result<String, KibelClientError> {
    config.resolve_team(requested_team).ok_or_else(|| {
        KibelClientError::InputInvalid(
            "team is required (--team or config.default_team)".to_string(),
        )
    })
}

/// Resolves an access token using fixed precedence:
/// stdin > env > keychain > config.
///
/// # Errors
/// Returns underlying store/config errors while resolving token candidates.
pub fn resolve_access_token(
    input: &ResolveTokenInput,
    config: &Config,
    store: &dyn TokenStore,
) -> Result<Option<TokenResolution>, KibelClientError> {
    let resolved_team = config.resolve_team(input.requested_team.as_deref());

    if let Some(token) = normalize_optional(input.stdin_token.as_deref()) {
        return Ok(Some(TokenResolution {
            token,
            source: TokenSource::Stdin,
            team: resolved_team,
        }));
    }

    if let Some(token) = normalize_optional(input.env_token.as_deref()) {
        return Ok(Some(TokenResolution {
            token,
            source: TokenSource::Env,
            team: resolved_team,
        }));
    }

    if let Some(team) = resolved_team.clone() {
        if let Some(token) = store.get_token(&team)?.as_deref().and_then(normalize_owned) {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Keychain,
                team: Some(team),
            }));
        }

        if let Some(token) = config.token_for_team(&team).and_then(normalize_borrowed) {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Config,
                team: Some(team),
            }));
        }

        return Ok(None);
    }

    for team in config.profiles.keys() {
        if let Some(token) = store.get_token(team)?.as_deref().and_then(normalize_owned) {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Keychain,
                team: Some(team.clone()),
            }));
        }
    }

    if let Some((team, token)) = config.first_profile_with_token() {
        if let Some(token) = normalize_owned(&token) {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Config,
                team: Some(team),
            }));
        }
    }

    Ok(None)
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value.and_then(normalize_owned)
}

fn normalize_owned(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn normalize_borrowed(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_access_token, ResolveTokenInput, TokenSource};
    use crate::config::Config;
    use crate::store::InMemoryTokenStore;

    fn seed_config() -> Config {
        let mut config = Config {
            default_team: Some("acme".to_string()),
            ..Config::default()
        };
        config.set_profile_token("acme", "config-token");
        config
    }

    #[test]
    fn resolve_prefers_stdin_first() {
        let config = seed_config();
        let store = InMemoryTokenStore::default();
        store
            .insert_token("acme", "keychain-token")
            .expect("seed token should succeed");

        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                stdin_token: Some("stdin-token".to_string()),
                env_token: Some("env-token".to_string()),
            },
            &config,
            &store,
        )
        .expect("resolve should succeed")
        .expect("token should exist");

        assert_eq!(result.source, TokenSource::Stdin);
        assert_eq!(result.token, "stdin-token");
    }

    #[test]
    fn resolve_prefers_env_over_keychain_and_config() {
        let config = seed_config();
        let store = InMemoryTokenStore::default();
        store
            .insert_token("acme", "keychain-token")
            .expect("seed token should succeed");

        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                stdin_token: None,
                env_token: Some("env-token".to_string()),
            },
            &config,
            &store,
        )
        .expect("resolve should succeed")
        .expect("token should exist");

        assert_eq!(result.source, TokenSource::Env);
        assert_eq!(result.token, "env-token");
    }

    #[test]
    fn resolve_prefers_keychain_over_config() {
        let config = seed_config();
        let store = InMemoryTokenStore::default();
        store
            .insert_token("acme", "keychain-token")
            .expect("seed token should succeed");

        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                stdin_token: None,
                env_token: None,
            },
            &config,
            &store,
        )
        .expect("resolve should succeed")
        .expect("token should exist");

        assert_eq!(result.source, TokenSource::Keychain);
        assert_eq!(result.token, "keychain-token");
    }

    #[test]
    fn resolve_falls_back_to_config_when_keychain_missing() {
        let config = seed_config();
        let store = InMemoryTokenStore::default();

        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                stdin_token: None,
                env_token: None,
            },
            &config,
            &store,
        )
        .expect("resolve should succeed")
        .expect("token should exist");

        assert_eq!(result.source, TokenSource::Config);
        assert_eq!(result.token, "config-token");
    }
}
