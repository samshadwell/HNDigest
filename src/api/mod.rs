//! HTTP API handler for subscription management.
//!
//! Framework-agnostic: accepts `ApiRequest`, returns `ApiResponse`.
//! The Lambda entry point in `src/bin/api.rs` adapts `lambda_http` types to/from
//! these and calls `handle`.

mod handlers;

use crate::captcha::Captcha;
use crate::mailer::Mailer;
use crate::storage::Storage;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Request / Response types
// ============================================================================

pub struct ApiRequest {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub body: Option<String>,
}

pub enum ApiResponse {
    Html(String),
    Json { status: u16, body: String },
    Text { status: u16, body: String },
    Redirect(String),
}

impl ApiResponse {
    pub fn status(&self) -> u16 {
        match self {
            Self::Html(_) => 200,
            Self::Json { status, .. } => *status,
            Self::Text { status, .. } => *status,
            Self::Redirect(_) => 303,
        }
    }

    pub fn redirect_location(&self) -> Option<&str> {
        if let Self::Redirect(loc) = self {
            Some(loc)
        } else {
            None
        }
    }

    pub fn body_contains(&self, s: &str) -> bool {
        match self {
            Self::Html(body) | Self::Json { body, .. } | Self::Text { body, .. } => {
                body.contains(s)
            }
            Self::Redirect(_) => false,
        }
    }
}

// ============================================================================
// Application state
// ============================================================================

pub struct AppState<S, M, C> {
    pub(crate) storage: Arc<S>,
    pub(crate) mailer: Arc<M>,
    pub(crate) captcha: C,
    pub(crate) base_url: String,
}

impl<S, M, C> AppState<S, M, C> {
    pub fn new(storage: Arc<S>, mailer: Arc<M>, captcha: C, base_url: String) -> Self {
        Self {
            storage,
            mailer,
            captcha,
            base_url,
        }
    }
}

// ============================================================================
// Dispatch
// ============================================================================

pub async fn handle<S, M, C>(request: &ApiRequest, state: &Arc<AppState<S, M, C>>) -> ApiResponse
where
    S: Storage,
    M: Mailer,
    C: Captcha,
{
    let token = request.query.get("token").map(|s| s.as_str()).unwrap_or("");
    let email = request.query.get("email").map(|s| s.as_str()).unwrap_or("");
    let body = request.body.as_deref().unwrap_or("");

    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/api/subscribe") => handlers::subscribe_post(state, body).await,
        ("GET", "/api/verify") => handlers::verify_get(state, email, token).await,
        ("GET", "/api/unsubscribe") => handlers::unsubscribe_get(&state.storage, token).await,
        ("POST", "/api/unsubscribe") => {
            handlers::unsubscribe_post(&state.storage, token, request.body.as_deref()).await
        }
        _ => ApiResponse::Text {
            status: 404,
            body: "Not Found".to_string(),
        },
    }
}
