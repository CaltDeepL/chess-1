use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shakmaty::Color;
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::auth::extract_user_id;
use crate::domain::history::outcome_for;
use crate::errors::{AppError, ProblemDetails};
use crate::state::AppState;

/// 一覧のページング指定
#[derive(Debug, Deserialize, IntoParams)]
pub struct HistoryQuery {
    /// 取得件数（既定 20、上限 100）
    limit: Option<i64>,
    /// スキップ件数（既定 0）
    offset: Option<i64>,
}

/// DBから取り出す生の行
#[derive(FromRow)]
struct HistoryRow {
    id: Uuid,
    my_color: String,
    opponent_username: Option<String>,
    result: Option<String>,
    end_reason: Option<String>,
    move_count: i64,
    finished_at: DateTime<Utc>,
}

/// 履歴一覧の1件
#[derive(Serialize, ToSchema)]
pub struct GameHistoryItem {
    pub game_id: Uuid,
    /// 自分の手番の色（"white" / "black"）
    pub my_color: String,
    /// 対戦相手のユーザー名。相手が参加しないまま終了した対局では null
    pub opponent_username: Option<String>,
    /// 盤面視点の結果（"white_win" / "black_win" / "draw"）
    pub result: Option<String>,
    /// 自分視点の勝敗（"win" / "loss" / "draw"）。判定できない場合は null
    pub outcome: Option<String>,
    /// 終局理由（"checkmate" / "resignation" / "stalemate" など）
    pub end_reason: Option<String>,
    pub move_count: i64,
    pub finished_at: DateTime<Utc>,
}

/// GET /users/me/games
///
/// 自分が参加した終了済みの対局を、新しい順に返す。
#[utoipa::path(
    get,
    path = "/users/me/games",
    tag = "history",
    params(HistoryQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "対局履歴の一覧", body = [GameHistoryItem]),
        (status = 400, description = "limit / offset の値が不正",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "認証が必要",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn list_my_games(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<GameHistoryItem>>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    // 上限を設けないと、1リクエストで全件を引かれてDBに負荷がかかる
    let limit = query.limit.unwrap_or(20);
    let offset = query.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) {
        return Err(AppError::BadRequest(
            "limit は 1〜100 の範囲で指定してください".to_string(),
        ));
    }
    if offset < 0 {
        return Err(AppError::BadRequest(
            "offset は 0 以上で指定してください".to_string(),
        ));
    }

    let rows = sqlx::query_as::<_, HistoryRow>(
        r#"
        SELECT
            g.id,
            CASE WHEN g.white_user_id = $1 THEN 'white' ELSE 'black' END AS my_color,
            CASE WHEN g.white_user_id = $1 THEN b.username ELSE w.username END
                AS opponent_username,
            g.result::text AS result,
            g.end_reason,
            COALESCE((SELECT count(*) FROM moves m WHERE m.game_id = g.id), 0)::bigint
                AS move_count,
            g.updated_at AS finished_at
        FROM games g
        JOIN users w ON w.id = g.white_user_id
        LEFT JOIN users b ON b.id = g.black_user_id
        WHERE g.status = 'finished'
          AND (g.white_user_id = $1 OR g.black_user_id = $1)
        ORDER BY g.updated_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            // my_color は SQL の CASE で作った値なので white 以外は black
            let color = if row.my_color == "white" {
                Color::White
            } else {
                Color::Black
            };
            let outcome = outcome_for(row.result.as_deref(), color).map(|o| o.as_str().to_string());

            GameHistoryItem {
                game_id: row.id,
                my_color: row.my_color,
                opponent_username: row.opponent_username,
                result: row.result,
                outcome,
                end_reason: row.end_reason,
                move_count: row.move_count,
                finished_at: row.finished_at,
            }
        })
        .collect();

    Ok(Json(items))
}
