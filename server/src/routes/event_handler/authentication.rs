use hyper::http::Uri;
use jsonwebtoken::EncodingKey;
use octocrab::{
    models::{AppId, InstallationId},
    Octocrab,
};
use thiserror::Error;

use super::remote::GitHubActionalbe;

pub fn authenticate<C: GitHubAuthenticator>(
    github_uri: Uri,
    app_id: AppId,
    app_key: EncodingKey,
) -> Result<AuthenticatedClient<C::Next>, C::Error> {
    let client = C::authenticate_app(github_uri, app_id, app_key)?;
    Ok(AuthenticatedClient { client })
}

#[derive(Clone)]
pub struct AuthenticatedClient<C: InstallationAuthenticator> {
    pub client: C,
}

pub trait GitHubAuthenticator {
    type Next: InstallationAuthenticator + Send + Sync;
    type Error: std::error::Error + Sync + Send;

    fn authenticate_app(
        base_uri: Uri,
        app_id: AppId,
        app_key: EncodingKey,
    ) -> Result<Self::Next, Self::Error>;
}

pub trait InstallationAuthenticator: Clone + Send + Sync {
    type Error: std::error::Error + Send + Sync;
    fn for_installation(&self, id: InstallationId) -> Result<impl GitHubActionalbe, Self::Error>;
}

#[derive(Debug, Error)]
pub enum OctocrabAuthenticationError {
    #[error("Error whilst creating the authentication: {0}")]
    Octocrab(#[from] octocrab::Error),
}

impl GitHubAuthenticator for Octocrab {
    type Next = Octocrab;
    type Error = OctocrabAuthenticationError;

    fn authenticate_app(
        base_uri: Uri,
        app_id: AppId,
        app_key: EncodingKey,
    ) -> Result<Self::Next, Self::Error> {
        Ok(Octocrab::builder()
            .base_uri(base_uri)?
            .app(app_id, app_key)
            .build()?)
    }
}

impl InstallationAuthenticator for Octocrab {
    type Error = octocrab::Error;
    fn for_installation(&self, id: InstallationId) -> Result<impl GitHubActionalbe, Self::Error> {
        self.installation(id)
    }
}
