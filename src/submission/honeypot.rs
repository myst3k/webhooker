use serde_json::Value;

/// Check if the honeypot field is filled. Returns true if spam detected.
pub fn is_spam(data: &Value, honeypot_field: Option<&str>) -> bool {
    let Some(field) = honeypot_field else {
        return false;
    };

    if field.is_empty() {
        return false;
    }

    match data.get(field) {
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}
