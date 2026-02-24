//! HTTP API Lambda for subscription management.
//!
//! Handles HTTP requests for subscribe, verify, and unsubscribe endpoints.
//! Business logic is in the `subscribe` and `unsubscribe` modules; this file
//! handles HTTP concerns (routing, templates, responses, redirects).

use askama::Template;
use aws_config::BehaviorVersion;
use aws_sdk_sesv2::Client as SesClient;
use aws_sdk_ssm::Client as SsmClient;
use email_address::EmailAddress;
use hndigest::mailer::Mailer;
use hndigest::storage_adapter::StorageAdapter;
use hndigest::strategies::DigestStrategy;
use hndigest::subscribe;
use hndigest::types::Token;
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

/// Request body for the subscribe endpoint.
#[derive(Debug, Deserialize)]
struct SubscribeRequest {
    email: String,
    strategy: String,
    #[serde(default)]
    website: String, // Honeypot field - should be empty
    #[serde(default)]
    turnstile_token: String,
}

/// Response from Cloudflare Turnstile siteverify API.
#[derive(Debug, Deserialize)]
struct TurnstileVerifyResponse {
    success: bool,
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

/// Extract request body as a string.
fn body_to_string(body: &Body) -> Option<String> {
    match body {
        Body::Text(s) => Some(s.clone()),
        Body::Binary(b) => std::str::from_utf8(b).ok().map(String::from),
        Body::Empty => None,
        _ => None,
    }
}

// ============================================================================
// Lambda Handler
// ============================================================================

struct AppState {
    storage: Arc<StorageAdapter>,
    mailer: Mailer,
    http_client: reqwest::Client,
    base_url: String,
    turnstile_secret_key: String,
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
    let ses_configuration_set = env::var("SES_CONFIGURATION_SET")
        .map_err(|_| Error::from("SES_CONFIGURATION_SET environment variable must be set"))?;

    let mailer = Mailer::new(
        ses_client,
        from_address,
        reply_to_address,
        ses_configuration_set,
    );

    // I'd love to pass this as an environment variable, but using AWS secrets manager is expensive
    // and this is effectively free
    let ssm_client = SsmClient::new(&config);
    let turnstile_secret_key_param = env::var("TURNSTILE_SECRET_KEY_PARAM")
        .map_err(|_| Error::from("TURNSTILE_SECRET_KEY_PARAM environment variable must be set"))?;
    let turnstile_secret_key = ssm_client
        .get_parameter()
        .name(&turnstile_secret_key_param)
        .with_decryption(true)
        .send()
        .await?
        .parameter()
        .and_then(|p| p.value.clone())
        .ok_or_else(|| {
            anyhow::format_err!(
                "SSM parameter value not found for name {}",
                turnstile_secret_key_param
            )
        })?;

    let http_client = reqwest::Client::new();
    let storage = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(AppState {
        storage,
        mailer,
        http_client,
        base_url,
        turnstile_secret_key,
    });

