mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn register_returns_token(pool: PgPool) {
    let state = test_state(pool);

    let (status, body) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "password123" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert!(body["user_id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn register_duplicate_username_returns_409(pool: PgPool) {
    let state = test_state(pool);
    let body = json!({ "username": "alice", "password": "password123" });

    let (status, _) = post_json(&state, "/auth/register", body.clone()).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = post_json(&state, "/auth/register", body).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn register_short_password_is_rejected(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "bob", "password": "short" }),
    )
    .await;

    assert_ne!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_succeeds_with_correct_password(pool: PgPool) {
    let state = test_state(pool);
    let creds = json!({ "username": "alice", "password": "password123" });

    post_json(&state, "/auth/register", creds.clone()).await;
    let (status, body) = post_json(&state, "/auth/login", creds).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_wrong_password_returns_401(pool: PgPool) {
    let state = test_state(pool);

    post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "password123" }),
    )
    .await;

    let (status, _) = post_json(
        &state,
        "/auth/login",
        json!({ "username": "alice", "password": "wrongpassword" }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_unknown_user_returns_401(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(
        &state,
        "/auth/login",
        json!({ "username": "nobody", "password": "password123" }),
    )
    .await;

    // ユーザー列挙攻撃を防ぐため、存在しないユーザーもパスワード誤りと同じ401
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}