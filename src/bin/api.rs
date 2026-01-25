//! HTTP API Lambda for subscription management.
//!
//! Handles HTTP requests for subscribe, verify, and unsubscribe endpoints.
//! Business logic is in the `subscribe` and `unsubscribe` modules; this file
//! handles HTTP concerns (routing, templates, responses, redirects).

use askama::Template;
use aws_config::BehaviorVersion;
use aws_sdk_sesv2::Client as SesClient;
use aws_sdk_sesv2::types::{Body as SesBody, Content, Destination, EmailContent, Message};
use hndigest::storage_adapter::StorageAdapter;
use hndigest::strategies::DigestStrategy;
use hndigest::subscribe::{self, SubscribeResult};
use hndigest::unsubscribe;
use lambda_http::{Body, Error, Request, RequestExt, Response, run, service_fn};
use reqwest::Method;
use serde::Deserialize;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

// ============================================================================
// Templates
// ============================================================================

/// Template for the unsubscribe confirmation page.
/// Success and error pages are static HTML served from S3.
#[derive(Template)]
#[template(path = "unsubscribe_confirm.html")]
struct UnsubscribeConfirmTemplate<'a> {
    email: &'a str,
    token: &'a str,
}

/// Template for the verification email (HTML).
#[derive(Template)]
#[template(path = "verification.html")]
struct VerificationEmailHtmlTemplate<'a> {
    verify_url: &'a str,
    strategy_description: &'a str,
}

/// Template for the verification email (plaintext).
#[derive(Template)]
#[template(path = "verification.txt")]
struct VerificationEmailTextTemplate<'a> {
    verify_url: &'a str,
    strategy_description: &'a str,
}

/// Request body for the subscribe endpoint.
#[derive(Debug, Deserialize)]
struct SubscribeRequest {
    email: String,
    strategy: String,
    #[serde(default)]
    website: String, // Honeypot field - should be empty
}

// ============================================================================
// HTTP Response Helpers
// ============================================================================

fn html_response(status_code: u16, body: String) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(body))
        .expect("Failed to build response")
}

fn redirect(location: &str) -> Response<Body> {
    Response::builder()
        .status(303)
        .header("Location", location)
        .body(Body::Empty)
        .expect("Failed to build response")
}

fn text_response(status_code: u16, message: &str) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Body::from(message.to_string()))
        .expect("Failed to build response")
}

fn json_response(status_code: u16, body: &str) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(Body::from(body.to_string()))
        .expect("Failed to build response")
}

// ============================================================================
// Lambda Handler
// ============================================================================

struct AppState {
    storage: Arc<StorageAdapter>,
    ses_client: SesClient,
    from_address: String,
    reply_to_address: String,
    base_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let ses_client = SesClient::new(&config);

    let dynamodb_table = env::var("DYNAMODB_TABLE")
        .map_err(|_| Error::from("DYNAMODB_TABLE environment variable must be set"))?;
    let from_address = env::var("EMAIL_FROM")
        .map_err(|_| Error::from("EMAIL_FROM environment variable must be set"))?;
    let reply_to_address = env::var("EMAIL_REPLY_TO")
        .map_err(|_| Error::from("EMAIL_REPLY_TO environment variable must be set"))?;
    let base_url = env::var("BASE_URL")
        .map_err(|_| Error::from("BASE_URL environment variable must be set"))?;

    let storage = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(AppState {
        storage,
        ses_client,
        from_address,
        reply_to_address,
        base_url,
    });

    run(service_fn(|event| handler(event, state.clone()))).await
}

