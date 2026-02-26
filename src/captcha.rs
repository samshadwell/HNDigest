use anyhow::Result;
use serde::Deserialize;

// ============================================================================
// Captcha trait
// ============================================================================

#[allow(async_fn_in_trait)]
pub trait Captcha: Send + Sync {
    async fn verify(&self, token: &str) -> Result<bool>;
}

// ============================================================================
// TurnstileCaptcha — Cloudflare Turnstile implementation
// ============================================================================

pub struct TurnstileCaptcha {
    secret_key: String,
    http_client: reqwest::Client,
}

#[derive(Deserialize)]
struct TurnstileVerifyResponse {
    success: bool,
}

impl TurnstileCaptcha {
    pub fn new(secret_key: String) -> Self {
        Self {
            secret_key,
            http_client: reqwest::Client::new(),
        }
    }
}

impl Captcha for TurnstileCaptcha {
    async fn verify(&self, token: &str) -> Result<bool> {
        let params = [("secret", self.secret_key.as_str()), ("response", token)];
        let response = self
            .http_client
            .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
            .form(&params)
            .send()
            .await?;
        let body: TurnstileVerifyResponse = response.json().await?;
        Ok(body.success)
    }
}

// ============================================================================
// Test utilities
// ============================================================================

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;

    pub(crate) struct FakeCaptcha {
        success: bool,
    }

    impl FakeCaptcha {
        pub(crate) fn pass() -> Self {
            Self { success: true }
        }

        pub(crate) fn fail() -> Self {
            Self { success: false }
        }
    }

    impl Captcha for FakeCaptcha {
        async fn verify(&self, _token: &str) -> anyhow::Result<bool> {
            Ok(self.success)
        }
    }
}
