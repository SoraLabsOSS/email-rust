use resend_rs::types::{CreateContactOptions, CreateEmailBaseOptions, Tag};
use resend_rs::{Error, Resend};

use super::secrets::read_binding;
use super::validate::ValidatedNewsletter;
use crate::types::EmailQueueMessage;
use reqwest::Client;
use worker::{console_error, console_log, Env};

pub struct ResendSendResult {
    pub ok: bool,
    pub id: Option<String>,
    pub error: Option<String>,
    pub status: u16,
    pub retryable: bool,
}

pub struct ResendContactResult {
    pub ok: bool,
    pub id: Option<String>,
    pub already_exists: bool,
    pub error: Option<String>,
    pub status: u16,
    pub retryable: bool,
}

fn create_client(env: &Env) -> Result<Resend, ResendSendResult> {
    let api_key = read_binding(env, "RESEND_API_KEY");
    if api_key.is_empty() {
        return Err(ResendSendResult {
            ok: false,
            id: None,
            error: Some("RESEND_API_KEY is not configured".to_string()),
            status: 503,
            retryable: false,
        });
    }
    Ok(Resend::new(&api_key))
}

pub async fn send_contact_email(env: &Env, message: &EmailQueueMessage) -> ResendSendResult {
    let resend = match create_client(env) {
        Ok(client) => client,
        Err(err) => return err,
    };
    let from = read_binding(env, "CONTACT_FROM_EMAIL");
    let to = read_binding(env, "CONTACT_TO_EMAIL");
    let subject = format!("[{}] {}", message.app, message.subject);
    let reply_to = format_reply_to(&message.name, &message.email);
    let app_tag = sanitize_tag(&message.app);

    let email = CreateEmailBaseOptions::new(from, [to], subject)
        .with_html(&build_html(message))
        .with_text(&build_text(message))
        .with_reply(&reply_to)
        .with_tag(Tag::new("app", &app_tag))
        .with_tag(Tag::new("source", "email-worker"))
        .with_idempotency_key(&message.id);

    match resend.emails.send(email).await {
        Ok(sent) => ResendSendResult {
            ok: true,
            id: Some(sent.id.to_string()),
            error: None,
            status: 200,
            retryable: false,
        },
        Err(err) => map_resend_error(err),
    }
}

pub async fn create_newsletter_contact(
    env: &Env,
    input: &ValidatedNewsletter,
) -> ResendContactResult {
    let resend = match create_client(env) {
        Ok(client) => client,
        Err(err) => {
            return ResendContactResult {
                ok: false,
                id: None,
                already_exists: false,
                error: err.error,
                status: err.status,
                retryable: err.retryable,
            };
        }
    };

    let segment_id = read_binding(env, "RESEND_NEWSLETTER_SEGMENT_ID");
    console_log!(
        "Creating newsletter contact email={} app={} segment={}",
        input.email,
        input.app,
        if segment_id.is_empty() {
            "none"
        } else {
            &segment_id
        }
    );
    let api_key = read_binding(env, "RESEND_API_KEY");

    let mut contact = CreateContactOptions::new(&input.email).with_unsubscribed(false);
    if !input.first_name.is_empty() {
        contact = contact.with_first_name(&input.first_name);
    }
    if !input.last_name.is_empty() {
        contact = contact.with_last_name(&input.last_name);
    }
    if !segment_id.is_empty() {
        contact = contact.with_segment(&segment_id);
    }

    // Resend's contacts.create() may succeed even if the contact already exists.
    // We therefore pre-check with a raw HTTP GET and set `already_exists` reliably.
    let already_exists = match newsletter_contact_exists_raw(&api_key, &input.email).await {
        Ok(v) => v,
        Err(e) => {
            console_error!(
                "Newsletter pre-check raw failed email={} error={}",
                input.email,
                e
            );
            false
        }
    };

    match resend.contacts.create(contact).await {
        Ok(id) => ResendContactResult {
            ok: true,
            id: Some(id.to_string()),
            already_exists,
            error: None,
            status: 201,
            retryable: false,
        },
        Err(err) => map_contact_error(err),
    }
}

