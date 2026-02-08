use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::state::SharedState;

pub async fn list_modules(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let modules: Vec<serde_json::Value> = state
        .modules
        .list()
        .iter()
        .map(|m| {
            json!({
                "id": m.id(),
                "name": m.name(),
                "config_schema": m.config_schema(),
            })
        })
        .collect();

    Json(json!({ "modules": modules }))
}