async fn handler(event: Request, state: Arc<AppState>) -> Result<Response<Body>, Error> {
    let method = event.method();
    let path = event.uri().path();

    info!(method = %method, path = %path, "Handling request");

    let query_params = event.query_string_parameters();
    let token = query_params.first("token").unwrap_or("");

    match (method, path) {
        (&Method::POST, "/api/subscribe") => {
            let body_str = match event.body() {
                Body::Text(s) => s.as_str().to_string(),
                Body::Binary(b) => std::str::from_utf8(b).unwrap_or("").to_string(),
                Body::Empty => String::new(),
                _ => String::new(),
            };
            Ok(handle_subscribe_post(&state, &body_str).await)
        }
        (&Method::GET, "/api/verify") => Ok(handle_verify_get(&state, token).await),
        (&Method::GET, "/api/unsubscribe") => {
            Ok(handle_unsubscribe_get(&state.storage, token).await)
        }
        (&Method::POST, "/api/unsubscribe") => {
            let body_str = match event.body() {
                Body::Text(s) => Some(s.as_str()),
                Body::Binary(b) => std::str::from_utf8(b).ok(),
                Body::Empty => None,
                _ => None,
            };
            Ok(handle_unsubscribe_post(&state.storage, token, body_str).await)
        }
        _ => Ok(text_response(404, "Not Found")),
    }
}

// ============================================================================
// Route Handlers
// ============================================================================

/// GET /api/unsubscribe?token=...
///
/// Shows confirmation page if token is valid, redirects to error page otherwise.
async fn handle_unsubscribe_get(storage: &Arc<StorageAdapter>, token: &str) -> Response<Body> {
    match unsubscribe::lookup_subscriber(storage, token).await {
        Ok(Some(subscriber)) => {
            // Render confirmation page
            let template = UnsubscribeConfirmTemplate {
                email: &subscriber.email,
                token,
            };
            match template.render() {
                Ok(html) => html_response(200, html),
                Err(e) => {
                    error!(error = %e, "Failed to render template");
                    text_response(500, "Internal server error")
                }
            }
        }
        Ok(None) => {
            // Token not found - redirect to error page
            redirect("/unsubscribe-error.html")
        }
        Err(e) => {
            error!(error = %e, "Error looking up subscriber");
            redirect("/unsubscribe-error.html")
        }
    }
}

/// POST /api/unsubscribe?token=...
///
/// Processes unsubscribe request. Handles both browser form submissions
/// (redirects to success/error page) and RFC 8058 one-click unsubscribe
/// (returns plain text response).
async fn handle_unsubscribe_post(
    storage: &Arc<StorageAdapter>,
    token: &str,
    body: Option<&str>,
) -> Response<Body> {
    // RFC 8058 one-click unsubscribe sends "List-Unsubscribe=One-Click" in body
    let is_one_click = body
        .map(|b| b.contains("List-Unsubscribe=One-Click"))
        .unwrap_or(false);

    if is_one_click {
        info!(token = %token, "Processing RFC 8058 one-click unsubscribe");
    }

    match unsubscribe::remove_subscriber(storage, token).await {
        Ok(true) => {
            // Successfully unsubscribed
            if is_one_click {
                // RFC 8058 expects 200 response, not redirect
                text_response(200, "Unsubscribed successfully")
            } else {
                redirect("/unsubscribe-success.html")
            }
        }
        Ok(false) => {
            // Token not found
            if is_one_click {
                text_response(404, "Token not found")
            } else {
                redirect("/unsubscribe-error.html")
            }
        }
        Err(e) => {
            error!(error = %e, "Error removing subscriber");
            if is_one_click {
                text_response(500, "Internal server error")
            } else {
                redirect("/unsubscribe-error.html")
            }
        }
    }
}