async fn newsletter_contact_exists_raw(api_key: &str, email: &str) -> Result<bool, String> {
    if api_key.is_empty() {
        return Ok(false);
    }

    // Resend API: GET /contacts/{id_or_email}
    let encoded_email = urlencoding::encode(email);
    let url = format!("https://api.resend.com/contacts/{encoded_email}");

    let client = Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Ok(false);
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let id = body.get("id").and_then(|v| v.as_str()).or_else(|| {
        body.get("data")
            .and_then(|d| d.get("id"))
            .and_then(|v| v.as_str())
    });

    Ok(id.is_some() && !id.unwrap_or_default().is_empty())
}

fn map_resend_error(err: Error) -> ResendSendResult {
    let error = err.to_string();
    match err {
        Error::RateLimit { .. } => ResendSendResult {
            ok: false,
            id: None,
            error: Some(error),
            status: 429,
            retryable: true,
        },
        Error::Http(http_err) => {
            let status = http_err.status().map(|s| s.as_u16()).unwrap_or(0);
            ResendSendResult {
                ok: false,
                id: None,
                error: Some(error),
                status,
                retryable: status == 0 || status == 429 || status >= 500,
            }
        }
        Error::Resend(response) => ResendSendResult {
            ok: false,
            id: None,
            error: Some(response.message),
            status: response.status_code,
            retryable: response.status_code == 429 || response.status_code >= 500,
        },
        Error::Parse { .. } | Error::Other(_) => ResendSendResult {
            ok: false,
            id: None,
            error: Some(error),
            status: 502,
            retryable: true,
        },
    }
}

fn is_already_exists(status: u16, message: &str) -> bool {
    status == 409 || message.to_ascii_lowercase().contains("already exists")
}

fn map_contact_error(err: Error) -> ResendContactResult {
    let mapped = map_resend_error(err);
    let message = mapped.error.clone().unwrap_or_default();
    if is_already_exists(mapped.status, &message) {
        return ResendContactResult {
            ok: true,
            id: None,
            already_exists: true,
            error: None,
            status: 409,
            retryable: false,
        };
    }

    ResendContactResult {
        ok: mapped.ok,
        id: None,
        already_exists: false,
        error: mapped.error,
        status: mapped.status,
        retryable: mapped.retryable,
    }
}

fn format_reply_to(name: &str, email: &str) -> String {
    let safe_name: String = name
        .chars()
        .filter(|c| !matches!(c, '\r' | '\n' | '<' | '>' | '"'))
        .collect();
    let safe_name = safe_name.trim();
    let safe_name = if safe_name.is_empty() {
        "Contact"
    } else {
        safe_name
    };
    format!("{safe_name} <{email}>")
}

fn sanitize_tag(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|c| if c == '.' { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .take(256)
        .collect();
    if sanitized.is_empty() {
        "default".to_string()
    } else {
        sanitized
    }
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn build_html(message: &EmailQueueMessage) -> String {
    let reply_to = escape_html(&message.email);
    format!(
        r#"
      <h2>New contact message</h2>
      <p>
        Reply tip: press Reply in your mail client — it will go to
        <strong>{reply_to}</strong>
        (Reply-To is set automatically).
      </p>
      <p><strong>App:</strong> {app}</p>
      <p><strong>Name:</strong> {name}</p>
      <p><strong>Email (Reply-To):</strong> {reply_to}</p>
      <p><strong>Subject:</strong> {subject}</p>
      <p><strong>IP:</strong> {ip}</p>
      <pre>{body}</pre>
    "#,
        app = escape_html(&message.app),
        name = escape_html(&message.name),
        subject = escape_html(&message.subject),
        ip = escape_html(&message.ip),
        body = escape_html(&message.message),
    )
    .trim()
    .to_string()
}

fn build_text(message: &EmailQueueMessage) -> String {
    [
        "New contact message",
        &format!("Reply tip: press Reply — it goes to {}", message.email),
        &format!("App: {}", message.app),
        &format!("Name: {}", message.name),
        &format!("Email (Reply-To): {}", message.email),
        &format!("Subject: {}", message.subject),
        &format!("IP: {}", message.ip),
        "",
        &message.message,
    ]
    .join("\n")
}
