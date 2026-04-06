pub mod builder;
pub mod relayer_key;

use reqwest::header::HeaderMap;

/// Authentication method for the relayer.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Builder Program HMAC-SHA256 authentication.
    Builder(BuilderConfig),
    /// Simple Relayer API key authentication.
    RelayerKey { api_key: String, address: String },
}

impl AuthMethod {
    /// Create a Builder auth method.
    pub fn builder(key: &str, secret: &str, passphrase: &str) -> Self {
        AuthMethod::Builder(BuilderConfig {
            key: key.to_string(),
            secret: secret.to_string(),
            passphrase: passphrase.to_string(),
        })
    }

    /// Create a Relayer Key auth method.
    pub fn relayer_key(api_key: &str, address: &str) -> Self {
        AuthMethod::RelayerKey {
            api_key: api_key.to_string(),
            address: address.to_string(),
        }
    }

    /// Generate auth headers for a request.
    pub fn headers(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> crate::error::Result<HeaderMap> {
        match self {
            AuthMethod::Builder(config) => builder::build_headers(config, method, path, body),
            AuthMethod::RelayerKey { api_key, address } => {
                relayer_key::build_headers(api_key, address)
            }
        }
    }
}

/// Builder Program API key credentials.
#[derive(Debug, Clone)]
pub struct BuilderConfig {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}
