use aws_sdk_ses::{
    Client,
    types::{Body, Content, Destination, Message},
};

use anyhow::{Context, Result};
use tracing::info;

const SES_RECIPIENT_LIMIT: usize = 50;
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

    pub async fn send_mail(
        &self,
        subject: &str,
        content: &str,
        recipients: &[String],
    ) -> Result<()> {
        for chunk in recipients.chunks(SES_RECIPIENT_LIMIT) {
            info!(recipients = chunk.len(), "Sending mail via SES");

            let dest = Destination::builder()
                .set_bcc_addresses(Some(chunk.to_vec()))
                .build();

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
                .context("Failed to send email via SES")?;

            info!(message_id = ?response.message_id(), "Email sent successfully");
        }

        Ok(())
    }
}
