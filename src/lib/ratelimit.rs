use axum::http::HeaderMap;
use reqwest::Client;
use serde_json::Value;

use super::client::get_client_ip;
use super::secrets::read_binding;
use worker::Env;

#[derive(Debug, Clone)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub retry_after_seconds: Option<u64>,
}

// Derived from @upstash/ratelimit-js fixedWindowLimitScript (single.ts),
// but extended to return TTL so we can set `Retry-After`.
const FIXED_WINDOW_LIMIT_WITH_TTL_SCRIPT: &str = r#"
 local key = KEYS[1]
 local dynamicLimitKey = KEYS[2] -- optional: key for dynamic limit in redis
 local tokens = tonumber(ARGV[1]) -- default limit
 local window = ARGV[2]
 local incrementBy = ARGV[3] -- increment rate per request at a given value, default is 1

 local effectiveLimit = tokens
 if dynamicLimitKey ~= "" then
   local dynamicLimit = redis.call("GET", dynamicLimitKey)
   if dynamicLimit then
     effectiveLimit = tonumber(dynamicLimit)
   end
 end

 local r = redis.call("INCRBY", key, incrementBy)
 if r == tonumber(incrementBy) then
   redis.call("PEXPIRE", key, window)
 end

 local ttlMs = redis.call("PTTL", key)
 if ttlMs < 0 then
   ttlMs = 0
 end

 return {r, effectiveLimit, ttlMs}
"#;

fn upstash_keys(prefix: &str, namespace: &str, identifier: &str) -> String {
    // Keep key format stable and explicit to avoid collisions.
    format!("{prefix}:{namespace}:{identifier}")
}

fn parse_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

async fn upstash_fixed_window_limit(
    env: &Env,
    key: &str,
    limit: u64,
    window_seconds: u64,
) -> RateLimitDecision {
    let url = read_binding(env, "UPSTASH_REDIS_REST_URL");
    let token = read_binding(env, "UPSTASH_REDIS_REST_TOKEN");

    // Fail open when configuration is missing (so we don't brick the API accidentally).
    if url.is_empty() || token.is_empty() {
        return RateLimitDecision {
            allowed: true,
            retry_after_seconds: None,
        };
    }

    let window_ms = window_seconds.saturating_mul(1000);
    let increment_by = 1u64;

    let payload = Value::Array(vec![
        Value::String("EVAL".to_string()),
        Value::String(FIXED_WINDOW_LIMIT_WITH_TTL_SCRIPT.to_string()),
        Value::Number(2.into()), // numkeys
        Value::String(key.to_string()), // KEYS[1]
        Value::String("".to_string()), // KEYS[2]
        Value::Number(limit.into()), // ARGV[1]
        Value::Number(window_ms.into()), // ARGV[2]
        Value::Number(increment_by.into()), // ARGV[3]
    ]);

    let client = Client::new();
    let resp = match client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return RateLimitDecision {
                allowed: true,
                retry_after_seconds: None,
            }
        }
    };

    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return RateLimitDecision {
                allowed: true,
                retry_after_seconds: None,
            }
        }
    };

    let result = body.get("result").cloned().unwrap_or(Value::Null);
    let result_arr = match result {
        Value::Array(a) => a,
        _ => return RateLimitDecision {
            allowed: true,
            retry_after_seconds: None,
        },
    };

    // Script returns: {r, effectiveLimit, ttlMs}
    let r = result_arr.get(0).and_then(parse_u64).unwrap_or(limit);
    let effective_limit = result_arr.get(1).and_then(parse_u64).unwrap_or(limit);
    let ttl_ms = result_arr.get(2).and_then(parse_u64).unwrap_or(0);

    let allowed = r <= effective_limit;
    let retry_after_seconds = if allowed {
        None
    } else {
        // ceil(ttl_ms/1000)
        Some((ttl_ms + 999) / 1000)
    };

    RateLimitDecision {
        allowed,
        retry_after_seconds,
    }
}

fn contact_rate_prefix() -> &'static str {
    "@upstash/ratelimit"
}

pub async fn check_contact_rate_limits(
    env: &Env,
    headers: &HeaderMap,
    email: &str,
) -> RateLimitDecision {
    let ip = get_client_ip(headers);

    // Contact form limits (from SoraLabsOSS/email README):
    // - 1 request / hour / IP
    // - 1 request / hour / sender email
    // - 100 requests / day globally
    //
    // We use fixed-window with a 1-hour or 1-day TTL.
    let ip_key = upstash_keys(contact_rate_prefix(), "contact:ip", &ip);
    let email_key = upstash_keys(contact_rate_prefix(), "contact:email", email);
    let global_key = upstash_keys(contact_rate_prefix(), "contact:global", "global");

    let ip_decision = upstash_fixed_window_limit(env, &ip_key, 1, 3600).await;
    if !ip_decision.allowed {
        return ip_decision;
    }

    let email_decision = upstash_fixed_window_limit(env, &email_key, 1, 3600).await;
    if !email_decision.allowed {
        return email_decision;
    }

    upstash_fixed_window_limit(env, &global_key, 100, 86400).await
}

pub async fn check_newsletter_rate_limits(
    env: &Env,
    headers: &HeaderMap,
    email: &str,
) -> RateLimitDecision {
    let ip = get_client_ip(headers);

    // Newsletter limits:
    // - 5 requests / hour / IP
    // - 3 requests / hour / email
    let ip_key = upstash_keys(contact_rate_prefix(), "newsletter:ip", &ip);
    let email_key = upstash_keys(contact_rate_prefix(), "newsletter:email", email);

    let ip_decision = upstash_fixed_window_limit(env, &ip_key, 5, 3600).await;
    if !ip_decision.allowed {
        return ip_decision;
    }

    upstash_fixed_window_limit(env, &email_key, 3, 3600).await
}

