use std::net::IpAddr;

use axum::http::HeaderMap;
use ipnet::IpNet;
use serde_json::json;

/// Extract submission metadata from request headers.
pub fn extract(
    headers: &HeaderMap,
    peer_addr: Option<IpAddr>,
    trusted_proxies: &[IpNet],
) -> serde_json::Value {
    let ip = extract_ip(headers, peer_addr, trusted_proxies);
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let referer = headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    json!({
        "ip": ip,
        "user_agent": user_agent,
        "referer": referer,
    })
}

fn extract_ip(
    headers: &HeaderMap,
    peer_addr: Option<IpAddr>,
    trusted_proxies: &[IpNet],
) -> String {
    let peer = peer_addr.unwrap_or(IpAddr::from([127, 0, 0, 1]));

    // Only trust X-Forwarded-For if the direct connection is from a trusted proxy
    if !trusted_proxies.is_empty() && trusted_proxies.iter().any(|net| net.contains(&peer)) {
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            // Take the first (leftmost) IP that isn't a trusted proxy
            for ip_str in xff.split(',').map(|s| s.trim()) {
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    if !trusted_proxies.iter().any(|net| net.contains(&ip)) {
                        return ip.to_string();
                    }
                }
            }
        }
    }

    peer.to_string()
}
