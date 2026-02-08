use serde_json::{Map, Value};

/// Sort incoming fields into `data` (matched) and `extras` (unmatched).
/// If no fields are defined, everything goes to `data`.
pub fn sort_fields(
    raw: &Value,
    field_defs: Option<&Value>,
) -> (Value, Value) {
    let Some(obj) = raw.as_object() else {
        return (raw.clone(), Value::Object(Map::new()));
    };

    let Some(defs) = field_defs else {
        // No field definitions: everything goes to data
        return (raw.clone(), Value::Object(Map::new()));
    };

    let defined_names: Vec<String> = defs
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|f| f.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
        .collect();

    let mut data = Map::new();
    let mut extras = Map::new();

    for (key, value) in obj {
        if defined_names.contains(key) {
            data.insert(key.clone(), value.clone());
        } else {
            extras.insert(key.clone(), value.clone());
        }
    }

    (Value::Object(data), Value::Object(extras))
}

/// Validate field types. Returns warnings (doesn't reject).
pub fn validate_fields(
    data: &Value,
    field_defs: Option<&Value>,
) -> Vec<String> {
    let mut warnings = Vec::new();

    let Some(defs) = field_defs.and_then(|d| d.as_array()) else {
        return warnings;
    };

    let obj = match data.as_object() {
        Some(o) => o,
        None => return warnings,
    };

    for def in defs {
        let name = match def.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };

        let required = def.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
        let field_type = def.get("type").and_then(|t| t.as_str()).unwrap_or("text");

        match obj.get(name) {
            None | Some(Value::Null) => {
                if required {
                    warnings.push(format!("Missing required field: {name}"));
                }
            }
            Some(Value::String(s)) => {
                if required && s.is_empty() {
                    warnings.push(format!("Required field is empty: {name}"));
                }
                if field_type == "email" && !s.contains('@') {
                    warnings.push(format!("Invalid email format: {name}"));
                }
                if field_type == "url" && !s.starts_with("http") {
                    warnings.push(format!("Invalid URL format: {name}"));
                }
                if field_type == "number" && s.parse::<f64>().is_err() {
                    warnings.push(format!("Invalid number format: {name}"));
                }
                if field_type == "integer" && s.parse::<i64>().is_err() {
                    warnings.push(format!("Invalid integer format: {name}"));
                }
            }
            _ => {}
        }
    }

    warnings
}