    run(service_fn(|event| handler(event, state.clone()))).await
}

async fn handler(event: Request, state: Arc<AppState>) -> Result<Response<Body>, Error> {
    let method = event.method();
    let path = event.uri().path();

    info!(method = %method, path = %path, "Handling request");

    let query_params = event.query_string_parameters();
    let token = query_params.first("token").unwrap_or("");
    let email = query_params.first("email").unwrap_or("");

    match (method, path) {
        (&Method::POST, "/api/subscribe") => {
            let body_str = body_to_string(event.body()).unwrap_or_default();
            Ok(handle_subscribe_post(&state, &body_str).await)
        }
        (&Method::GET, "/api/verify") => Ok(handle_verify_get(&state, email, token).await),
        (&Method::GET, "/api/unsubscribe") => {
            let token = match Token::from_str(token) {
                Ok(t) => t,
                Err(_) => return Ok(redirect("/unsubscribe-error.html")),
            };
            Ok(handle_unsubscribe_get(&state.storage, &token).await)
        }
        (&Method::POST, "/api/unsubscribe") => {
            let token = match Token::from_str(token) {
                Ok(t) => t,
                Err(_) => return Ok(text_response(400, "Invalid token")),
            };
            let body_str = body_to_string(event.body());
            Ok(handle_unsubscribe_post(&state.storage, &token, body_str.as_deref()).await)
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
async fn handle_unsubscribe_get(storage: &Arc<StorageAdapter>, token: &Token) -> Response<Body> {
    match unsubscribe::lookup_subscriber(storage, token).await {
        Ok(Some(subscriber)) => {
            // Render confirmation page
            let email_str = subscriber.email.to_string();
            let token_str = token.to_string();
            let template = UnsubscribeConfirmTemplate {
                email: &email_str,
                token: &token_str,
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
    token: &Token,
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

/// Verify a Cloudflare Turnstile CAPTCHA token.
async fn verify_turnstile_token(
    http_client: &reqwest::Client,
    secret_key: &str,
    token: &str,
) -> Result<bool, reqwest::Error> {
    let params = [("secret", secret_key), ("response", token)];
    let response = http_client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&params)
        .send()
        .await?;
    let body: TurnstileVerifyResponse = response.json().await?;
    Ok(body.success)
}

fn subscribe_200_response() -> Response<Body> {
    json_response(
        200,
        r#"{"message": "Check your email to confirm your subscription"}"#,
    )
}

fn subscribe_500_response() -> Response<Body> {
    json_response(
        500,
        r#"{"error": "Internal server error, please try again later"}"#,
    )
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
        return subscribe_200_response();
    }

    // Parse strategy
    let strategy = match DigestStrategy::from_str(&request.strategy) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, strategy = %request.strategy, "Invalid strategy");
            return json_response(400, r#"{"error": "Invalid strategy"}"#);
        }
    };

    let email = match EmailAddress::from_str(&request.email) {
        Ok(e) => e,
        Err(_) => return json_response(400, r#"{"error": "Invalid email address"}"#),
    };

    // Verify Turnstile CAPTCHA token
    if request.turnstile_token.is_empty() {
        warn!("Missing Turnstile CAPTCHA token");
        return json_response(400, r#"{"error": "CAPTCHA verification required"}"#);
    }

    match verify_turnstile_token(
        &state.http_client,
        &state.turnstile_secret_key,
        &request.turnstile_token,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            warn!("Turnstile CAPTCHA verification failed");
            return json_response(400, r#"{"error": "CAPTCHA verification failed"}"#);
        }
        Err(e) => {
            error!(error = %e, "Turnstile API request failed");
            return subscribe_500_response();
        }
    }

    // Check if a verified subscriber already exists for this email
    let existing_subscriber = match state.storage.get_subscriber_by_email(&email).await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to check for existing subscriber");
            return subscribe_500_response();
        }
    };

    if let Some(existing) = existing_subscriber {
        match subscribe::update_subscription_strategy(&state.storage, existing, strategy).await {
            Ok(old_strategy) => {
                if let Err(e) = state
                    .mailer
                    .send_preference_update_email(
                        &email,
                        &old_strategy.description(),
                        &strategy.description(),
                    )
                    .await
                {
                    error!(error = %e, "Failed to send preference update email");
                    return subscribe_500_response();
                }
                // Generic success to avoid leaking information about subscribers
                return subscribe_200_response();
            }
            Err(e) => {
                error!(error = %e, "Failed to update existing subscription");
                return subscribe_500_response();
            }
        }
    }

    // No existing verified subscriber - create pending subscription and send verification email
    let pending =
        match subscribe::create_pending_subscription(&state.storage, &email, strategy).await {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "Failed to create pending subscription");
                return subscribe_500_response();
            }
        };

    let email_str = email.to_string().to_lowercase();
    let verify_url = format!(
        "{}/api/verify?email={}&token={}",
        state.base_url,
        urlencoding::encode(&email_str),
        pending.token
    );
    if let Err(e) = state
        .mailer
        .send_verification_email(&pending.email, &verify_url, &strategy.description())
        .await
    {
        error!(error = %e, email = %pending.email, "Failed to send verification email");
        return subscribe_500_response();
    }

    info!(email = %pending.email, strategy = %strategy, "Verification email sent");
    subscribe_200_response()
}

/// GET /api/verify?email=...&token=...
///
/// Verifies a pending subscription and creates the subscriber.
async fn handle_verify_get(
    state: &Arc<AppState>,
    maybe_email: &str,
    maybe_token: &str,
) -> Response<Body> {
    let email = match EmailAddress::from_str(maybe_email) {
        Ok(e) => e,
        Err(_) => return redirect("/verify-error.html"),
    };
    let token = match Token::from_str(maybe_token) {
        Ok(t) => t,
        Err(_) => return redirect("/verify-error.html"),
    };

    match subscribe::verify_subscription(&state.storage, &email, &token).await {
        Ok(Some(_subscriber)) => {
            // Note: this log line is used for a custom metric
            info!(email = %email, "Subscription verified successfully");
            redirect("/verify-success.html")
        }
        Ok(None) => {
            warn!(email = %email, "Verification failed: not found, expired, or invalid token");
            redirect("/verify-error.html")
        }
        Err(e) => {
            error!(error = %e, email = %email, "Error verifying subscription");
            redirect("/verify-error.html")
        }
    }
}
