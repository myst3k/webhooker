use std::net::IpAddr;

use async_trait::async_trait;
use ipnet::IpNet;
use serde_json::json;

use super::context::ActionContext;
use super::template;
use super::{ActionError, ActionModule, ActionResult, ActionStatus};
use crate::config::SsrfMode;

pub struct WebhookModule {
    client: reqwest::Client,
    ssrf_mode: SsrfMode,
    allowed_cidrs: Vec<IpNet>,
}

impl WebhookModule {
    pub fn new(ssrf_mode: SsrfMode, allowed_cidrs: Vec<IpNet>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build reqwest client"),
            ssrf_mode,
            allowed_cidrs,
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

        validate_url(&url, &self.ssrf_mode, &self.allowed_cidrs)?;

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
                    let rendered = template::render(val, ctx);
                    if rendered.contains('\r') || rendered.contains('\n') {
                        return Err(ActionError::from(format!(
                            "Header value for '{k}' contains invalid characters"
                        )));
                    }
                    req = req.header(k, rendered);
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

        let action_status = if (200..300).contains(&status_code) {
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

/// Validate a webhook URL to prevent SSRF attacks.
fn validate_url(url: &str, mode: &SsrfMode, allowed_cidrs: &[IpNet]) -> Result<(), ActionError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| ActionError::from(format!("Invalid webhook URL: {e}")))?;

    // Only allow http/https schemes (always enforced, even in relaxed mode)
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(ActionError::from(format!(
                "Unsupported URL scheme: {scheme}"
            )));
        }
    }

    if *mode == SsrfMode::Relaxed {
        return Ok(());
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| ActionError::from("Webhook URL must have a host"))?;

    let addrs: Vec<IpAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![ip]
    } else {
        use std::net::ToSocketAddrs;
        let socket_addr = format!("{}:{}", host, parsed.port_or_known_default().unwrap_or(80));
        socket_addr
            .to_socket_addrs()
            .map_err(|e| ActionError::from(format!("Failed to resolve host '{host}': {e}")))?
            .map(|sa| sa.ip())
            .collect()
    };

    if addrs.is_empty() {
        return Err(ActionError::from(format!(
            "Could not resolve host: {host}"
        )));
    }

    for addr in &addrs {
        if is_private_ip(addr) && !allowed_cidrs.iter().any(|cidr| cidr.contains(addr)) {
            return Err(ActionError::from(format!(
                "Webhook URL resolves to private/reserved IP: {addr}"
            )));
        }
    }

    Ok(())
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()                              // 127.0.0.0/8
                || v4.is_private()                        // 10/8, 172.16/12, 192.168/16
                || v4.is_link_local()                     // 169.254.0.0/16
                || v4.is_broadcast()                      // 255.255.255.255
                || v4.is_unspecified()                    // 0.0.0.0
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64  // CGNAT 100.64.0.0/10
                || v4.octets()[0] == 198 && (v4.octets()[1] & 0xFE) == 18  // Benchmarks 198.18.0.0/15
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()                              // ::1
                || v6.is_unspecified()                    // ::
                || (v6.segments()[0] & 0xFE00) == 0xFC00  // ULA fc00::/7
                || (v6.segments()[0] & 0xFFC0) == 0xFE80  // Link-local fe80::/10
                || matches!(v6.to_ipv4_mapped(), Some(v4) if is_private_ip(&IpAddr::V4(v4)))
        }
    }
}
