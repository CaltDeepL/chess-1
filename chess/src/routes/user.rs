use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::models::UserPublicResponse;
use crate::state::AppState;

/// GET /users/:id
/// ユーザー名の参照用エンドポイント(対戦相手の表示名取得などに使う)。
/// パスワードハッシュ等は含まない公開情報のみ返す。
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserPublicResponse>, (StatusCode, String)> {
    let user = sqlx::query_as::<_, UserPublicResponse>(
        "SELECT id, username FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DBエラー: {}", e)))?
    .ok_or((StatusCode::NOT_FOUND, "ユーザーが見つかりません".to_string()))?;

    Ok(Json(user))
}
