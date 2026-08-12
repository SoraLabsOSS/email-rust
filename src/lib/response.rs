use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::types::ErrorResponse;

pub fn json(data: impl Serialize, status: StatusCode) -> Response {
    json_with_headers(data, status, HeaderMap::new())
}

fn json_with_headers(data: impl Serialize, status: StatusCode, mut headers: HeaderMap) -> Response {
    let body = serde_json::to_vec(&data).unwrap_or_else(|_| {
        br#"{"ok":false,"error":"Failed to encode response","code":"internal_error"}"#.to_vec()
    });
    if !headers.contains_key(header::CONTENT_TYPE) {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
    }
    (status, headers, body).into_response()
}

pub fn error(status: StatusCode, message: &str, code: &'static str) -> Response {
    json(
        ErrorResponse {
            ok: false,
            error: message.to_string(),
            code: Some(code),
            retry_after_seconds: None,
        },
        status,
    )
}

pub fn error_with_retry_after(
    status: StatusCode,
    message: &str,
    code: &'static str,
    retry_after_seconds: u64,
) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Retry-After",
        HeaderValue::from_str(&retry_after_seconds.to_string()).unwrap(),
    );

    json_with_headers(
        ErrorResponse {
            ok: false,
            error: message.to_string(),
            code: Some(code),
            retry_after_seconds: Some(retry_after_seconds),
        },
        status,
        headers,
    )
}
