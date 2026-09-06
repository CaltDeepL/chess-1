use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::abandon::decide;
use crate::domain::abandon::GRACE_SECONDS;
use crate::errors::AppError;
use crate::state::AppState;

/// 対局ごとの接続数を数えるキー
pub type ConnectionKey = (Uuid, Uuid); // (game_id, user_id)

/// WebSocket が接続したことを記録し、切断時刻を消す。
///
/// 同じユーザーが複数タブを開いている場合があるため接続数を数える。
/// 1本目の接続でのみ DB を更新する。
pub async fn mark_connected(
    state: &AppState,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let is_first = {
        let mut conns = state.game_connections.write().await;
        let count = conns.entry((game_id, user_id)).or_insert(0);
        *count += 1;
        *count == 1
    };

    if is_first {
        let was_disconnected = clear_disconnected_at(&state.db, game_id, user_id).await?;

        // 初回参加(切断していなかった)まで「復帰」として配信すると、
        // 自分自身が直前に購読したチャンネルへ自分宛のイベントが
        // 積まれてしまい、その直後に届くはずの本来のイベントより
        // 先に読まれてテストや実装が混乱する
        if was_disconnected {
            if let Some(tx) = state.game_channels.read().await.get(&game_id) {
                let _ = tx.send(crate::state::GameEvent::PlayerReconnected { user_id });
            }
        }
    }

    Ok(())
}

