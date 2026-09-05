mod common;

use axum::http::{header, StatusCode};
use common::*;
use serde_json::json;
use sqlx::PgPool;

/// エラーレスポンスが RFC 9457 (application/problem+json) の形を満たすことを検証する。
///
/// `AppError` の `IntoResponse` は `Json` が付ける `application/json` を
/// 後段のヘッダで上書きしている。この上書きが外れても本文は返るため、
/// ヘッダを明示的に assert しないと壊れたことに気づけない。
#[sqlx::test(migrations = "./migrations")]
async fn bad_request_returns_problem_details(pool: PgPool) {
    let state = test_state(pool);

    let (status, headers, body) = post_json_raw(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "short" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );

    assert_eq!(body["type"], "/problems/bad-request");
    assert_eq!(body["title"], "Bad request");
    // status メンバーは HTTP ステータスと一致していなければならない(RFC 9457 §3.1.2)
    assert_eq!(body["status"], 400);
    assert!(
        body["detail"].as_str().is_some_and(|s| !s.is_empty()),
        "detail に利用者向けの文言が入っている"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn conflict_returns_problem_details(pool: PgPool) {
    let state = test_state(pool);
    let creds = json!({ "username": "alice", "password": "password123" });

    post_json(&state, "/auth/register", creds.clone()).await;
    let (status, _, body) = post_json_raw(&state, "/auth/register", creds).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["type"], "/problems/conflict");
    assert_eq!(body["status"], 409);
}

#[sqlx::test(migrations = "./migrations")]
async fn unauthorized_returns_problem_details(pool: PgPool) {
    let state = test_state(pool);

    let (status, _, body) = post_json_raw(
        &state,
        "/auth/login",
        json!({ "username": "nobody", "password": "password123" }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["type"], "/problems/unauthorized");
    assert_eq!(body["status"], 401);
}
