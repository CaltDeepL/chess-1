use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::auth::extract_user_id;
use crate::errors::{AppError, ProblemDetails};
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct RankingQuery {
    /// 取得件数（既定 50、上限 100）
    limit: Option<i64>,
}

#[derive(FromRow, Serialize, ToSchema)]
pub struct RankingEntry {
    /// 同じレーティングは同順位（3位が2人なら次は5位）
    pub rank: i64,
    pub user_id: Uuid,
    pub username: String,
    pub rating: i32,
    pub games_played: i64,
}

#[derive(Serialize, ToSchema)]
pub struct RankingResponse {
    pub entries: Vec<RankingEntry>,
    /// 認証済みリクエストのときだけ、自分の順位を返す。
    /// 圏外でも自分の位置は知りたいが、そのために画面が2回叩くのは避けたい。
    pub me: Option<RankingEntry>,
}

/// ランキングの本体。上位・自分の行の両方で同じ定義を使うため CTE に切り出す。
///
/// 1局も終えていないユーザーは除外する。登録しただけの 1500 が並ぶと
/// 表として意味を成さないため。
const RANKED_USERS: &str = r#"
WITH played AS (
    SELECT
        u.id,
        u.username,
        u.rating,
        (SELECT count(*) FROM games g
          WHERE g.status = 'finished'
            AND (g.white_user_id = u.id OR g.black_user_id = u.id))::bigint
            AS games_played
    FROM users u
),
ranked AS (
    SELECT
        RANK() OVER (ORDER BY rating DESC)::bigint AS rank,
        id AS user_id,
        username,
        rating,
        games_played
    FROM played
    WHERE games_played > 0
)
"#;

/// GET /users/ranking
///
/// レーティング順の一覧。認証は不要（順位は公開情報）だが、
/// トークンが付いていれば自分の順位も返す。
#[utoipa::path(
    get,
    path = "/users/ranking",
    tag = "users",
    params(RankingQuery),
    responses(
        (status = 200, description = "レーティング順の一覧", body = RankingResponse),
        (status = 400, description = "limit の値が不正",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn get_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RankingQuery>,
) -> Result<Json<RankingResponse>, AppError> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(AppError::BadRequest(
            "limit は 1〜100 の範囲で指定してください".to_string(),
        ));
    }

    let entries = sqlx::query_as::<_, RankingEntry>(&format!(
        "{RANKED_USERS} SELECT * FROM ranked ORDER BY rank, username LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    // 認証は必須ではない。トークンが無い・無効なら me を返さないだけ。
    // ここで 401 にすると、ログイン前にランキングを見られなくなる。
    let me = match extract_user_id(&headers, &state.jwt_secret) {
        Ok(user_id) => {
            sqlx::query_as::<_, RankingEntry>(&format!(
                "{RANKED_USERS} SELECT * FROM ranked WHERE user_id = $1"
            ))
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
        }
        Err(_) => None,
    };

    Ok(Json(RankingResponse { entries, me }))
}
