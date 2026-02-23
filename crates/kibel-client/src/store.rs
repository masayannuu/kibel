use crate::error::KibelClientError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

pub trait TokenStore {
    /// Returns token for `team`.
    ///
    /// # Errors
    /// Returns backend-specific storage errors.
    fn get_token(&self, team: &str) -> Result<Option<String>, KibelClientError>;
    /// Persists token for `team`.
    ///
    /// # Errors
    /// Returns backend-specific storage errors.
    fn set_token(&self, team: &str, token: &str) -> Result<(), KibelClientError>;
    /// Deletes token for `team`.
    ///
    /// # Errors
    /// Returns backend-specific storage errors.
    fn delete_token(&self, team: &str) -> Result<(), KibelClientError>;
}

#[derive(Debug, Clone)]
pub struct KeychainTokenStore {
    service: String,
}

impl Default for KeychainTokenStore {
    fn default() -> Self {
        Self {
            service: "com.masayannuu.kibel.access-token".to_string(),
        }
    }
}

impl KeychainTokenStore {
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry_for_team(&self, team: &str) -> Result<keyring::Entry, KibelClientError> {
        keyring::Entry::new(&self.service, team).map_err(KibelClientError::from)
    }
}

impl TokenStore for KeychainTokenStore {
    fn get_token(&self, team: &str) -> Result<Option<String>, KibelClientError> {
        let entry = self.entry_for_team(team)?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(KibelClientError::from(err)),
        }
    }

    fn set_token(&self, team: &str, token: &str) -> Result<(), KibelClientError> {
        let entry = self.entry_for_team(team)?;
        entry.set_password(token).map_err(KibelClientError::from)
    }

    fn delete_token(&self, team: &str) -> Result<(), KibelClientError> {
        let entry = self.entry_for_team(team)?;
        match entry.delete_password() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(KibelClientError::from(err)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTokenStore {
    tokens: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemoryTokenStore {
    /// Seeds an in-memory token for tests.
    ///
    /// # Errors
    /// Returns an error if the internal mutex is poisoned.
    pub fn insert_token(&self, team: &str, token: &str) -> Result<(), KibelClientError> {
        let mut lock = self.lock_tokens()?;
        lock.insert(team.to_string(), token.to_string());
        Ok(())
    }

    fn lock_tokens(&self) -> Result<MutexGuard<'_, HashMap<String, String>>, KibelClientError> {
        self.tokens.lock().map_err(|_| {
            KibelClientError::Transport("in-memory token store lock poisoned".to_string())
        })
    }
}

impl TokenStore for InMemoryTokenStore {
    fn get_token(&self, team: &str) -> Result<Option<String>, KibelClientError> {
        let lock = self.lock_tokens()?;
        Ok(lock.get(team).cloned())
    }

    fn set_token(&self, team: &str, token: &str) -> Result<(), KibelClientError> {
        let mut lock = self.lock_tokens()?;
        lock.insert(team.to_string(), token.to_string());
        Ok(())
    }

    fn delete_token(&self, team: &str) -> Result<(), KibelClientError> {
        let mut lock = self.lock_tokens()?;
        lock.remove(team);
        Ok(())
    }
}
