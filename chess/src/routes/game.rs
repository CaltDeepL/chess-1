use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess, EnPassantMode, Position};
use uuid::Uuid;

use crate::auth::extract_user_id;
use crate::domain::outcome::{determine_outcome, winner_after_resign};
use crate::domain::player::{expected_player, role_of};
use crate::errors::{AppError, ProblemDetails};
use crate::models::{
    GameCreatedResponse, GameDetailResponse, GameDetailRow, GameRow, GameStateResponse,
    GameSummary, ListGamesQuery, MoveRequest, MoveRow,
};
use crate::state::{AppState, GameEvent};

/// 対局一覧を取得する
///
/// statusを指定するとその状態の対局のみに絞り込む(未指定なら全件)。
/// ロビーには基本的に status=waiting を指定して呼び出す想定。
#[utoipa::path(
    get,
    path = "/games",
    tag = "games",
    params(ListGamesQuery),
    responses(
        (status = 200, description = "対局一覧", body = Vec<GameSummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_games(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListGamesQuery>,
) -> Result<Json<Vec<GameSummary>>, AppError> {
    extract_user_id(&headers, &state.jwt_secret)?;

    let games = if let Some(status) = query.status {
        sqlx::query_as::<_, GameSummary>(
            "SELECT id, white_user_id, black_user_id, status::text AS status, fen, created_at \
             FROM games WHERE status::text = $1 ORDER BY created_at DESC",
        )
        .bind(status)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, GameSummary>(
            "SELECT id, white_user_id, black_user_id, status::text AS status, fen, created_at \
             FROM games ORDER BY created_at DESC",
        )
        .fetch_all(&state.db)
        .await
    }?;

    Ok(Json(games))
}

/// 新しい対局を作成する
#[utoipa::path(
    post,
    path = "/games",
    tag = "games",
    responses(
        (status = 200, description = "対局を新規作成", body = GameCreatedResponse),
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_game(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GameCreatedResponse>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let game_id = Uuid::new_v4();
    let position = Chess::default();
    let fen = position_to_fen(&position);

    sqlx::query("INSERT INTO games (id, white_user_id, fen) VALUES ($1, $2, $3)")
        .bind(game_id)
        .bind(user_id)
        .bind(&fen)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to insert game");
            AppError::Internal("対局の作成に失敗しました".to_string())
        })?;

    state.games.write().await.insert(game_id, position);

    tracing::info!(%game_id, %user_id, "new game created");

    Ok(Json(GameCreatedResponse { game_id, fen }))
}

/// 対局の詳細(参加者・状態・現在の盤面)を取得する
///
/// 進行中の対局はメモリ上の局面を、終了済み・サーバー再起動後の対局は
/// DB に保存された FEN を使う。
#[utoipa::path(
    get,
    path = "/games/{id}",
    tag = "games",
    params(
        ("id" = Uuid, Path, description = "対局ID"),
    ),
    responses(
        (status = 200, description = "対局の詳細", body = GameDetailResponse),
        (status = 404, description = "対局が見つからない",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn get_game(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GameDetailResponse>, AppError> {
    let row = sqlx::query_as::<_, GameDetailRow>(
        "SELECT white_user_id, black_user_id, status::text AS status, result::text AS result, fen \
         FROM games WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("対局が見つかりません".to_string()))?;

    // 進行中の対局はメモリ上の局面が正。終局するとメモリから削除されるため、
    // 無い場合は DB の FEN から局面を復元する。
    // (サーバー再起動後の進行中対局もこの経路を通る)
    let position = {
        let games = state.games.read().await;
        match games.get(&id) {
            Some(p) => p.clone(),
            None => position_from_fen(&row.fen)?,
        }
    };

    Ok(Json(GameDetailResponse {
        game_id: id,
        white_user_id: row.white_user_id,
        black_user_id: row.black_user_id,
        status: row.status,
        result: row.result,
        fen: position_to_fen(&position),
        is_check: position.is_check(),
        is_game_over: position.is_game_over(),
    }))
}
/// 対局に参加する(対戦相手として入室する)
#[utoipa::path(
    post,
    path = "/games/{id}/join",
    tag = "games",
    params(
        ("id" = Uuid, Path, description = "対局ID"),
    ),
    responses(
        (status = 200, description = "対局に参加した", body = serde_json::Value),
        (status = 400, description = "自分が作成した対局には参加できない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "対局が見つからない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "既に対戦相手がいる",
            body = ProblemDetails, content_type = "application/problem+json"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn join_game(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let game = sqlx::query_as::<_, GameRow>(
        "SELECT white_user_id, black_user_id, status::text AS status FROM games WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("対局が見つかりません".to_string()))?;

    if game.white_user_id == user_id {
        return Err(AppError::BadRequest(
            "自分が作成した対局には参加できません".to_string(),
        ));
    }

    if game.black_user_id.is_some() {
        return Err(AppError::Conflict(
            "この対局には既に対戦相手がいます".to_string(),
        ));
    }

    let result = sqlx::query(
        "UPDATE games SET black_user_id = $1, status = 'in_progress', updated_at = now() \
         WHERE id = $2 AND black_user_id IS NULL",
    )
    .bind(user_id)
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "この対局には既に対戦相手がいます".to_string(),
        ));
    }

    tracing::info!(%id, %user_id, "player joined game");

    // 先に接続している側(通常は対局作成者)へ、対戦相手が参加したことを通知する
    let _ = state
        .game_channel(id)
        .await
        .send(GameEvent::OpponentJoined { user_id });

    Ok(Json(
        serde_json::json!({ "game_id": id, "status": "in_progress" }),
    ))
}

/// 投了する
///
/// 対局の参加者が投了する。相手の勝ちとして対局を終了させる。
#[utoipa::path(
    post,
    path = "/games/{id}/resign",
    tag = "games",
    params(
        ("id" = Uuid, Path, description = "対局ID"),
    ),
    responses(
        (status = 200, description = "投了して対局を終了した", body = serde_json::Value),
        (status = 403, description = "この対局の参加者ではない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "対局が見つからない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "既に対局が終了している",
            body = ProblemDetails, content_type = "application/problem+json"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn resign_game(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let game = sqlx::query_as::<_, GameRow>(
        "SELECT white_user_id, black_user_id, status::text AS status FROM games WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("対局が見つかりません".to_string()))?;

    // 参加者本人かどうかのチェック
    let role = role_of(user_id, game.white_user_id, game.black_user_id);
    let color = role
        .color()
        .ok_or_else(|| AppError::Forbidden("この対局の参加者ではありません".to_string()))?;

    // 既に終了している対局への投了は無効
    if game.status == "finished" {
        return Err(AppError::Conflict(
            "この対局は既に終了しています".to_string(),
        ));
    }

    // 投了した側の逆が勝者
    let result = winner_after_resign(color);

    let update_result = sqlx::query(
        "UPDATE games SET status = 'finished', result = $1::game_result, end_reason = 'resignation', updated_at = now() \
         WHERE id = $2 AND status != 'finished'",
    )
    .bind(result)
    .bind(id)
    .execute(&state.db)
    .await?;

    if update_result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "この対局は既に終了しています".to_string(),
        ));
    }

    crate::rating::apply_rating(&state.db, id).await?;

    // メモリ上の対局データも削除(進行中対局の管理対象から外す)
    state.games.write().await.remove(&id);

    tracing::info!(%id, %user_id, result, "game resigned");

    // WebSocket購読者へ終局を配信(購読者がいなくてもエラーにはしない)
    let _ = state.game_channel(id).await.send(GameEvent::GameOver {
        result: result.to_string(),
        end_reason: "resignation".to_string(),
    });

    Ok(Json(
        serde_json::json!({ "game_id": id, "status": "finished", "result": result }),
    ))
}

/// 指し手を送信する
#[utoipa::path(
    post,
    path = "/games/{id}/move",
    tag = "games",
    params(
        ("id" = Uuid, Path, description = "対局ID"),
    ),
    request_body = MoveRequest,
    responses(
        (status = 200, description = "指し手を適用した結果の盤面", body = GameStateResponse),
        (status = 400, description = "指し手の形式が不正、または合法手ではない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "参加者ではない、または手番違い",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "対局が見つからない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "対戦相手がまだ参加していない",
            body = ProblemDetails, content_type = "application/problem+json"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn make_move(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<MoveRequest>,
) -> Result<Json<GameStateResponse>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let game = sqlx::query_as::<_, GameRow>(
        "SELECT white_user_id, black_user_id, status::text AS status FROM games WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("対局が見つかりません".to_string()))?;

    if !role_of(user_id, game.white_user_id, game.black_user_id).is_participant() {
        return Err(AppError::Forbidden(
            "この対局の参加者ではありません".to_string(),
        ));
    }

    let uci_move: UciMove = payload
        .uci
        .parse()
        .map_err(|e| AppError::BadRequest(format!("指し手の形式が不正です: {}", e)))?;

    let mut games = state.games.write().await;

    let position = games
        .get_mut(&id)
        .ok_or_else(|| AppError::NotFound("対局が見つかりません".to_string()))?;

    let expected = expected_player(position.turn(), game.white_user_id, game.black_user_id)
        .ok_or_else(|| AppError::Conflict("対戦相手がまだ参加していません".to_string()))?;
    if user_id != expected {
        return Err(AppError::Forbidden(
            "あなたの手番ではありません".to_string(),
        ));
    }

    let mv = uci_move
        .to_move(position)
        .map_err(|e| AppError::BadRequest(format!("不正な指し手です: {}", e)))?;

    match position.clone().play(&mv) {
        Ok(new_position) => {
            *position = new_position;

            let fen_after = position_to_fen(position);
            let move_number = position.fullmoves().get() as i32;
            let is_check = position.is_check();
            let is_game_over = position.is_game_over();

            if let Err(e) = sqlx::query(
                "INSERT INTO moves (game_id, move_number, uci, fen_after) VALUES ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(move_number)
            .bind(&payload.uci)
            .bind(&fen_after)
            .execute(&state.db)
            .await
            {
                tracing::error!(error = %e, %id, "failed to insert move");
            }

            // 盤面更新後、WebSocket購読者へ指し手を配信(購読者がいなくてもエラーにはしない)
            let _ = state.game_channel(id).await.send(GameEvent::Move {
                fen: fen_after.clone(),
                uci: payload.uci.clone(),
                is_check,
                is_game_over,
            });

            if is_game_over {
                let (result, end_reason) = determine_outcome(position);

                if let Err(e) = sqlx::query(
                    "UPDATE games SET status = 'finished', result = $1::game_result, end_reason = $2, updated_at = now() \
                     WHERE id = $3",
                )
                .bind(result)
                .bind(end_reason)
                .bind(id)
                .execute(&state.db)
                .await
                {
                    tracing::error!(error = %e, %id, "failed to update game result");
                }

                tracing::info!(%id, result, end_reason, "game finished");

                crate::rating::apply_rating(&state.db, id).await?;

                // 終局もあわせて配信
                let _ = state.game_channel(id).await.send(GameEvent::GameOver {
                    result: result.to_string(),
                    end_reason: end_reason.to_string(),
                });
            }

            tracing::info!(%id, %user_id, uci = %payload.uci, "move applied");

            Ok(Json(GameStateResponse {
                game_id: id,
                fen: fen_after,
                is_check,
                is_game_over,
            }))
        }
        Err(e) => Err(AppError::BadRequest(format!(
            "指し手を適用できません: {}",
            e
        ))),
    }
}

/// shakmatyのChess局面をFEN文字列に変換するヘルパー
pub fn position_to_fen(position: &Chess) -> String {
    Fen::from_position(position.clone(), EnPassantMode::Legal).to_string()
}

/// DB に保存された FEN から局面を復元する。
///
/// FEN が壊れているのは DB 側の異常なので 500 として扱う。
/// クライアントの入力に起因しないため 400 ではない。
fn position_from_fen(fen: &str) -> Result<Chess, AppError> {
    fen.parse::<Fen>()
        .map_err(|e| AppError::Internal(format!("保存されたFENの解析に失敗しました: {e}")))?
        .into_position(CastlingMode::Standard)
        .map_err(|e| AppError::Internal(format!("保存されたFENが不正な局面です: {e}")))
}

/// 棋譜(指し手履歴)を取得する
///
/// 対局の指し手履歴(棋譜)を手数順に取得する。
#[utoipa::path(
    get,
    path = "/games/{id}/moves",
    tag = "games",
    params(
        ("id" = Uuid, Path, description = "対局ID"),
    ),
    responses(
        (status = 200, description = "棋譜(指し手履歴)", body = Vec<MoveRow>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_moves(
    State(state): State<AppState>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<MoveRow>>, AppError> {
    extract_user_id(&headers, &state.jwt_secret)?;

    let moves = sqlx::query_as::<_, MoveRow>(
        "SELECT move_number, uci, fen_after FROM moves WHERE game_id = $1 ORDER BY move_number ASC",
    )
    .bind(game_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(moves))
}
