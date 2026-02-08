use async_trait::async_trait;
use serde_json::json;

use super::context::ActionContext;
use super::template;
use super::{ActionError, ActionModule, ActionResult, ActionStatus};

pub struct WebhookModule {
    client: reqwest::Client,
}

impl WebhookModule {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build reqwest client"),
        }
    }
}

#[async_trait]
impl ActionModule for WebhookModule {
    fn id(&self) -> &str {
        "webhook"
    }

    fn name(&self) -> &str {
        "Webhook"
    }

    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Webhook URL" },
                "method": { "type": "string", "enum": ["POST", "PUT"], "default": "POST" },
                "headers": { "type": "object", "description": "Custom headers" },
                "body_template": { "type": "string", "description": "Custom body template (JSON). If empty, sends full submission data." }
            },
            "required": ["url"]
        })
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ActionError> {
        config
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ActionError::from("url is required"))?;
        Ok(())
    }

    async fn execute(
        &self,
        ctx: &ActionContext,
        config: &serde_json::Value,
    ) -> Result<ActionResult, ActionError> {
        let url = config["url"]
            .as_str()
            .ok_or_else(|| ActionError::from("url is required"))?;
        let url = template::render(url, ctx);

        let method = config["method"].as_str().unwrap_or("POST");

        let body = if let Some(tmpl) = config.get("body_template").and_then(|v| v.as_str()) {
            if tmpl.is_empty() {
                json!({
                    "data": &ctx.submission.data,
                    "extras": &ctx.submission.extras,
                    "metadata": &ctx.submission.metadata,
                    "endpoint": &ctx.endpoint.name,
                    "project": &ctx.project.name,
                    "submitted_at": &ctx.submission.created_at,
                })
            } else {
                let rendered = template::render(tmpl, ctx);
                serde_json::from_str(&rendered).unwrap_or(json!(rendered))
            }
        } else {
            json!({
                "data": &ctx.submission.data,
                "extras": &ctx.submission.extras,
                "metadata": &ctx.submission.metadata,
                "endpoint": &ctx.endpoint.name,
                "project": &ctx.project.name,
                "submitted_at": &ctx.submission.created_at,
            })
        };

        let mut req = match method {
            "PUT" => self.client.put(&url),
            _ => self.client.post(&url),
        };

        req = req.header("Content-Type", "application/json");

        if let Some(headers) = config.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in headers {
                if let Some(val) = v.as_str() {
                    req = req.header(k, template::render(val, ctx));
                }
            }
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| ActionError::from(format!("Webhook request failed: {e}")))?;

        let status_code = resp.status().as_u16();
        let resp_body = resp
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(1024)
            .collect::<String>();

        let action_status = if status_code >= 200 && status_code < 300 {
            ActionStatus::Success
        } else {
            ActionStatus::Failed
        };

        Ok(ActionResult {
            status: action_status,
            response: Some(json!({
                "status_code": status_code,
                "body": resp_body,
            })),
        })
    }
}
