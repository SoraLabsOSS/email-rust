use axum::{routing::get, Router};
use tower_service::Service;
use worker::*;

fn router() -> Router {
    Router::new().route("/", get(root)).route("/hello", get(hello))
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    Ok(router().call(req).await?)
}

pub async fn root() -> &'static str {
    "Hello World!"
}

pub async fn hello() -> &'static str {
    "Hello, World!"
}
