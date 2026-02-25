use crate::error::KibelClientError;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "masayannuu";
const PROJECT_APPLICATION: &str = "kibel";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_team: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
    #[serde(default)]
    pub search_note_presets: BTreeMap<String, SearchNotePreset>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchNotePreset {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub coediting: Option<bool>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub user_ids: Vec<String>,
    #[serde(default)]
    pub folder_ids: Vec<String>,
    #[serde(default)]
    pub liker_ids: Vec<String>,
    #[serde(default)]
    pub is_archived: Option<bool>,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub first: Option<u32>,
    #[serde(default)]
    pub after: Option<String>,
}

/// Returns the default config file path.
///
/// # Errors
/// Returns [`KibelClientError::ConfigDirectoryUnavailable`] when the OS config
/// directory cannot be resolved.
pub fn default_config_path() -> Result<PathBuf, KibelClientError> {
    let dirs = ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
        .ok_or(KibelClientError::ConfigDirectoryUnavailable)?;
    Ok(dirs.config_dir().join("config.toml"))
}

impl Config {
    /// Loads config from `path`.
    ///
    /// If the file does not exist, this returns `Config::default()`.
    ///
    /// # Errors
    /// Returns [`KibelClientError::ConfigRead`] on I/O errors and
    /// [`KibelClientError::ConfigParse`] when TOML parsing fails.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, KibelClientError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(path).map_err(KibelClientError::ConfigRead)?;
        let parsed = toml::from_str::<Self>(&raw).map_err(KibelClientError::ConfigParse)?;
        Ok(parsed)
    }

    /// Saves config to `path`, creating parent directories if needed.
    ///
    /// # Errors
    /// Returns [`KibelClientError::ConfigWrite`] for filesystem errors and
    /// [`KibelClientError::ConfigSerialize`] when TOML serialization fails.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), KibelClientError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(KibelClientError::ConfigWrite)?;
        }

        let serialized = toml::to_string_pretty(self).map_err(KibelClientError::ConfigSerialize)?;
        fs::write(path, serialized).map_err(KibelClientError::ConfigWrite)?;
        Ok(())
    }

    #[must_use]
    pub fn token_for_team(&self, team: &str) -> Option<&str> {
        self.profiles
            .get(team)
            .and_then(|profile| profile.token.as_deref())
    }

    #[must_use]
    pub fn origin_for_team(&self, team: &str) -> Option<&str> {
        self.profiles
            .get(team)
            .and_then(|profile| profile.origin.as_deref())
    }

    #[must_use]
    pub fn first_profile_with_token(&self) -> Option<(String, String)> {
        self.profiles
            .iter()
            .find_map(|(team, profile)| profile.token.clone().map(|token| (team.clone(), token)))
    }

    pub fn set_profile_token(&mut self, team: &str, token: &str) {
        let normalized = token.trim();
        if normalized.is_empty() {
            return;
        }

        let profile = self.profiles.entry(team.to_string()).or_default();
        profile.token = Some(normalized.to_string());
    }

    pub fn set_profile_origin(&mut self, team: &str, origin: &str) {
        let normalized = origin.trim();
        if normalized.is_empty() {
            return;
        }

        let profile = self.profiles.entry(team.to_string()).or_default();
        profile.origin = Some(normalized.to_string());
    }

    pub fn clear_profile_token(&mut self, team: &str) -> bool {
        if let Some(profile) = self.profiles.get_mut(team) {
            let had_token = profile.token.is_some();
            profile.token = None;
            return had_token;
        }
        false
    }

    pub fn set_default_team_if_missing(&mut self, team: &str) {
        if self.default_team.is_none() {
            self.default_team = Some(team.to_string());
        }
    }

    pub fn set_default_team(&mut self, team: &str) -> bool {
        let normalized = team.trim();
        if normalized.is_empty() {
            return false;
        }
        self.default_team = Some(normalized.to_string());
        true
    }

    #[must_use]
    pub fn search_note_preset(&self, name: &str) -> Option<&SearchNotePreset> {
        self.search_note_presets.get(name)
    }

    pub fn set_search_note_preset(&mut self, name: &str, preset: SearchNotePreset) -> bool {
        let normalized = name.trim();
        if normalized.is_empty() {
            return false;
        }
        self.search_note_presets
            .insert(normalized.to_string(), preset);
        true
    }

    /// Resolves effective team from request/default config.
    ///
    /// # Examples
    /// ```
    /// use kibel_client::Config;
    ///
    /// let mut config = Config::default();
    /// config.set_default_team("acme");
    ///
    /// assert_eq!(config.resolve_team(Some("spike")).as_deref(), Some("spike"));
    /// assert_eq!(config.resolve_team(None).as_deref(), Some("acme"));
    /// ```
    pub fn resolve_team(&self, requested_team: Option<&str>) -> Option<String> {
        requested_team
            .map(str::trim)
            .filter(|team| !team.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.default_team.clone())
    }

    /// Resolves origin using explicit request first, then team profile.
    ///
    /// # Examples
    /// ```
    /// use kibel_client::Config;
    ///
    /// let mut config = Config::default();
    /// config.set_default_team("acme");
    /// config.set_profile_origin("acme", "https://acme.kibe.la");
    ///
    /// assert_eq!(
    ///     config.resolve_origin(None, None).as_deref(),
    ///     Some("https://acme.kibe.la")
    /// );
    /// assert_eq!(
    ///     config.resolve_origin(Some("https://override.kibe.la"), Some("acme")).as_deref(),
    ///     Some("https://override.kibe.la")
    /// );
    /// ```
    pub fn resolve_origin(
        &self,
        requested_origin: Option<&str>,
        requested_team: Option<&str>,
    ) -> Option<String> {
        if let Some(origin) = normalize_non_empty(requested_origin) {
            return Some(origin.to_string());
        }

        let team = self.resolve_team(requested_team)?;
        self.origin_for_team(&team)
            .and_then(|origin| normalize_non_empty(Some(origin)))
            .map(ToOwned::to_owned)
    }
}

