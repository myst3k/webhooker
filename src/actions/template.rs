use regex::Regex;
use std::sync::LazyLock;

use super::context::ActionContext;

static TEMPLATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{(\w+(?:\.\w+)*)\}\}").unwrap());

/// Replace {{variable}} placeholders in a template string with values from the context.
pub fn render(template: &str, ctx: &ActionContext) -> String {
    TEMPLATE_RE
        .replace_all(template, |caps: &regex::Captures| {
            let path = &caps[1];
            resolve(path, ctx).unwrap_or_default()
        })
        .to_string()
}

fn resolve(path: &str, ctx: &ActionContext) -> Option<String> {
    let parts: Vec<&str> = path.splitn(2, '.').collect();
    match parts.as_slice() {
        ["data", field] => json_string_field(&ctx.submission.data, field),
        ["extras", field] => json_string_field(&ctx.submission.extras, field),
        ["metadata", field] => json_string_field(&ctx.submission.metadata, field),
        ["endpoint", "name"] => Some(ctx.endpoint.name.clone()),
        ["endpoint", "slug"] => Some(ctx.endpoint.slug.clone()),
        ["endpoint", "id"] => Some(ctx.endpoint.id.to_string()),
        ["project", "name"] => Some(ctx.project.name.clone()),
        ["project", "slug"] => Some(ctx.project.slug.clone()),
        ["tenant", "name"] => Some(ctx.tenant.name.clone()),
        ["submission", "id"] => Some(ctx.submission.id.to_string()),
        ["submission", "created_at"] => Some(ctx.submission.created_at.to_rfc3339()),
        _ => None,
    }
}

fn json_string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    match value.get(field)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}
