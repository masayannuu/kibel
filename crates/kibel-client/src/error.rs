use thiserror::Error;

#[derive(Debug, Error)]
pub enum KibelClientError {
    #[error("input invalid: {0}")]
    InputInvalid(String),
    #[error("configuration directory is unavailable")]
    ConfigDirectoryUnavailable,
    #[error("failed to read config file: {0}")]
    ConfigRead(#[source] std::io::Error),
    #[error("failed to write config file: {0}")]
    ConfigWrite(#[source] std::io::Error),
    #[error("failed to parse config file: {0}")]
    ConfigParse(#[source] toml::de::Error),
    #[error("failed to serialize config file: {0}")]
    ConfigSerialize(#[source] toml::ser::Error),
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("api error [{code}]: {message}")]
    Api { code: String, message: String },
    #[error("transport error: {0}")]
    Transport(String),
}

impl From<keyring::Error> for KibelClientError {
    fn from(value: keyring::Error) -> Self {
        Self::Keychain(value.to_string())
    }
}
