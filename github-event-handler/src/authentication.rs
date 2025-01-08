use crate::api::GitHubApi;
use hyper::http::Uri;
use jsonwebtoken::EncodingKey;
use octocrab::{
    models::{AppId, InstallationId},
    Octocrab,
};
use snafu::{ResultExt, Snafu};
use std::fmt::Debug;
use std::future::Future;

#[derive(Clone)]
pub struct AuthenticatedClient<C: InstallationAuthenticator> {
    pub client: C,
}

pub trait GitHubAppAuthenticator {
    type Next: InstallationAuthenticator + Send + Sync;
    type Error: std::error::Error + Sync + Send;

    fn authenticate_app(
        base_uri: Uri,
        app_id: AppId,
        app_key: EncodingKey,
    ) -> Result<Self::Next, Self::Error>;
}

pub trait InstallationAuthenticator: Clone + Send + Sync {
    type Error: std::error::Error + Send + Sync + Debug + 'static;
    fn for_installation(
        &self,
        id: InstallationId,
    ) -> impl Future<Output = Result<impl GitHubApi, Self::Error>> + Send;
}

#[derive(Debug, Snafu)]
pub enum OctocrabAuthenticationError {
    #[snafu(display("Error whilst creating the authentication: {source}"))]
    Octocrab { source: octocrab::Error },
}

impl GitHubAppAuthenticator for Octocrab {
    type Next = Octocrab;
    type Error = OctocrabAuthenticationError;

    fn authenticate_app(
        base_uri: Uri,
        app_id: AppId,
        app_key: EncodingKey,
    ) -> Result<Self::Next, Self::Error> {
        Octocrab::builder()
            .base_uri(base_uri)
            .context(OctocrabSnafu)?
            .app(app_id, app_key)
            .build()
            .context(OctocrabSnafu)
    }
}

impl InstallationAuthenticator for Octocrab {
    type Error = octocrab::Error;
    async fn for_installation(&self, id: InstallationId) -> Result<impl GitHubApi, Self::Error> {
        self.installation_and_token(id).await.map(|r| r.0)
    }
}