/// WebSocket が切断されたことを記録する。
///
/// 接続が0本になったときだけ切断時刻を書く。タブを1枚閉じただけで
/// 切断扱いにすると誤判定になる。
pub async fn mark_disconnected(
    state: &AppState,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let is_last = {
        let mut conns = state.game_connections.write().await;
        match conns.get_mut(&(game_id, user_id)) {
            Some(count) => {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    conns.remove(&(game_id, user_id));
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    };

    if is_last {
        set_disconnected_at(&state.db, game_id, user_id).await?;

        if let Some(tx) = state.game_channels.read().await.get(&game_id) {
            let _ = tx.send(crate::state::GameEvent::PlayerDisconnected {
                user_id,
                remaining_seconds: GRACE_SECONDS,
            });
        }
    }

    Ok(())
}

/// 切断時刻を消す。戻り値は「実際に切断していたか」
/// (= PlayerReconnected を配信すべきか呼び出し側が判断するため)。
async fn clear_disconnected_at(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<bool, AppError> {
    // prev CTE で更新前の値を取っておき、RETURNING で「更新前に
    // 切断時刻が入っていたか」を返す(UPDATE後のRETURNINGは新しい値
    // しか見えないため、更新前の値を別途確保する必要がある)。
    // 白か黒かを SQL 側で判定する。呼び出し側が色を知る必要をなくす
    let was_disconnected: Option<bool> = sqlx::query_scalar(
        "WITH prev AS (\
           SELECT white_user_id, black_user_id, white_disconnected_at, black_disconnected_at \
           FROM games WHERE id = $1\
         ) \
         UPDATE games SET \
           white_disconnected_at = CASE WHEN prev.white_user_id = $2 THEN NULL ELSE games.white_disconnected_at END, \
           black_disconnected_at = CASE WHEN prev.black_user_id = $2 THEN NULL ELSE games.black_disconnected_at END \
         FROM prev \
         WHERE games.id = $1 \
         RETURNING \
           (prev.white_user_id = $2 AND prev.white_disconnected_at IS NOT NULL) \
           OR (prev.black_user_id = $2 AND prev.black_disconnected_at IS NOT NULL)",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(was_disconnected.unwrap_or(false))
}

async fn set_disconnected_at(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    // 既に値が入っている場合は上書きしない。上書きすると、
    // 短時間の再接続を繰り返すことで猶予を無限に延ばせてしまう
    sqlx::query(
        "UPDATE games SET \
           white_disconnected_at = CASE \
             WHEN white_user_id = $2 AND white_disconnected_at IS NULL THEN now() \
             ELSE white_disconnected_at END, \
           black_disconnected_at = CASE \
             WHEN black_user_id = $2 AND black_disconnected_at IS NULL THEN now() \
             ELSE black_disconnected_at END \
         WHERE id = $1 AND status = 'in_progress'",
    )
    .bind(game_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 1局を判定し、放棄が成立していれば終了させる。
///
/// WebSocket 接続時と sweep の両方から呼ぶ。判定の条件を1箇所に置くため、
/// 呼び出し元ごとに書かない（task-34 の apply_rating と同じ方針）。
///
/// 戻り値は「終了させたか」。
pub async fn finish_if_abandoned(state: &AppState, game_id: Uuid) -> Result<bool, AppError> {
    let row = sqlx::query(
        "SELECT white_disconnected_at, black_disconnected_at, black_user_id \
         FROM games WHERE id = $1 AND status = 'in_progress'",
    )
    .bind(game_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok(false);
    };

    // 相手が未参加の対局は放棄の対象外。勝敗に意味がない
    if row.try_get::<Option<Uuid>, _>("black_user_id")?.is_none() {
        return Ok(false);
    }

    let white_at: Option<DateTime<Utc>> = row.try_get("white_disconnected_at")?;
    let black_at: Option<DateTime<Utc>> = row.try_get("black_disconnected_at")?;

    let Some(abandonment) = decide(white_at, black_at, Utc::now()) else {
        return Ok(false);
    };

    // 条件付き UPDATE にすることで、複数のプロセスが同時に判定しても
    // 実際に終了させるのは1つだけになる
    let updated = sqlx::query(
        "UPDATE games SET status = 'finished', result = $1::game_result, \
           end_reason = $2, updated_at = now() \
         WHERE id = $3 AND status = 'in_progress'",
    )
    .bind(abandonment.result)
    .bind(abandonment.end_reason)
    .bind(game_id)
    .execute(&state.db)
    .await?;

    if updated.rows_affected() == 0 {
        return Ok(false);
    }

    // メモリ上の局面を片付ける（終局時の既存処理に合わせること）
    state.games.write().await.remove(&game_id);

    crate::rating::apply_rating(&state.db, game_id).await?;

    // 残っている側の画面に反映する。誰も接続していなければ送信は失敗するが、
    // それは正常なので無視してよい
    if let Some(tx) = state.game_channels.read().await.get(&game_id) {
        let _ = tx.send(crate::state::GameEvent::GameOver {
            result: abandonment.result.to_string(),
            end_reason: abandonment.end_reason.to_string(),
        });
    }

    tracing::info!(
        %game_id,
        result = abandonment.result,
        end_reason = abandonment.end_reason,
        "game finished by abandonment"
    );

    Ok(true)
}

/// sweep の排他用 advisory lock ID。0x53574545（"SWEE"）の10進表記。
/// 他の用途で advisory lock を使うときは、必ず違う値にすること。
const SWEEP_LOCK_ID: i64 = 1_398_228_293;

/// 進行中の対局をまとめて判定する。
///
/// 双方が画面を閉じていると誰も判定を起こさないため、外部からの
/// 定期実行で叩く（ops-hub と同じ考え方）。
pub async fn sweep(state: &AppState) -> Result<usize, AppError> {
    // 同時に複数の sweep が走ると同じ対局を二重に処理しようとする。
    // advisory lock で1つだけに絞る。取れなければ何もせず成功扱い
    // （エラーにすると定期実行の失敗として通知が飛んでしまう）
    //
    // lock/unlock は同一セッション(=同一コネクション)でなければならない。
    // `&state.db`(プール)に対して呼ぶと、呼び出しごとに別の接続が
    // 割り当てられうるため、unlock が別の接続に飛んで解放に失敗し、
    // lock を取った接続がプールの中で永久にロックを持ち続けてしまう
    // (ops-hub の RunLock が pool.acquire() で1本に固定しているのと同じ理由)。
    let mut conn = state.db.acquire().await?;

    let lock: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(SWEEP_LOCK_ID)
        .fetch_one(&mut *conn)
        .await?;

    if !lock {
        tracing::info!("sweep: 別のプロセスが実行中のためスキップ");
        return Ok(0);
    }

    let result = sweep_inner(state).await;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SWEEP_LOCK_ID)
        .execute(&mut *conn)
        .await?;

    result
}

async fn sweep_inner(state: &AppState) -> Result<usize, AppError> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM games \
         WHERE status = 'in_progress' \
           AND black_user_id IS NOT NULL \
           AND (white_disconnected_at IS NOT NULL OR black_disconnected_at IS NOT NULL)",
    )
    .fetch_all(&state.db)
    .await?;

    let mut finished = 0;
    for id in ids {
        // 1局の失敗で全体を止めない。次の実行で再度拾える
        match finish_if_abandoned(state, id).await {
            Ok(true) => finished += 1,
            Ok(false) => {}
            Err(e) => tracing::error!(game_id = %id, error = ?e, "sweep: 判定に失敗"),
        }
    }

    Ok(finished)
}

/// ログアウト時に、その人が参加している進行中の対局をすべて終わらせる。
///
/// 明示的なログアウトは意図的な離脱なので猶予を与えない。切断（ブラウザを
/// 閉じた、回線が切れた）と違い、本人が「やめる」と操作しているため。
///
/// 相手がまだ参加していない対局は、勝敗ではなく取り消しとして終了させる。
/// 放置するとロビーに参加できない対局が残り続ける。
pub async fn forfeit_active_games(state: &AppState, user_id: Uuid) -> Result<usize, AppError> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM games \
         WHERE status IN ('waiting', 'in_progress') \
           AND (white_user_id = $1 OR black_user_id = $1)",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let mut count = 0;
    for game_id in ids {
        // 1局の失敗で残りを止めない。ログアウト自体は成功させたい
        match forfeit_one(state, game_id, user_id).await {
            Ok(true) => count += 1,
            Ok(false) => {}
            Err(e) => tracing::error!(%game_id, error = ?e, "ログアウト時の終了処理に失敗"),
        }
    }

    Ok(count)
}

async fn forfeit_one(state: &AppState, game_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let row = sqlx::query(
        "SELECT white_user_id, black_user_id FROM games \
         WHERE id = $1 AND status IN ('waiting', 'in_progress')",
    )
    .bind(game_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok(false);
    };

    let white_id: Uuid = row.try_get("white_user_id")?;
    let black_id: Option<Uuid> = row.try_get("black_user_id")?;

    // 相手が未参加なら取り消し扱い。result は NULL のまま
    let (result, end_reason) = match black_id {
        None => (None, "cancelled"),
        Some(_) if user_id == white_id => (Some("black_win"), "logout"),
        Some(_) => (Some("white_win"), "logout"),
    };

    let updated = sqlx::query(
        "UPDATE games SET status = 'finished', result = $1::game_result, \
           end_reason = $2, updated_at = now() \
         WHERE id = $3 AND status IN ('waiting', 'in_progress')",
    )
    .bind(result)
    .bind(end_reason)
    .bind(game_id)
    .execute(&state.db)
    .await?;

    if updated.rows_affected() == 0 {
        return Ok(false);
    }

    state.games.write().await.remove(&game_id);

    // 取り消しにレーティングは適用しない。apply_rating 側も result が
    // NULL なら何もしないので二重の防御になっている
    if result.is_some() {
        crate::rating::apply_rating(&state.db, game_id).await?;
    }

    if let (Some(result), Some(tx)) = (result, state.game_channels.read().await.get(&game_id)) {
        let _ = tx.send(crate::state::GameEvent::GameOver {
            result: result.to_string(),
            end_reason: end_reason.to_string(),
        });
    }

    tracing::info!(%game_id, %user_id, end_reason, "game finished by logout");

    Ok(true)
}

/// 相手の切断残り秒数を返す。接続中、または既に猶予を過ぎていれば `None`。
///
/// 再接続した人に「相手はあと何秒で切断確定か」を伝えるために使う。
/// **残り秒数を返すのは、時刻を返すとクライアントとサーバーの時計のずれが
/// そのままカウントダウンの誤差になるため。** 受け取った側が
/// 「今から N 秒後」として扱えば、ずれの影響を受けない。
pub async fn opponent_grace_remaining(
    state: &AppState,
    game_id: Uuid,
    viewer_id: Uuid,
) -> Result<Option<(Uuid, i64)>, AppError> {
    let row = sqlx::query(
        "SELECT white_user_id, black_user_id, white_disconnected_at, black_disconnected_at \
         FROM games WHERE id = $1 AND status = 'in_progress'",
    )
    .bind(game_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let white_id: Uuid = row.try_get("white_user_id")?;
    let Some(black_id) = row.try_get::<Option<Uuid>, _>("black_user_id")? else {
        return Ok(None);
    };

    let (opponent_id, disconnected_at) = if viewer_id == white_id {
        (
            black_id,
            row.try_get::<Option<DateTime<Utc>>, _>("black_disconnected_at")?,
        )
    } else if viewer_id == black_id {
        (
            white_id,
            row.try_get::<Option<DateTime<Utc>>, _>("white_disconnected_at")?,
        )
    } else {
        return Ok(None);
    };

    let Some(at) = disconnected_at else {
        return Ok(None);
    };

    let elapsed = (Utc::now() - at).num_seconds();
    let remaining = GRACE_SECONDS - elapsed;

    if remaining <= 0 {
        // 既に猶予切れ。呼び出し側が finish_if_abandoned を通っているはずなので
        // ここでカウントダウンを出す意味はない
        return Ok(None);
    }

    Ok(Some((opponent_id, remaining)))
}
