use aws_sdk_ses::{
    types::{Body, Content, Destination, Message},
    Client,
};
use crate::digest_renderer::DigestRenderer;
use anyhow::{Result, Context};
use log::info;

const SES_RECIPIENT_LIMIT: usize = 50;
const FROM: &str = "hndigest@samshadwell.com";
const REPLY_TO: &str = "hi@samshadwell.com";
const ENCODING: &str = "UTF-8";

pub struct DigestMailer {
    ses_client: Client,
}

impl DigestMailer {
    pub fn new(ses_client: Client) -> Self {
        Self { ses_client }
    }

    pub async fn send_mail(&self, renderer: &DigestRenderer<'_>, recipients: &[String]) -> Result<()> {
        let subject = renderer.subject();
        let content = renderer.content()?;

        for chunk in recipients.chunks(SES_RECIPIENT_LIMIT) {
            info!("Sending mail via SES to {} recipients...", chunk.len());
            
            let dest = Destination::builder()
                .set_bcc_addresses(Some(chunk.to_vec()))
                .build();

            let subject_content = Content::builder()
                .data(&subject)
                .charset(ENCODING)
                .build()?;

            let body_content = Content::builder()
                .data(&content)
                .charset(ENCODING)
                .build()?;

            let body = Body::builder()
                .html(body_content)
                .build();

            let message = Message::builder()
                .subject(subject_content)
                .body(body)
                .build();

            let response = self.ses_client.send_email()
                .source(FROM)
                .destination(dest)
                .reply_to_addresses(REPLY_TO)
                .return_path(REPLY_TO)
                .message(message)
                .send()
                .await
                .context("Failed to send email via SES")?;

            info!("Success! message_id={:?}", response.message_id());
        }

        Ok(())
    }
}
