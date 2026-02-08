pub mod templates;

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::SmtpConfig;

pub struct SystemMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl SystemMailer {
    pub fn new(config: &SmtpConfig) -> Result<Self, String> {
        let creds = Credentials::new(config.user.clone(), config.pass.clone());

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|e| format!("System SMTP error: {e}"))?
            .port(config.port)
            .credentials(creds)
            .build();

        Ok(Self {
            transport,
            from: config.from.clone(),
        })
    }

    pub async fn send_welcome(&self, to_email: &str, to_name: &str, base_url: &str) -> Result<(), String> {
        let html = templates::render_welcome(to_name, base_url);
        self.send(to_email, "Welcome to Webhooker", &html).await
    }

    pub async fn send_password_reset(
        &self,
        to_email: &str,
        reset_url: &str,
    ) -> Result<(), String> {
        let html = templates::render_password_reset(reset_url);
        self.send(to_email, "Password Reset - Webhooker", &html)
            .await
    }

    pub async fn send_member_added(
        &self,
        to_email: &str,
        to_name: &str,
        tenant_name: &str,
        base_url: &str,
    ) -> Result<(), String> {
        let html = templates::render_member_added(to_name, tenant_name, base_url);
        self.send(
            to_email,
            &format!("You've been added to {} - Webhooker", tenant_name),
            &html,
        )
        .await
    }

    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<(), String> {
        let message = Message::builder()
            .from(
                self.from
                    .parse()
                    .map_err(|e| format!("Invalid from address: {e}"))?,
            )
            .to(to.parse().map_err(|e| format!("Invalid to address: {e}"))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .map_err(|e| format!("Failed to build email: {e}"))?;

        self.transport
            .send(message)
            .await
            .map_err(|e| format!("Failed to send email: {e}"))?;

        Ok(())
    }
}
