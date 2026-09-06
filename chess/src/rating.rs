use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::elo::{score_from_result, white_delta};
use crate::errors::AppError;

/// 終局した対局のレーティングを両者に適用する。
///
/// 投了・チェックメイト・引き分けのいずれの経路からも、対局を finished に
/// 更新した直後に呼ぶ。経路ごとに計算を書くと、片方だけ更新漏れが起きても
/// 気づけない（task-07 の ENUM キャスト漏れと同じ構図）。
///
/// 冪等: `white_rating_delta` が既に入っている対局には何もしない。
///
/// 黒が未参加のまま終了した対局（`black_user_id IS NULL`）は対象外。
/// 相手がいないのでレーティングを動かす意味がない。
pub async fn apply_rating(pool: &PgPool, game_id: Uuid) -> Result<(), AppError> {
    // レーティングの読み取りと書き込みの間に別の対局が割り込むと、
    // 片方の更新が失われる（lost update）。トランザクション内で
    // FOR UPDATE を掛けて直列化する。
    let mut tx = pool.begin().await?;

    let row = sqlx::query(
        "SELECT white_user_id, black_user_id, result::text AS result, white_rating_delta \
         FROM games WHERE id = $1 FOR UPDATE",
    )
    .bind(game_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        // 対局が無いのは呼び出し側の異常だが、終局処理を巻き戻すほどではない
        tracing::warn!(%game_id, "apply_rating: 対局が見つからない");
        return Ok(());
    };

    // 既に適用済み
    if row
        .try_get::<Option<i32>, _>("white_rating_delta")?
        .is_some()
    {
        return Ok(());
    }

    let white_id: Uuid = row.try_get("white_user_id")?;
    let Some(black_id) = row.try_get::<Option<Uuid>, _>("black_user_id")? else {
        return Ok(());
    };
    let Some(result) = row.try_get::<Option<String>, _>("result")? else {
        tracing::warn!(%game_id, "apply_rating: result が未設定");
        return Ok(());
    };
    let Some(score) = score_from_result(&result) else {
        tracing::warn!(%game_id, %result, "apply_rating: 未知の result");
        return Ok(());
    };

    // ID の順で必ずロックすることでデッドロックを避ける。
    // 2局が同じ2人を含む場合、逆順にロックすると互いに待ち合う。
    let (first, second) = if white_id < black_id {
        (white_id, black_id)
    } else {
        (black_id, white_id)
    };

    let ratings =
        sqlx::query("SELECT id, rating FROM users WHERE id IN ($1, $2) ORDER BY id FOR UPDATE")
            .bind(first)
            .bind(second)
            .fetch_all(&mut *tx)
            .await?;

    let mut white_rating = 1500;
    let mut black_rating = 1500;
    for r in &ratings {
        let id: Uuid = r.try_get("id")?;
        let rating: i32 = r.try_get("rating")?;
        if id == white_id {
            white_rating = rating;
        } else if id == black_id {
            black_rating = rating;
        }
    }

    let delta = white_delta(white_rating, black_rating, score);

    sqlx::query("UPDATE users SET rating = rating + $1 WHERE id = $2")
        .bind(delta)
        .bind(white_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE users SET rating = rating - $1 WHERE id = $2")
        .bind(delta)
        .bind(black_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE games SET white_rating_delta = $1, black_rating_delta = $2 WHERE id = $3")
        .bind(delta)
        .bind(-delta)
        .bind(game_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    tracing::info!(%game_id, delta, "rating applied");

    Ok(())
}

// ---------------------------------------------------------------------------
// 呼び出し側（src/routes/game.rs）
//
// 対局を finished に更新した直後、WebSocket で game_over を配信する前に
// 1行を足す。resign / チェックメイト / 引き分けの全経路が対象。
// ---------------------------------------------------------------------------
//
//     // 既存: games を finished に更新する処理
//     ...
//
//     crate::rating::apply_rating(&state.db, game_id).await?;
//
//     // 既存: WebSocket へ game_over を配信
//     ...
