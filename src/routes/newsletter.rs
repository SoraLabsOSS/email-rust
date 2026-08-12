use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;

use crate::lib::auth::{authorize, is_app_allowed, AuthError};
use crate::lib::resend::create_newsletter_contact;
use crate::lib::ratelimit::check_newsletter_rate_limits;
use crate::lib::response::{error, error_with_retry_after, json};
use crate::lib::validate::validate_newsletter_body;
use crate::types::NewsletterAcceptedResponse;
use crate::AppState;
use worker::console_error;

#[worker::send]
pub async fn handle_newsletter(
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

    let validated = match validate_newsletter_body(&parsed) {
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

    // Upstash rate limiting (IP + email).
    let rate = check_newsletter_rate_limits(&state.env, &headers, &validated.email).await;
    if !rate.allowed {
        return error_with_retry_after(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limited",
            "rate_limited",
            rate.retry_after_seconds.unwrap_or(0),
        );
    }

    let result = create_newsletter_contact(&state.env, &validated).await;
    if !result.ok {
        if result.retryable {
            return error(
                if result.status == 429 {
                    StatusCode::TOO_MANY_REQUESTS
                } else {
                    StatusCode::BAD_GATEWAY
                },
                "Newsletter signup failed. Try again shortly.",
                "resend_unavailable",
            );
        }

        console_error!(
            "Resend contact create failed status={} error={:?}",
            result.status,
            result.error
        );
        return error(
            StatusCode::BAD_GATEWAY,
            "Failed to create newsletter contact",
            "newsletter_failed",
        );
    }

    json(
        NewsletterAcceptedResponse {
            ok: true,
            id: result.id.unwrap_or_default(),
            already_exists: Some(result.already_exists),
            message: if result.already_exists {
                "Already subscribed.".to_string()
            } else {
                "Subscribed to the newsletter.".to_string()
            },
        },
        StatusCode::CREATED,
    )
}
