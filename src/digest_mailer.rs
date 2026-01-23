use aws_sdk_ses::{
    Client,
    types::{Body, Content, Destination, Message},
};

use anyhow::{Context, Result};
use tracing::info;

const ENCODING: &str = "UTF-8";

pub struct DigestMailer {
    ses_client: Client,
    from_address: String,
    reply_to_address: String,
}

impl DigestMailer {
    pub fn new(ses_client: Client, from_address: String, reply_to_address: String) -> Self {
        Self {
            ses_client,
            from_address,
            reply_to_address,
        }
    }

    /// Send an email to a single recipient.
    pub async fn send_mail(&self, subject: &str, content: &str, recipient: &str) -> Result<()> {
        let dest = Destination::builder().to_addresses(recipient).build();

        let subject_content = Content::builder().data(subject).charset(ENCODING).build()?;
        let body_content = Content::builder().data(content).charset(ENCODING).build()?;
        let body = Body::builder().html(body_content).build();

        let message = Message::builder()
            .subject(subject_content)
            .body(body)
            .build();

        let response = self
            .ses_client
            .send_email()
            .source(&self.from_address)
            .destination(dest)
            .reply_to_addresses(&self.reply_to_address)
            .return_path(&self.reply_to_address)
            .message(message)
            .send()
            .await
            .context(format!("Failed to send email to {}", recipient))?;

        info!(
            message_id = ?response.message_id(),
            recipient = recipient,
            "Email sent"
        );

        Ok(())
    }
}
