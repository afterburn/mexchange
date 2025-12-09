use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use super::{MailError, MailProvider};

pub struct SmtpProvider {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl SmtpProvider {
    pub fn new(host: String, port: u16, username: String, password: String, from: String) -> Self {
        let creds = Credentials::new(username, password);

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
            .expect("Failed to create SMTP transport")
            .port(port)
            .credentials(creds)
            .build();

        Self { mailer, from }
    }
}

#[async_trait::async_trait]
impl MailProvider for SmtpProvider {
    async fn send_otp(&self, to: &str, code: &str) -> Result<(), MailError> {
        let email = Message::builder()
            .from(self.from.parse().map_err(|e| {
                MailError::ConfigError(format!("Invalid from address: {}", e))
            })?)
            .to(to.parse().map_err(|e| {
                MailError::SendFailed(format!("Invalid to address: {}", e))
            })?)
            .subject("Your verification code")
            .header(ContentType::TEXT_HTML)
            .body(format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background-color: #000; color: #fff; padding: 40px 20px; margin: 0;">
    <div style="max-width: 400px; margin: 0 auto;">
        <h1 style="font-size: 24px; font-weight: 500; margin: 0 0 24px 0;">mExchange</h1>
        <p style="color: #999; font-size: 14px; margin: 0 0 24px 0;">Your verification code is:</p>
        <div style="background-color: #111; border: 1px solid #333; border-radius: 8px; padding: 24px; text-align: center; margin: 0 0 24px 0;">
            <span style="font-size: 32px; font-weight: 600; letter-spacing: 8px; font-family: monospace;">{}</span>
        </div>
        <p style="color: #666; font-size: 12px; margin: 0;">This code expires in 10 minutes. If you didn't request this, you can safely ignore this email.</p>
    </div>
</body>
</html>"#,
                code
            ))
            .map_err(|e| MailError::SendFailed(format!("Failed to build email: {}", e)))?;

        self.mailer
            .send(email)
            .await
            .map_err(|e| MailError::SendFailed(format!("Failed to send email: {}", e)))?;

        tracing::info!("OTP email sent to {}", to);
        Ok(())
    }
}
