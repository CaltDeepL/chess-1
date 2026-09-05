use axum::Json;
use serde_json::{json, Value};

/// サーバーの疎通確認用ヘルスチェック
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, description = "サーバーが正常に稼働している", body = serde_json::Value),
    )
)]
pub async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
