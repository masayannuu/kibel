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
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolveTokenInput {
    pub requested_team: Option<String>,
    pub requested_origin: Option<String>,
    pub stdin_token: Option<String>,
    pub env_token: Option<String>,
}

/// Returns a stable label string for token source reporting.
///
/// # Examples
/// ```
/// use kibel_client::{token_source_label, TokenSource};
///
/// assert_eq!(token_source_label(TokenSource::Env), "env");
/// ```
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
/// # Examples
/// ```
/// use kibel_client::{require_team, Config};
///
/// let mut config = Config::default();
/// assert_eq!(require_team(Some("acme"), &config).unwrap(), "acme");
///
/// config.set_default_team("spike");
/// assert_eq!(require_team(None, &config).unwrap(), "spike");
/// ```
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

/// Returns tenant-aware keychain subject for token storage.
///
/// Subject format:
/// - with origin: `origin::<normalized-origin>::team::<team>`
/// - legacy fallback: `<team>`
///
/// # Examples
/// ```
/// use kibel_client::token_store_subject;
///
/// assert_eq!(
///     token_store_subject("acme", Some("https://ACME.kibe.la/")),
///     "origin::https://acme.kibe.la::team::acme"
/// );
/// assert_eq!(token_store_subject("acme", None), "acme");
/// ```
#[must_use]
pub fn token_store_subject(team: &str, origin: Option<&str>) -> String {
    match normalize_origin(origin) {
        Some(normalized_origin) => format!("origin::{normalized_origin}::team::{team}"),
        None => team.to_string(),
    }
}

/// Resolves an access token using fixed precedence:
/// stdin > env > keychain > config.
///
/// # Examples
/// ```
/// use kibel_client::{resolve_access_token, Config, InMemoryTokenStore, ResolveTokenInput};
///
/// let config = Config::default();
/// let store = InMemoryTokenStore::default();
/// let input = ResolveTokenInput {
///     env_token: Some("env-token".to_string()),
///     ..ResolveTokenInput::default()
/// };
///
/// let resolved = resolve_access_token(&input, &config, &store)
///     .unwrap()
///     .unwrap();
/// assert_eq!(resolved.token, "env-token");
/// ```
///
/// # Errors
/// Returns config-related errors. Keychain read errors are ignored to allow
/// config fallback on server environments without credential-store support.
pub fn resolve_access_token(
    input: &ResolveTokenInput,
    config: &Config,
    store: &dyn TokenStore,
) -> Result<Option<TokenResolution>, KibelClientError> {
    let resolved_team = config.resolve_team(input.requested_team.as_deref());
    let requested_origin = input
        .requested_origin
        .as_deref()
        .and_then(normalize_borrowed);
    let resolved_origin =
        config.resolve_origin(requested_origin.as_deref(), resolved_team.as_deref());

    if let Some(token) = normalize_optional(input.stdin_token.as_deref()) {
        return Ok(Some(TokenResolution {
            token,
            source: TokenSource::Stdin,
            team: resolved_team,
            origin: resolved_origin,
        }));
    }

    if let Some(token) = normalize_optional(input.env_token.as_deref()) {
        return Ok(Some(TokenResolution {
            token,
            source: TokenSource::Env,
            team: resolved_team,
            origin: resolved_origin,
        }));
    }

    if let Some(team) = resolved_team.clone() {
        if let Some(token) = read_keychain_token(store, &team, resolved_origin.as_deref())? {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Keychain,
                team: Some(team),
                origin: resolved_origin,
            }));
        }

        if let Some(token) = config.token_for_team(&team).and_then(normalize_borrowed) {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Config,
                team: Some(team),
                origin: resolved_origin,
            }));
        }

        return Ok(None);
    }

    for team in config.profiles.keys() {
        let team_origin = config.origin_for_team(team).and_then(normalize_borrowed);
        if let Some(token) = read_keychain_token(store, team, team_origin.as_deref())? {
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Keychain,
                team: Some(team.clone()),
                origin: team_origin,
            }));
        }
    }

    if let Some((team, token)) = config.first_profile_with_token() {
        if let Some(token) = normalize_owned(&token) {
            let origin = config.origin_for_team(&team).and_then(normalize_borrowed);
            return Ok(Some(TokenResolution {
                token,
                source: TokenSource::Config,
                team: Some(team),
                origin,
            }));
        }
    }

    Ok(None)
}

