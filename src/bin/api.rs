//! HTTP API Lambda for subscription management.
//!
//! Handles HTTP requests for unsubscribe endpoints. Business logic is in
//! the `unsubscribe` module; this file handles HTTP concerns (routing,
//! templates, responses, redirects).

use askama::Template;
use aws_config::BehaviorVersion;
use hndigest::storage_adapter::StorageAdapter;
use hndigest::unsubscribe;
use lambda_http::{Body, Error, Request, RequestExt, Response, run, service_fn};
use reqwest::Method;
use std::env;
use std::sync::Arc;
use tracing::{error, info};

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

// ============================================================================
// Lambda Handler
// ============================================================================

struct AppState {
    storage: Arc<StorageAdapter>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let dynamodb_table = env::var("DYNAMODB_TABLE")
        .map_err(|_| Error::from("DYNAMODB_TABLE environment variable must be set"))?;

    let storage = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(AppState { storage });

    run(service_fn(|event| handler(event, state.clone()))).await
}

async fn handler(event: Request, state: Arc<AppState>) -> Result<Response<Body>, Error> {
    let method = event.method();
    let path = event.uri().path();

    info!(method = %method, path = %path, "Handling request");

    let query_params = event.query_string_parameters();
    let token = query_params.first("token").unwrap_or("");

    match (method, path) {
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
