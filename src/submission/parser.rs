use axum::http::HeaderMap;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Parse a request body based on Content-Type header.
pub fn parse_body(content_type: Option<&str>, body: &[u8]) -> Result<Value, String> {
    let ct = content_type.unwrap_or("application/json");

    if ct.contains("application/json") {
        serde_json::from_slice(body).map_err(|e| format!("Invalid JSON: {e}"))
    } else if ct.contains("application/x-www-form-urlencoded") {
        parse_form_urlencoded(body)
    } else if ct.contains("multipart/form-data") {
        Err("multipart".to_string())
    } else {
        // Try JSON first, then form-urlencoded
        serde_json::from_slice(body)
            .or_else(|_| parse_form_urlencoded(body))
            .map_err(|e| format!("Unable to parse body: {e}"))
    }
}

fn parse_form_urlencoded(body: &[u8]) -> Result<Value, String> {
    let body_str = std::str::from_utf8(body).map_err(|e| format!("Invalid UTF-8: {e}"))?;
    let pairs: HashMap<String, String> = form_urlencoded::parse(body_str.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k, Value::String(v));
    }
    Ok(Value::Object(map))
}

/// Parse multipart form data using multer.
pub async fn parse_multipart(headers: &HeaderMap, body: bytes::Bytes) -> Result<Value, String> {
    let boundary = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|ct| multer::parse_boundary(ct).ok())
        .ok_or_else(|| "Missing multipart boundary".to_string())?;

    let stream = futures_util::stream::once(async { Ok::<_, std::io::Error>(body) });
    let mut multipart = multer::Multipart::new(stream, boundary);

    let mut map = Map::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| format!("Multipart error: {e}"))?
    {
        let name = field.name().unwrap_or("unknown").to_string();
        let value = field
            .text()
            .await
            .map_err(|e| format!("Field read error: {e}"))?;
        map.insert(name, Value::String(value));
    }

    Ok(Value::Object(map))
}
