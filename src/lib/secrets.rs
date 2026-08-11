use std::collections::HashSet;

use worker::Env;

/// Wrangler secrets, `.dev.vars`, and `[vars]` all show up as env bindings.
pub fn read_binding(env: &Env, name: &str) -> String {
    if let Ok(value) = env.secret(name) {
        let trimmed = value.to_string().trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    env.var(name)
        .map(|value| value.to_string().trim().to_string())
        .unwrap_or_default()
}

pub fn parse_csv(value: &str) -> HashSet<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn now_iso() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
}
