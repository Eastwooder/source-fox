use axum::{routing::get, Router};

pub fn router() -> Router {
    Router::new().route("/ui", get(frontend_ui))
}

#[cfg(debug_assertions)]
async fn frontend_ui() -> String {
    let serve = std::env::var("CLIENT_DIST").unwrap();
    format!("Hello {serve}")
}

#[cfg(not(debug_assertions))]
async fn frontend_ui() -> &'static str {
    // TODO: fix this later on
    //   const SERVE: &str = include_str!(env!("CLIENT_DIST"));
    //   SERVE
    // const SERVE: &str = env!("CLIENT_DIST");
    const SERVE: &str = "serve";
    const_format::concatcp!("Hello ", SERVE)
}
