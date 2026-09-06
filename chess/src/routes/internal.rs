use axum::{extract::State, http::HeaderMap, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::errors::AppError;
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct SweepResponse {
    /// 今回終了させた対局の数
    pub finished: usize,
}

/// POST /internal/sweep
///
/// 切断されたまま猶予を過ぎた対局を終了させる。GitHub Actions から
/// 定期的に叩く。双方が画面を閉じていると誰も判定を起こさないため、
/// 外から蹴る仕組みが要る（ops-hub のデッドマンスイッチと同じ構図）。
///
/// OpenAPI には載せない。利用者向けの API ではなく運用用のため、
/// 公開仕様に混ぜると「このアプリの使い方」が読みにくくなる。
pub async fn sweep(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SweepResponse>, AppError> {
    verify_sweep_token(&headers, &state.sweep_token)?;

    let finished = crate::abandon::sweep(&state).await?;

    Ok(Json(SweepResponse { finished }))
}

/// 共有シークレットで認証する。
///
/// JWT ではなく共有シークレットにしたのは、呼び出し元が利用者ではなく
/// CI だから。ユーザーを作ってトークンを発行させると、その資格情報が
/// 通常のログインにも使えてしまう。
fn verify_sweep_token(headers: &HeaderMap, expected: &str) -> Result<(), AppError> {
    // 未設定のまま公開すると誰でも叩ける。起動時に落とすのが理想だが、
    // ここでも防ぐ
    if expected.is_empty() {
        return Err(AppError::Internal(
            "SWEEP_TOKEN が設定されていません".to_string(),
        ));
    }

    let provided = headers
        .get("x-sweep-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // 比較時間から正解の文字数が推測されるのを防ぐため、
    // 長さが違っても同じだけ比較する
    if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return Err(AppError::Unauthorized(
            "sweep トークンが不正です".to_string(),
        ));
    }

    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
