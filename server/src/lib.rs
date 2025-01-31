pub mod config;
pub mod routes;

use crate::config::{InternalEndpointConfiguration, WebhookEndpointConfiguration};
use axum::{middleware::from_fn, Router};
use config::GitHubAppConfiguration;
use github_event_handler::authentication::GitHubAppAuthenticator;
pub use routes::metrics::track_metrics;
use tokio::net::TcpListener;
use tracing::instrument;

#[instrument(skip(app_config))]
pub async fn public_app<C: GitHubAppAuthenticator>(
    app_config: GitHubAppConfiguration,
    endpoint_config: WebhookEndpointConfiguration,
) -> Result<(), Box<dyn std::error::Error>>
where
    C::Error: 'static,
    C::Next: 'static,
{
    let routes = Router::new()
        .merge(routes::ui::router())
        .merge(routes::event_handler::router::<C>(app_config, &endpoint_config.path).await?)
        .route_layer(from_fn(track_metrics));

    let listener = {
        let addr = endpoint_config.addr;
        tracing::debug!("listening");
        TcpListener::bind(addr).await?
    };

    Ok(axum::serve(listener, routes).await?)
}

#[instrument]
pub async fn internal_app(
    endpoint_config: InternalEndpointConfiguration,
) -> Result<(), Box<dyn std::error::Error>> {
    let routes = routes::metrics::router();
    let listener = {
        let addr = endpoint_config.addr;
        tracing::debug!("listening");
        TcpListener::bind(addr).await?
    };

    Ok(axum::serve(listener, routes).await?)
}
