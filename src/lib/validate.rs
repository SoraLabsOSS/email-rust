use serde_json::Value;

pub struct ValidatedContact {
    pub name: String,
    pub email: String,
    pub message: String,
    pub subject: String,
    pub app: String,
}

const EMAIL_RE_AT: &str = "@";
const MAX_NAME: usize = 100;
const MAX_EMAIL: usize = 254;
const MAX_SUBJECT: usize = 200;
const MAX_MESSAGE: usize = 5000;
const MAX_APP: usize = 64;

pub fn validate_contact_body(body: &Value) -> Result<ValidatedContact, String> {
    let object = body
        .as_object()
        .ok_or_else(|| "Request body must be a JSON object".to_string())?;

    let name = as_string(object.get("name"));
    let email = as_string(object.get("email"));
    let message = as_string(object.get("message"));
    let subject =
        as_string(object.get("subject")).unwrap_or_else(|| "New contact message".to_string());
    let app = as_string(object.get("app")).unwrap_or_else(|| "default".to_string());

    let name = name.ok_or_else(|| "name is required".to_string())?;
    if name.len() > MAX_NAME {
        return Err(format!("name must be <= {MAX_NAME} characters"));
    }

    let email = email.ok_or_else(|| "email is required".to_string())?;
    if email.len() > MAX_EMAIL || !is_valid_email(&email) {
        return Err("email is invalid".to_string());
    }

    let message = message.ok_or_else(|| "message is required".to_string())?;
    if message.len() > MAX_MESSAGE {
        return Err(format!("message must be <= {MAX_MESSAGE} characters"));
    }

    if subject.len() > MAX_SUBJECT {
        return Err(format!("subject must be <= {MAX_SUBJECT} characters"));
    }

    if app.len() > MAX_APP || !is_valid_app(&app) {
        return Err("app must be alphanumeric (._- allowed), max 64 chars".to_string());
    }

    Ok(ValidatedContact {
        name,
        email,
        message,
        subject,
        app,
    })
}

pub struct ValidatedNewsletter {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub app: String,
}

/// Newsletter signup: only `email` is required; name may be omitted or empty.
pub fn validate_newsletter_body(body: &Value) -> Result<ValidatedNewsletter, String> {
    let object = body
        .as_object()
        .ok_or_else(|| "Request body must be a JSON object".to_string())?;

    let email = as_string(object.get("email"));
    let first_name = as_string(object.get("firstName"))
        .or_else(|| as_string(object.get("first_name")))
        .unwrap_or_default();
    let last_name = as_string(object.get("lastName"))
        .or_else(|| as_string(object.get("last_name")))
        .unwrap_or_default();
    let full_name = as_string(object.get("name"));
    let app = as_string(object.get("app")).unwrap_or_else(|| "default".to_string());

    let email = email.ok_or_else(|| "email is required".to_string())?;
    if email.len() > MAX_EMAIL || !is_valid_email(&email) {
        return Err("email is invalid".to_string());
    }

    if first_name.len() > MAX_NAME {
        return Err(format!("firstName must be <= {MAX_NAME} characters"));
    }
    if last_name.len() > MAX_NAME {
        return Err(format!("lastName must be <= {MAX_NAME} characters"));
    }
    if let Some(full_name) = &full_name {
        if full_name.len() > MAX_NAME {
            return Err(format!("name must be <= {MAX_NAME} characters"));
        }
    }

    if app.len() > MAX_APP || !is_valid_app(&app) {
        return Err("app must be alphanumeric (._- allowed), max 64 chars".to_string());
    }

    let (first_name, last_name) = if first_name.is_empty() && last_name.is_empty() {
        if let Some(full_name) = full_name {
            let mut parts = full_name.split_whitespace();
            let first = parts.next().unwrap_or("").to_string();
            let last = parts.collect::<Vec<_>>().join(" ");
            (first, last)
        } else {
            (first_name, last_name)
        }
    } else {
        (first_name, last_name)
    };

    Ok(ValidatedNewsletter {
        email: email.to_lowercase(),
        first_name,
        last_name,
        app,
    })
}

fn as_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once(EMAIL_RE_AT) else {
        return false;
    };
    if local.is_empty() || local.contains(char::is_whitespace) || local.contains('@') {
        return false;
    }
    if domain.is_empty() || domain.contains(char::is_whitespace) || domain.contains('@') {
        return false;
    }
    domain.contains('.')
}

fn is_valid_app(app: &str) -> bool {
    app.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}
