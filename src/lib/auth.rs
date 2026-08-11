use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

use super::secrets::{parse_csv, read_binding};
use worker::Env;

pub enum AuthError {
    MissingApiKeys,
    MissingCredentials,
    InvalidCredentials,
}

fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        let value = value.trim();
        if value.len() > 7
            && value[..6].eq_ignore_ascii_case("bearer")
            && value.as_bytes()[6].is_ascii_whitespace()
        {
            let token = value[7..].trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string)
}

pub fn authorize(headers: &HeaderMap, env: &Env) -> Result<(), AuthError> {
    let keys = parse_csv(&read_binding(env, "API_KEYS"));
    if keys.is_empty() {
        return Err(AuthError::MissingApiKeys);
    }

    let presented = extract_api_key(headers).ok_or(AuthError::MissingCredentials)?;
    if keys.contains(&presented) {
        Ok(())
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

pub fn is_app_allowed(app: &str, env: &Env) -> bool {
    let allowlist = parse_csv(&read_binding(env, "ALLOWED_APPS"));
    allowlist.is_empty() || allowlist.contains(app)
}
