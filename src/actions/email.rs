use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde_json::json;
use sqlx::PgPool;

use super::context::ActionContext;
use super::template;
use super::{ActionError, ActionModule, ActionResult, ActionStatus};
use crate::{crypto, db};

pub struct EmailModule {
    pool: PgPool,
    encryption_key: String,
}

impl EmailModule {
    pub fn new(pool: PgPool, encryption_key: String) -> Self {
        Self { pool, encryption_key }
    }
}

#[async_trait]
impl ActionModule for EmailModule {
    fn id(&self) -> &str {
        "email"
    }

    fn name(&self) -> &str {
        "Email"
    }

    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "Recipient email (supports {{data.email}})" },
                "subject": { "type": "string", "description": "Email subject (supports template vars)" },
                "body": { "type": "string", "description": "Email body (supports template vars)" },
                "html": { "type": "boolean", "default": false, "description": "Send as HTML" }
            },
            "required": ["to", "subject", "body"]
        })
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ActionError> {
        for field in &["to", "subject", "body"] {
            config
                .get(*field)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ActionError::from(format!("{field} is required")))?;
        }
        Ok(())
    }

    async fn execute(
        &self,
        ctx: &ActionContext,
        config: &serde_json::Value,
    ) -> Result<ActionResult, ActionError> {
        let smtp_config = load_tenant_smtp(&self.pool, ctx.tenant.id, &self.encryption_key)
            .await
            .map_err(|e| ActionError::from(format!("Failed to load tenant SMTP: {e}")))?;

        let to = template::render(
            config["to"].as_str().unwrap_or_default(),
            ctx,
        );
        let subject = template::render(
            config["subject"].as_str().unwrap_or_default(),
            ctx,
        );
        let body = template::render(
            config["body"].as_str().unwrap_or_default(),
            ctx,
        );
        let is_html = config["html"].as_bool().unwrap_or(false);

        let from = if let Some(name) = &smtp_config.from_name {
            format!("{} <{}>", name, smtp_config.from_address)
        } else {
            smtp_config.from_address.clone()
        };

        let mut message_builder = Message::builder()
            .from(from.parse().map_err(|e| ActionError::from(format!("Invalid from address: {e}")))?)
            .to(to.parse().map_err(|e| ActionError::from(format!("Invalid to address: {e}")))?);

        message_builder = message_builder.subject(subject);

        let message = if is_html {
            message_builder
                .header(ContentType::TEXT_HTML)
                .body(body)
                .map_err(|e| ActionError::from(format!("Failed to build email: {e}")))?
        } else {
            message_builder
                .header(ContentType::TEXT_PLAIN)
                .body(body)
                .map_err(|e| ActionError::from(format!("Failed to build email: {e}")))?
        };

        let transport = build_smtp_transport(&smtp_config)
            .map_err(|e| ActionError::from(format!("Failed to build SMTP transport: {e}")))?;

        transport
            .send(message)
            .await
            .map_err(|e| ActionError::from(format!("Failed to send email: {e}")))?;

        Ok(ActionResult {
            status: ActionStatus::Success,
            response: Some(json!({ "message": "Email sent successfully" })),
        })
    }
}

pub struct TenantSmtp {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub tls_mode: String,
}

async fn load_tenant_smtp(
    pool: &PgPool,
    tenant_id: uuid::Uuid,
    encryption_key: &str,
) -> Result<TenantSmtp, String> {
    let config = db::tenant_smtp::find_by_tenant(pool, tenant_id)
        .await
        .map_err(|e| format!("DB error: {e}"))?
        .ok_or("Tenant SMTP not configured. Please configure SMTP in tenant settings.")?;

    let username = crypto::decrypt(&config.username_enc, encryption_key)?;
    let password = crypto::decrypt(&config.password_enc, encryption_key)?;

    Ok(TenantSmtp {
        host: config.host,
        port: config.port as u16,
        username,
        password,
        from_address: config.from_address,
        from_name: config.from_name,
        tls_mode: config.tls_mode,
    })
}

pub fn build_smtp_transport(
    config: &TenantSmtp,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let creds = Credentials::new(config.username.clone(), config.password.clone());

    let transport = match config.tls_mode.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|e| format!("SMTP relay error: {e}"))?
            .port(config.port)
            .credentials(creds)
            .build(),
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port)
            .credentials(creds)
            .build(),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|e| format!("SMTP starttls error: {e}"))?
            .port(config.port)
            .credentials(creds)
            .build(),
    };

    Ok(transport)
}
