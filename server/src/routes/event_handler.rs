use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing::any, Router};

use axum_core::extract::FromRef;
use hyper::StatusCode;
use orion::hazardous::mac::hmac::sha256::SecretKey;

use crate::{config::GitHubAppConfiguration, routes::event_handler::remote::GitHubActionalbe};

use self::extractors::GitHubEvent;

pub use authentication::{AuthenticatedClient, GitHubAuthenticator, InstallationAuthenticator};

mod authentication;
mod extractors;
mod remote;

pub fn router<C: GitHubAuthenticator>(
    config: GitHubAppConfiguration,
) -> Result<Router, Box<dyn std::error::Error>>
where
    C::Error: 'static,
    C::Next: 'static,
{
    let client =
        authentication::authenticate::<C>(config.uri, config.app_identifier, config.app_key)?;
    let signature_config = ConfigState {
        webhook_secret: config.webhook_secret.into(),
        client,
    };
    Ok(Router::new().route(
        "/event_handler",
        any(handle_github_event).with_state(signature_config),
    ))
}

#[derive(Clone)]
struct ConfigState<C: InstallationAuthenticator + Clone> {
    webhook_secret: Arc<SecretKey>,
    client: AuthenticatedClient<C>,
}

impl<C: InstallationAuthenticator + Clone> FromRef<ConfigState<C>> for Arc<SecretKey> {
    fn from_ref(input: &ConfigState<C>) -> Self {
        input.webhook_secret.clone()
    }
}

impl<C: InstallationAuthenticator + Clone> FromRef<ConfigState<C>> for AuthenticatedClient<C> {
    fn from_ref(input: &ConfigState<C>) -> Self {
        input.client.clone()
    }
}

async fn handle_github_event<C: InstallationAuthenticator + Clone>(
    State(AuthenticatedClient { client }): State<AuthenticatedClient<C>>,
    GitHubEvent(event): GitHubEvent,
) -> impl IntoResponse {
    tracing::error!(kind = ?event, "logic starts now");
    if let Some(t) = event.installation {
        let id = match t {
            octocrab::models::webhook_events::EventInstallation::Full(install) => install.id,
            octocrab::models::webhook_events::EventInstallation::Minimal(mini) => mini.id,
        };
        let Ok(client) = client.for_installation(id) else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to authenticate installation",
            )
                .into_response();
        };
        client.post_message();
    }
    (StatusCode::OK, "hello world").into_response()
}

#[cfg(test)]
mod test {
    use axum::{body::Body, http::Request};
    use futures_util::never::Never;
    use http_body_util::BodyExt;
    use hyper::{StatusCode, Uri};
    use orion::hazardous::mac::hmac::sha256::{HmacSha256, SecretKey};
    use rsa::RsaPublicKey;
    use serde_json::json;
    use thiserror::Error;
    use tower::ServiceExt;

    use crate::config::GitHubAppConfiguration;

    use super::{remote::GitHubActionalbe, GitHubAuthenticator, InstallationAuthenticator};

    #[derive(Clone)]
    struct TestClient;

    #[derive(Debug, Error)]
    enum TestError {}

    struct NoOpActionable;

    impl GitHubActionalbe for NoOpActionable {
        fn post_message(&self) {
            todo!()
        }
    }

    impl GitHubAuthenticator for TestClient {
        type Next = TestClient;
        type Error = TestError;

        fn authenticate_app(
            _uri: Uri,
            _app_id: octocrab::models::AppId,
            _app_key: jsonwebtoken::EncodingKey,
        ) -> Result<Self::Next, Self::Error> {
            Ok(TestClient)
        }
    }

    impl InstallationAuthenticator for TestClient {
        type Error = Never;
        fn for_installation(
            &self,
            _id: octocrab::models::InstallationId,
        ) -> Result<impl GitHubActionalbe, Self::Error> {
            Ok(NoOpActionable)
        }
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_happy_path() {
        let (config, _, secret) = create_test_config();
        let app = super::router::<TestClient>(config).unwrap();

        let body = serde_json::to_vec(&json!({"hello": "world"})).unwrap();
        let body_hmac = calc_hmac_for_body(&secret, &body);
        let request = Request::builder()
            .uri("/event_handler")
            .header("X-GitHub-Event", "pull_request.*")
            .header("x-hub-signature-256", format!("sha256={body_hmac}"))
            .body(Body::from(body))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        let (parts, body) = response.into_parts();
        let body = body.collect().await.unwrap().to_bytes();
        // let body: serde_json::Value = str::get(&body).unwrap();
        tracing::info!(?body);
        assert_eq!(parts.status, StatusCode::OK);
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_missing_signature() {
        let (config, _, _) = create_test_config();
        let app = super::router::<TestClient>(config).unwrap();

        let body = serde_json::to_vec(&json!({"hello": "world"})).unwrap();
        let request = Request::builder()
            .uri("/event_handler")
            .header("X-GitHub-Event", "pull_request.*")
            .body(Body::from(body))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_wrong_signature() {
        let (config, _, _) = create_test_config();
        let app = super::router::<TestClient>(config).unwrap();

        let body = serde_json::to_vec(&json!({"hello": "world"})).unwrap();
        let request = Request::builder()
            .uri("/event_handler")
            .header("X-GitHub-Event", "pull_request.*")
            .header(
                "x-hub-signature-256",
                "sha256=46288437613044114D21E7FAD79837C12336202F4C85008548FB226693426F56",
            )
            .body(Body::from(body))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    fn create_test_config() -> (GitHubAppConfiguration, RsaPublicKey, SecretKey) {
        use jsonwebtoken::EncodingKey;
        use octocrab::models::AppId;
        use rand::SeedableRng;
        use rsa::pkcs8::EncodePrivateKey;

        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(17_832_551);
        let bits = 256;
        let priv_key = rsa::RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
        let pub_key = rsa::RsaPublicKey::from(&priv_key);

        let der_encoded_key = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let cert_pem_str = der_encoded_key.to_string();

        let secret = SecretKey::from_slice(&[0; 32]).unwrap();

        (
            GitHubAppConfiguration {
                webhook_secret: secret,
                app_identifier: AppId(1),
                app_key: { EncodingKey::from_rsa_pem(cert_pem_str.as_bytes()).unwrap() },
                uri: Uri::from_static("https://github.local"),
            },
            pub_key,
            SecretKey::from_slice(&[0; 32]).unwrap(),
        )
    }

    fn calc_hmac_for_body(secret: &SecretKey, data: &[u8]) -> String {
        hex::encode(
            HmacSha256::hmac(secret, data)
                .unwrap()
                .unprotected_as_bytes(),
        )
    }
}
