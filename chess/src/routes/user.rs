use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::errors::{AppError, ProblemDetails};
use crate::models::UserPublicResponse;
use crate::state::AppState;

/// ユーザーの公開プロフィールを取得する
///
/// 対戦相手の表示名取得などに使う。パスワードハッシュ等は含まない公開情報のみ返す。
#[utoipa::path(
    get,
    path = "/users/{id}",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "ユーザーID"),
    ),
    responses(
        (status = 200, description = "公開ユーザー情報", body = UserPublicResponse),
        (status = 404, description = "ユーザーが見つからない",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserPublicResponse>, AppError> {
    let user = sqlx::query_as::<_, UserPublicResponse>(
        "SELECT id, username, rating FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("ユーザーが見つかりません".to_string()))?;

    Ok(Json(user))
}