fn read_keychain_token(
    store: &dyn TokenStore,
    team: &str,
    origin: Option<&str>,
) -> Result<Option<String>, KibelClientError> {
    for subject in token_store_lookup_subjects(team, origin) {
        // Keychain backend may be unavailable in server environments.
        // Fallback to next candidate and eventually config instead of hard-failing.
        let candidate = match store.get_token(&subject) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(token) = candidate.as_deref().and_then(normalize_owned) {
            return Ok(Some(token));
        }
    }

    Ok(None)
}

fn token_store_lookup_subjects(team: &str, origin: Option<&str>) -> Vec<String> {
    let mut subjects = Vec::new();
    if let Some(normalized_origin) = normalize_origin(origin) {
        subjects.push(token_store_subject(team, Some(&normalized_origin)));
    }
    subjects.push(team.to_string());
    subjects
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

fn normalize_origin(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }

    let without_trailing = raw.trim_end_matches('/');
    if without_trailing.is_empty() {
        return None;
    }

    Some(without_trailing.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{resolve_access_token, token_store_subject, ResolveTokenInput, TokenSource};
    use crate::config::Config;
    use crate::error::KibelClientError;
    use crate::store::{InMemoryTokenStore, TokenStore};

    struct ErrorTokenStore;

    impl TokenStore for ErrorTokenStore {
        fn get_token(&self, _team: &str) -> Result<Option<String>, KibelClientError> {
            Err(KibelClientError::Keychain(
                "simulated keychain unavailable".to_string(),
            ))
        }

        fn set_token(&self, _team: &str, _token: &str) -> Result<(), KibelClientError> {
            Ok(())
        }

        fn delete_token(&self, _team: &str) -> Result<(), KibelClientError> {
            Ok(())
        }
    }

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
                requested_origin: Some("https://acme.kibe.la".to_string()),
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
                requested_origin: Some("https://acme.kibe.la".to_string()),
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
        let key_subject = token_store_subject("acme", Some("https://acme.kibe.la"));
        store
            .insert_token(&key_subject, "keychain-token")
            .expect("seed token should succeed");

        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                requested_origin: Some("https://acme.kibe.la".to_string()),
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
                requested_origin: Some("https://acme.kibe.la".to_string()),
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

    #[test]
    fn resolve_keychain_falls_back_to_legacy_team_subject() {
        let config = seed_config();
        let store = InMemoryTokenStore::default();
        store
            .insert_token("acme", "legacy-keychain-token")
            .expect("seed token should succeed");

        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                requested_origin: Some("https://acme.kibe.la".to_string()),
                stdin_token: None,
                env_token: None,
            },
            &config,
            &store,
        )
        .expect("resolve should succeed")
        .expect("token should exist");

        assert_eq!(result.source, TokenSource::Keychain);
        assert_eq!(result.token, "legacy-keychain-token");
    }

    #[test]
    fn token_store_subject_uses_origin_when_available() {
        let subject = token_store_subject("acme", Some("https://acme.kibe.la/"));
        assert_eq!(
            subject,
            "origin::https://acme.kibe.la::team::acme".to_string()
        );
    }

    #[test]
    fn resolve_falls_back_to_config_when_keychain_backend_errors() {
        let config = seed_config();
        let store = ErrorTokenStore;
        let result = resolve_access_token(
            &ResolveTokenInput {
                requested_team: Some("acme".to_string()),
                requested_origin: Some("https://acme.kibe.la".to_string()),
                stdin_token: None,
                env_token: None,
            },
            &config,
            &store,
        )
        .expect("resolve should not hard-fail on keychain backend error")
        .expect("token should resolve from config");
        assert_eq!(result.source, TokenSource::Config);
        assert_eq!(result.token, "config-token");
    }
}
