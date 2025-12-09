mod smtp;

use std::sync::Arc;

pub use smtp::SmtpProvider;

/// Mail provider trait - implement this to add new email providers (AWS SES, SendGrid, etc.)
#[async_trait::async_trait]
pub trait MailProvider: Send + Sync {
    async fn send_otp(&self, to: &str, code: &str) -> Result<(), MailError>;
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("Failed to send email: {0}")]
    SendFailed(String),
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

/// Mail service wrapper - holds the active provider
#[derive(Clone)]
pub struct MailService {
    provider: Arc<dyn MailProvider>,
}

impl MailService {
    pub fn new(provider: impl MailProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    pub async fn send_otp(&self, to: &str, code: &str) -> Result<(), MailError> {
        self.provider.send_otp(to, code).await
    }
}

/// Console provider for development - just logs to stdout
pub struct ConsoleProvider;

#[async_trait::async_trait]
impl MailProvider for ConsoleProvider {
    async fn send_otp(&self, to: &str, code: &str) -> Result<(), MailError> {
        tracing::info!("========================================");
        tracing::info!("OTP for {}: {}", to, code);
        tracing::info!("========================================");
        Ok(())
    }
}

/// Create mail service from environment variables
///
/// Required env vars for SMTP:
/// - MAIL_PROVIDER=smtp (or "console" for dev)
/// - SMTP_HOST=smtp.example.com
/// - SMTP_PORT=587
/// - SMTP_USERNAME=user
/// - SMTP_PASSWORD=pass
/// - SMTP_FROM=noreply@example.com
pub fn create_mail_service() -> MailService {
    let provider = std::env::var("MAIL_PROVIDER").unwrap_or_else(|_| "console".to_string());

    match provider.as_str() {
        "smtp" => {
            let host = std::env::var("SMTP_HOST").expect("SMTP_HOST required for smtp provider");
            let port = std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .expect("SMTP_PORT must be a number");
            let username = std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME required");
            let password = std::env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD required");
            let from = std::env::var("SMTP_FROM").expect("SMTP_FROM required");

            tracing::info!("Mail provider: SMTP ({}:{})", host, port);
            MailService::new(SmtpProvider::new(host, port, username, password, from))
        }
        _ => {
            tracing::info!("Mail provider: Console (OTPs will be logged)");
            MailService::new(ConsoleProvider)
        }
    }
}