/// POST /api/subscribe
///
/// Creates a pending subscription and sends a verification email.
/// Expects JSON body with `email` and `strategy` fields.
async fn handle_subscribe_post(state: &Arc<AppState>, body: &str) -> Response<Body> {
    // Parse request body
    let request: SubscribeRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to parse subscribe request");
            return json_response(400, r#"{"error": "Invalid request body"}"#);
        }
    };

    // Check honeypot field
    if !request.website.is_empty() {
        info!("Honeypot field filled - rejecting bot submission");
        // Return success to not tip off bots, but don't actually process
        return json_response(
            200,
            r#"{"message": "Check your email to confirm your subscription"}"#,
        );
    }

    // Parse strategy
    let strategy = match DigestStrategy::from_str(&request.strategy) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, strategy = %request.strategy, "Invalid strategy");
            return json_response(400, r#"{"error": "Invalid strategy"}"#);
        }
    };

    // Create pending subscription
    let pending = match subscribe::create_pending_subscription(
        &state.storage,
        &request.email,
        strategy,
    )
    .await
    {
        Ok(SubscribeResult::PendingCreated(p)) => p,
        Ok(SubscribeResult::AlreadySubscribed) => {
            info!(email = %request.email, "Email already subscribed");
            // Return generic message to avoid email enumeration
            return json_response(
                200,
                r#"{"message": "Check your email to confirm your subscription"}"#,
            );
        }
        Err(e) => {
            // Check if it's a validation error
            let err_msg = e.to_string();
            if err_msg.contains("Invalid email") {
                return json_response(400, r#"{"error": "Invalid email address"}"#);
            }
            error!(error = %e, "Failed to create pending subscription");
            return json_response(500, r#"{"error": "Internal server error"}"#);
        }
    };

    // Send verification email
    let verify_url = format!("{}/api/verify?token={}", state.base_url, pending.token);
    let strategy_description = describe_strategy(&strategy);

    if let Err(e) =
        send_verification_email(state, &pending.email, &verify_url, &strategy_description).await
    {
        error!(error = %e, email = %pending.email, "Failed to send verification email");
        return json_response(500, r#"{"error": "Failed to send verification email"}"#);
    }

    info!(email = %pending.email, strategy = %strategy, "Verification email sent");
    json_response(
        200,
        r#"{"message": "Check your email to confirm your subscription"}"#,
    )
}

/// GET /api/verify?token=...
///
/// Verifies a pending subscription and creates the subscriber.
async fn handle_verify_get(state: &Arc<AppState>, token: &str) -> Response<Body> {
    if token.is_empty() {
        return redirect("/verify-error.html");
    }

    match subscribe::verify_subscription(&state.storage, token).await {
        Ok(Some(_subscriber)) => {
            info!(token = %token, "Subscription verified successfully");
            redirect("/verify-success.html")
        }
        Ok(None) => {
            warn!(token = %token, "Verification token not found or expired");
            redirect("/verify-error.html")
        }
        Err(e) => {
            error!(error = %e, token = %token, "Error verifying subscription");
            redirect("/verify-error.html")
        }
    }
}

/// Send a verification email to the subscriber.
async fn send_verification_email(
    state: &Arc<AppState>,
    email: &str,
    verify_url: &str,
    strategy_description: &str,
) -> anyhow::Result<()> {
    use anyhow::Context;

    // Render email templates
    let html_template = VerificationEmailHtmlTemplate {
        verify_url,
        strategy_description,
    };
    let text_template = VerificationEmailTextTemplate {
        verify_url,
        strategy_description,
    };

    let html_content = html_template
        .render()
        .context("Failed to render HTML template")?;
    let text_content = text_template
        .render()
        .context("Failed to render text template")?;

    // Build email
    let subject = Content::builder()
        .data("Confirm your HNDigest subscription")
        .charset("UTF-8")
        .build()?;
    let html_body = Content::builder()
        .data(html_content)
        .charset("UTF-8")
        .build()?;
    let text_body = Content::builder()
        .data(text_content)
        .charset("UTF-8")
        .build()?;

    let body = SesBody::builder().html(html_body).text(text_body).build();
    let message = Message::builder().subject(subject).body(body).build();
    let destination = Destination::builder().to_addresses(email).build();
    let email_content = EmailContent::builder().simple(message).build();

    let response = state
        .ses_client
        .send_email()
        .from_email_address(&state.from_address)
        .reply_to_addresses(&state.reply_to_address)
        .destination(destination)
        .content(email_content)
        .send()
        .await
        .context(format!("Failed to send email to {}", email))?;

    info!(
        message_id = ?response.message_id(),
        recipient = email,
        "Verification email sent"
    );

    Ok(())
}

/// Get a human-readable description of a digest strategy.
fn describe_strategy(strategy: &DigestStrategy) -> String {
    match strategy {
        DigestStrategy::TopN(n) => format!("Top {} stories by points", n),
        DigestStrategy::OverPointThreshold(t) => format!("All stories with {}+ points", t),
    }
}
