#[path = "lib/mod.rs"]
mod lib;
mod routes;
mod types;

use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::{any, get};
use axum::Router;
use tower_service::Service;
use worker::{
    console_error, console_log, event, send, Context, Env, HttpRequest, MessageBatch, MessageExt,
    QueueRetryOptionsBuilder, Result,
};

use crate::lib::cors::{apply_cors, options_response};
use crate::lib::resend::send_contact_email;
use crate::lib::response::error;
use crate::lib::secrets::read_binding;
use crate::routes::contact::handle_contact;
use crate::routes::health::handle_health;
use crate::types::EmailQueueMessage;

#[derive(Clone)]
pub struct AppState {
    pub env: send::SendWrapper<Env>,
}

fn router(env: Env) -> Router {
    Router::new()
        .route("/", get(handle_health))
        .route("/health", get(handle_health))
        .route("/api/contact", any(handle_contact))
        .fallback(not_found)
        .with_state(AppState {
            env: send::SendWrapper::new(env),
        })
}

async fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "Not found", "not_found")
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    let origin = req.headers().get(header::ORIGIN).cloned();
    let allowed_origins = read_binding(&env, "ALLOWED_ORIGINS");

    if req.method() == axum::http::Method::OPTIONS {
        return Ok(options_response(origin.as_ref(), &allowed_origins));
    }

    let mut response = router(env).call(req).await?;
    apply_cors(response.headers_mut(), origin.as_ref(), &allowed_origins);
    Ok(response)
}

#[event(queue)]
pub async fn consume(
    batch: MessageBatch<EmailQueueMessage>,
    env: Env,
    _ctx: Context,
) -> Result<()> {
    for msg in batch.messages()? {
        let result = send_contact_email(&env, msg.body()).await;

        if result.ok {
            console_log!(
                "Email sent queueId={} resendId={:?} app={}",
                msg.body().id,
                result.id,
                msg.body().app
            );
            msg.ack();
            continue;
        }

        console_error!(
            "Resend failed queueId={} status={} error={:?} retryable={}",
            msg.body().id,
            result.status,
            result.error,
            result.retryable
        );

        if result.retryable {
            msg.retry_with_options(
                &QueueRetryOptionsBuilder::new()
                    .with_delay_seconds(60)
                    .build(),
            );
        } else {
            msg.ack();
        }
    }

    Ok(())
}
