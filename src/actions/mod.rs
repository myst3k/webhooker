pub mod context;
pub mod template;
pub mod email;
pub mod webhook;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use context::ActionContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub status: ActionStatus,
    pub response: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    Success,
    Failed,
}

#[derive(Debug)]
pub struct ActionError {
    pub message: String,
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<String> for ActionError {
    fn from(s: String) -> Self {
        ActionError { message: s }
    }
}

impl From<&str> for ActionError {
    fn from(s: &str) -> Self {
        ActionError {
            message: s.to_string(),
        }
    }
}

#[async_trait]
pub trait ActionModule: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn config_schema(&self) -> serde_json::Value;
    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ActionError>;
    async fn execute(
        &self,
        ctx: &ActionContext,
        config: &serde_json::Value,
    ) -> Result<ActionResult, ActionError>;
}

pub struct ModuleRegistry {
    modules: HashMap<String, Arc<dyn ActionModule>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    pub fn register(&mut self, module: Arc<dyn ActionModule>) {
        self.modules.insert(module.id().to_string(), module);
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn ActionModule>> {
        self.modules.get(id)
    }

    pub fn list(&self) -> Vec<&Arc<dyn ActionModule>> {
        self.modules.values().collect()
    }
}
