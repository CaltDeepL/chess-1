use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess, EnPassantMode, Position};
use utoipa::ToSchema;
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
        "SELECT g.white_user_id, g.black_user_id, g.status::text AS status, \
                g.result::text AS result, \
                COALESCE((SELECT m.fen_after FROM moves m \
                          WHERE m.game_id = g.id ORDER BY m.id DESC LIMIT 1), g.fen) AS fen \
         FROM games g WHERE g.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("対局が見つかりません".to_string()))?;

    // 進行中の対局はメモリ上の局面が正。終局するとメモリから削除されるため、
    // 無い場合は DB に永続化した最後の指し手の FEN から局面を復元する。
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
        (status = 409, description = "対戦相手が未参加、または対局が終了済み",
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
    if game.status != "in_progress" {
        let detail = if game.status == "finished" {
            "この対局は既に終了しています"
        } else {
            "対戦相手がまだ参加していません"
        };
        return Err(AppError::Conflict(detail.to_string()));
    }

    // 投了した側の逆が勝者
    let result = winner_after_resign(color);

    let update_result = sqlx::query(
        "UPDATE games SET status = 'finished', result = $1::game_result, end_reason = 'resignation', updated_at = now() \
         WHERE id = $2 AND status = 'in_progress'",
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
        (status = 409, description = "対戦相手が未参加、または対局が終了済み",
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

    let game = sqlx::query_as::<_, GameDetailRow>(
        "SELECT g.white_user_id, g.black_user_id, g.status::text AS status, \
                g.result::text AS result, \
                COALESCE((SELECT m.fen_after FROM moves m \
                          WHERE m.game_id = g.id ORDER BY m.id DESC LIMIT 1), g.fen) AS fen \
         FROM games g WHERE g.id = $1",
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

    if game.status != "in_progress" {
        let detail = if game.status == "finished" {
            "この対局は既に終了しています"
        } else {
            "対戦相手がまだ参加していません"
        };
        return Err(AppError::Conflict(detail.to_string()));
    }

    let uci_move: UciMove = payload
        .uci
        .parse()
        .map_err(|e| AppError::BadRequest(format!("指し手の形式が不正です: {}", e)))?;

    let mut games = state.games.write().await;

    // サーバー再起動後はメモリ上の局面が無いため、最後に永続化した棋譜の
    // FEN から復元する。以降の成功時にキャッシュへ戻す。
    let position = match games.get(&id) {
        Some(position) => position.clone(),
        None => position_from_fen(&game.fen)?,
    };

    let expected = expected_player(position.turn(), game.white_user_id, game.black_user_id)
        .ok_or_else(|| AppError::Conflict("対戦相手がまだ参加していません".to_string()))?;
    if user_id != expected {
        return Err(AppError::Forbidden(
            "あなたの手番ではありません".to_string(),
        ));
    }

    let mv = uci_move
        .to_move(&position)
        .map_err(|e| AppError::BadRequest(format!("不正な指し手です: {}", e)))?;

    match position.play(mv) {
        Ok(new_position) => {
            let fen_after = position_to_fen(&new_position);
            let is_check = new_position.is_check();
            let is_game_over = new_position.is_game_over();

            // 棋譜と終局結果は同じトランザクションで確定する。以前は失敗を
            // ログだけに残して 200 を返しており、メモリと DB が不整合に
            // なり得た。コミット後にだけメモリと WebSocket を更新する。
            let mut tx = state.db.begin().await?;
            let persisted_status: String =
                sqlx::query_scalar("SELECT status::text FROM games WHERE id = $1 FOR UPDATE")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            if persisted_status != "in_progress" {
                return Err(AppError::Conflict(
                    "この対局は既に終了しています".to_string(),
                ));
            }

            let move_number: i32 =
                sqlx::query_scalar("SELECT count(*)::int + 1 FROM moves WHERE game_id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;

            sqlx::query(
                "INSERT INTO moves (game_id, move_number, uci, fen_after) VALUES ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(move_number)
            .bind(&payload.uci)
            .bind(&fen_after)
            .execute(&mut *tx)
            .await?;

            if is_game_over {
                let (result, end_reason) = determine_outcome(&new_position);

                let updated = sqlx::query(
                    "UPDATE games SET status = 'finished', result = $1::game_result, end_reason = $2, updated_at = now() \
                     WHERE id = $3 AND status = 'in_progress'",
                )
                .bind(result)
                .bind(end_reason)
                .bind(id)
                .execute(&mut *tx)
                .await?;

                if updated.rows_affected() == 0 {
                    return Err(AppError::Conflict(
                        "この対局は既に終了しています".to_string(),
                    ));
                }

                tx.commit().await?;
                games.remove(&id);
                drop(games);

                tracing::info!(%id, result, end_reason, "game finished");

                crate::rating::apply_rating(&state.db, id).await?;

                // DB の確定後にだけ配信する。購読者がいなくてもエラーではない。
                let channel = state.game_channel(id).await;
                let _ = channel.send(GameEvent::Move {
                    fen: fen_after.clone(),
                    uci: payload.uci.clone(),
                    is_check,
                    is_game_over,
                });

                // 終局もあわせて配信
                let _ = channel.send(GameEvent::GameOver {
                    result: result.to_string(),
                    end_reason: end_reason.to_string(),
                });
            } else {
                tx.commit().await?;
                games.insert(id, new_position);
                drop(games);

                // DB の確定後にだけ配信する。
                let _ = state.game_channel(id).await.send(GameEvent::Move {
                    fen: fen_after.clone(),
                    uci: payload.uci.clone(),
                    is_check,
                    is_game_over,
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
    Fen::from_position(position, EnPassantMode::Legal).to_string()
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
        "SELECT row_number() OVER (ORDER BY id)::int AS move_number, uci, fen_after \
         FROM moves WHERE game_id = $1 ORDER BY id ASC",
    )
    .bind(game_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(moves))
}

/// 相手の切断による勝ちを確定させる
///
/// 残っているプレイヤーの画面でカウントダウンが0になったときに呼ぶ。
///
/// **これが無いと、残っている側は既に WebSocket に接続済みなので
/// 誰も判定を起こさず、次の sweep（最大10分後）まで待たされる。**
/// 接続時の判定と sweep の隙間を埋める経路。
///
/// 判定そのものは `finish_if_abandoned` に任せるので、猶予前に呼ばれても
/// 何も起きない。クライアントの時計を信用する必要がない。
#[utoipa::path(
    post,
    path = "/games/{id}/claim-abandonment",
    tag = "games",
    params(("id" = Uuid, Path, description = "対局ID")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "判定結果。finished が false なら猶予内", body = ClaimResponse),
        (status = 401, description = "認証が必要",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "対局の参加者ではない",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn claim_abandonment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ClaimResponse>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    // 参加者しか判定を起こせない。無関係な相手に連打されて
    // DBへの問い合わせが増えるのを防ぐ
    let is_participant: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM games \
         WHERE id = $1 AND (white_user_id = $2 OR black_user_id = $2))",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    if !is_participant {
        return Err(AppError::Forbidden(
            "この対局の参加者ではありません".to_string(),
        ));
    }

    let finished = crate::abandon::finish_if_abandoned(&state, id).await?;

    Ok(Json(ClaimResponse { finished }))
}

#[derive(Serialize, ToSchema)]
pub struct ClaimResponse {
    /// 対局を終了させたかどうか。false は「まだ猶予内」
    pub finished: bool,
}
