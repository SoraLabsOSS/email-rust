use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

const ALLOW_HEADERS: &str = "Content-Type, Authorization, X-API-Key";
const ALLOW_METHODS: &str = "GET, POST, OPTIONS";

fn parse_origins(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return vec!["*".to_string()];
    }
    trimmed
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn apply_cors(headers: &mut HeaderMap, origin: Option<&HeaderValue>, allowed_origins: &str) {
    let allowed = parse_origins(allowed_origins);
    let origin = origin.and_then(|value| value.to_str().ok());

    if allowed.iter().any(|item| item == "*") {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    } else if let Some(origin) = origin {
        if allowed.iter().any(|item| item == origin) {
            if let Ok(value) = HeaderValue::from_str(origin) {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
                headers.insert(header::VARY, HeaderValue::from_static("Origin"));
            }
        }
    }

    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(ALLOW_METHODS),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(ALLOW_HEADERS),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
}

pub fn options_response(origin: Option<&HeaderValue>, allowed_origins: &str) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    apply_cors(response.headers_mut(), origin, allowed_origins);
    response
}
