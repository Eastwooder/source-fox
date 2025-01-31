use axum::http::uri::InvalidUri;
use envious::EnvDeserializationError;
use hyper::Uri;
use jsonwebtoken::EncodingKey;
use octocrab::models::AppId;
use orion::{errors::UnknownCryptoError, hazardous::mac::hmac::sha256::SecretKey};
use std::net::{IpAddr, SocketAddr};
use thiserror::Error;

pub fn load_github_app_config() -> Result<
    (
        GitHubAppConfiguration,
        WebhookEndpointConfiguration,
        InternalEndpointConfiguration,
    ),
    ConfigurationError,
> {
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    struct ApplicationRawConfig {
        github_private_key: String,
        github_webhook_secret: String,
        github_app_identifier: u64,
        github_uri: String,
        webhook_addr: Option<SocketAddr>,
        webhook_endpoint: Option<String>,
        internal_addr: Option<SocketAddr>,
    }

    let raw_config: ApplicationRawConfig = {
        let mut env_config = envious::Config::new();
        env_config.case_sensitive(false);
        env_config.build_from_env()?
    };

    let webhook_secret = SecretKey::from_slice(raw_config.github_webhook_secret.as_bytes())?;
    let app_identifier = AppId(raw_config.github_app_identifier);
    let app_key = EncodingKey::from_rsa_pem(raw_config.github_private_key.as_bytes())?;
    let uri = Uri::try_from(raw_config.github_uri)?;

    let app_config = GitHubAppConfiguration {
        webhook_secret,
        app_identifier,
        app_key,
        uri,
    };
    let public_ep_config = WebhookEndpointConfiguration {
        addr: raw_config
            .webhook_addr
            .unwrap_or(SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 3000)),
        path: raw_config
            .webhook_endpoint
            .unwrap_or("/event_handler".into()),
    };
    let internal_ep_config = InternalEndpointConfiguration {
        addr: raw_config
            .internal_addr
            .unwrap_or(SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 3001)),
    };
    Ok((app_config, public_ep_config, internal_ep_config))
}

pub struct GitHubAppConfiguration {
    pub webhook_secret: SecretKey,
    pub app_identifier: AppId,
    pub app_key: EncodingKey,
    pub uri: Uri,
}

#[derive(Debug)]
pub struct WebhookEndpointConfiguration {
    pub addr: SocketAddr,
    pub path: String,
}

#[derive(Debug)]
pub struct InternalEndpointConfiguration {
    pub addr: SocketAddr,
}

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("Cannot read from environment configuration")]
    EnvironmentConfigNotReadable(#[from] EnvDeserializationError),
    #[error("Unable to read the cryptocraphical key")]
    UnsupportedCryptography(#[from] UnknownCryptoError),
    #[error("Invalid RSA Key")]
    InvalidRsaError(#[from] jsonwebtoken::errors::Error),
    #[error("Provided base uri is invalid: {0}")]
    InvalidUri(#[from] InvalidUri),
}
