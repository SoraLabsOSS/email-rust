use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;

use crate::lib::auth::{authorize, is_app_allowed, AuthError};
use crate::lib::client::{get_client_ip, get_user_agent};
use crate::lib::ratelimit::check_contact_rate_limits;
use crate::lib::response::{error, error_with_retry_after, json};
use crate::lib::secrets::now_iso;
use crate::lib::validate::validate_contact_body;
use crate::types::{ContactAcceptedResponse, EmailQueueMessage};
use crate::AppState;
use worker::console_error;

#[worker::send]
pub async fn handle_contact(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if method != Method::POST {
        return error(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method not allowed",
            "method_not_allowed",
        );
    }

    match authorize(&headers, &state.env) {
        Err(AuthError::MissingApiKeys) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "API_KEYS secret is not configured on this Worker",
                "missing_api_keys",
            );
        }
        Err(AuthError::MissingCredentials) => {
            return error(
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "missing_credentials",
            );
        }
        Err(AuthError::InvalidCredentials) => {
            return error(
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "invalid_credentials",
            );
        }
        Ok(()) => {}
    }

    let parsed = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(_) => {
            return error(StatusCode::BAD_REQUEST, "Invalid JSON body", "invalid_json");
        }
    };

    let validated = match validate_contact_body(&parsed) {
        Ok(data) => data,
        Err(message) => {
            return error(StatusCode::BAD_REQUEST, &message, "validation_error");
        }
    };

    if !is_app_allowed(&validated.app, &state.env) {
        return error(
            StatusCode::FORBIDDEN,
            "App is not allowed",
            "app_not_allowed",
        );
    }

    // Upstash rate limiting (IP + sender email + global).
    let rate = check_contact_rate_limits(&state.env, &headers, &validated.email).await;
    if !rate.allowed {
        return error_with_retry_after(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limited",
            "rate_limited",
            rate.retry_after_seconds.unwrap_or(0),
        );
    }

    let message = EmailQueueMessage {
        id: uuid::Uuid::new_v4().to_string(),
        name: validated.name,
        email: validated.email,
        message: validated.message,
        subject: validated.subject,
        app: validated.app,
        ip: get_client_ip(&headers),
        user_agent: get_user_agent(&headers),
        enqueued_at: now_iso(),
    };

    let queue = match state.env.queue("EMAIL_QUEUE") {
        Ok(queue) => queue,
        Err(err) => {
            console_error!("EMAIL_QUEUE binding missing: {}", err);
            return error(
                StatusCode::BAD_GATEWAY,
                "Failed to enqueue email",
                "enqueue_failed",
            );
        }
    };

    let id = message.id.clone();
    if let Err(err) = queue.send(message).await {
        console_error!("Failed to enqueue email: {}", err);
        return error(
            StatusCode::BAD_GATEWAY,
            "Failed to enqueue email",
            "enqueue_failed",
        );
    }

    json(
        ContactAcceptedResponse {
            ok: true,
            id,
            message: "Accepted. Email will be sent shortly.".to_string(),
        },
        StatusCode::ACCEPTED,
    )
}
