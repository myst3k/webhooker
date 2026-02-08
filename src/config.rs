use std::net::IpAddr;

use ipnet::IpNet;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub encryption_key: String,
    pub host: IpAddr,
    pub port: u16,
    pub base_url: String,
    pub registration: RegistrationMode,
    pub max_body_size: usize,
    pub trusted_proxies: Vec<IpNet>,
    pub log_level: String,
    pub smtp: Option<SmtpConfig>,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub from: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegistrationMode {
    Open,
    Closed,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = env_required("DATABASE_URL")?;
        let jwt_secret = env_required("JWT_SECRET")?;
        let encryption_key = env_required("WEBHOOKER_ENCRYPTION_KEY")?;

        let host: IpAddr = env_or("WEBHOOKER_HOST", "0.0.0.0")
            .parse()
            .map_err(|e| format!("Invalid WEBHOOKER_HOST: {e}"))?;

        let port: u16 = env_or("WEBHOOKER_PORT", "3000")
            .parse()
            .map_err(|e| format!("Invalid WEBHOOKER_PORT: {e}"))?;

        let base_url = env_or("WEBHOOKER_BASE_URL", &format!("http://{host}:{port}"));

        let registration = match env_or("WEBHOOKER_REGISTRATION", "closed").as_str() {
            "open" => RegistrationMode::Open,
            _ => RegistrationMode::Closed,
        };

        let max_body_size: usize = env_or("WEBHOOKER_MAX_BODY_SIZE", "1048576")
            .parse()
            .map_err(|e| format!("Invalid WEBHOOKER_MAX_BODY_SIZE: {e}"))?;

        let trusted_proxies: Vec<IpNet> = env_or("WEBHOOKER_TRUSTED_PROXIES", "")
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                s.trim()
                    .parse()
                    .map_err(|e| format!("Invalid WEBHOOKER_TRUSTED_PROXIES entry '{s}': {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let log_level = env_or("WEBHOOKER_LOG_LEVEL", "info");

        let smtp = match (
            std::env::var("WEBHOOKER_SMTP_HOST").ok(),
            std::env::var("WEBHOOKER_SMTP_PORT").ok(),
            std::env::var("WEBHOOKER_SMTP_USER").ok(),
            std::env::var("WEBHOOKER_SMTP_PASS").ok(),
            std::env::var("WEBHOOKER_SMTP_FROM").ok(),
        ) {
            (Some(host), Some(port), Some(user), Some(pass), Some(from)) => Some(SmtpConfig {
                host,
                port: port
                    .parse()
                    .map_err(|e| format!("Invalid WEBHOOKER_SMTP_PORT: {e}"))?,
                user,
                pass,
                from,
            }),
            _ => None,
        };

        Ok(Config {
            database_url,
            jwt_secret,
            encryption_key,
            host,
            port,
            base_url,
            registration,
            max_body_size,
            trusted_proxies,
            log_level,
            smtp,
        })
    }
}

fn env_required(key: &str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("Missing required environment variable: {key}"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
