pub mod config;
pub mod routes;

use std::net::SocketAddr;

use axum::{middleware::from_fn, Router};
use config::GitHubAppConfiguration;
pub use routes::metrics::track_metrics;
use tokio::net::TcpListener;

pub async fn public_app(app_config: GitHubAppConfiguration) -> Result<(), std::io::Error> {
    let routes = Router::new()
        .merge(routes::ui::router())
        .merge(routes::event_handler::router(app_config))
        .route_layer(from_fn(track_metrics));

    let listener = {
        let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
        tracing::debug!("going to listen on {}", addr);
        TcpListener::bind(addr).await?
    };

    axum::serve(listener, routes).await
}

pub async fn internal_app() -> Result<(), std::io::Error> {
    let routes = routes::metrics::router();
    let listener = {
        let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
        tracing::debug!("going to listen on {}", addr);
        TcpListener::bind(addr).await?
    };

    axum::serve(listener, routes).await
}
