use axum::http::HeaderMap;

pub fn get_client_ip(headers: &HeaderMap) -> String {
    if let Some(ip) = headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return ip.to_string();
    }

    if let Some(ip) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return ip.to_string();
    }

    "unknown".to_string()
}

pub fn get_user_agent(headers: &HeaderMap) -> String {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.chars().take(300).collect())
        .filter(|value: &String| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
