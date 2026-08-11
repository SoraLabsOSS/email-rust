use axum::http::StatusCode;
use axum::response::Response;

use crate::lib::response::json;
use crate::lib::secrets::now_iso;
use crate::types::HealthResponse;

pub async fn handle_health() -> Response {
    json(
        HealthResponse {
            ok: true,
            service: "email-rust",
            time: now_iso(),
        },
        StatusCode::OK,
    )
}