fn normalize_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{Config, SearchNotePreset};

    #[test]
    fn resolve_origin_prefers_requested_value() {
        let mut config = Config {
            default_team: Some("acme".to_string()),
            ..Config::default()
        };
        config.set_profile_origin("acme", "https://acme.kibe.la");

        let resolved = config.resolve_origin(Some("https://override.kibe.la"), Some("acme"));
        assert_eq!(resolved.as_deref(), Some("https://override.kibe.la"));
    }

    #[test]
    fn resolve_origin_falls_back_to_default_team_profile() {
        let mut config = Config {
            default_team: Some("acme".to_string()),
            ..Config::default()
        };
        config.set_profile_origin("acme", "https://acme.kibe.la");

        let resolved = config.resolve_origin(None, None);
        assert_eq!(resolved.as_deref(), Some("https://acme.kibe.la"));
    }

    #[test]
    fn resolve_origin_returns_none_without_requested_or_profile_origin() {
        let config = Config {
            default_team: Some("acme".to_string()),
            ..Config::default()
        };
        let resolved = config.resolve_origin(None, None);
        assert!(resolved.is_none());
    }

    #[test]
    fn set_default_team_rejects_empty_values() {
        let mut config = Config::default();
        assert!(!config.set_default_team("   "));
        assert!(config.default_team.is_none());
    }

    #[test]
    fn search_note_preset_round_trip() {
        let mut config = Config::default();
        assert!(!config.set_search_note_preset("   ", SearchNotePreset::default()));
        assert!(config.search_note_preset("daily").is_none());

        let preset = SearchNotePreset {
            query: "onboarding".to_string(),
            first: Some(10),
            ..SearchNotePreset::default()
        };
        assert!(config.set_search_note_preset("daily", preset));
        let loaded = config
            .search_note_preset("daily")
            .expect("preset should be set");
        assert_eq!(loaded.query, "onboarding");
        assert_eq!(loaded.first, Some(10));
    }
}
