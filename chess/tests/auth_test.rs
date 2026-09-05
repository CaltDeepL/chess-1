use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

// テスト用にAppStateとRouterを組み立てるヘルパー
fn app(pool: PgPool) -> axum::Router {
    let state = chess_server::state::AppState {
        // 実際のAppStateのフィールドに合わせる
        games: Default::default(),
        db: pool,
        jwt_secret: std::sync::Arc::new("test-secret".to_string()),
        game_channels: Default::default(),
    };
    chess_server::build_router(state)
}

async fn post_json(pool: PgPool, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app(pool)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[sqlx::test(migrations = "./migrations")]
async fn register_returns_token(pool: PgPool) {
    let (status, body) = post_json(
        pool,
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
    let body = json!({ "username": "alice", "password": "password123" });

    let (status, _) = post_json(pool.clone(), "/auth/register", body.clone()).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = post_json(pool, "/auth/register", body).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn register_short_password_is_rejected(pool: PgPool) {
    let (status, _) = post_json(
        pool,
        "/auth/register",
        json!({ "username": "bob", "password": "short" }),
    )
    .await;

    assert_ne!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_succeeds_with_correct_password(pool: PgPool) {
    let creds = json!({ "username": "alice", "password": "password123" });
    post_json(pool.clone(), "/auth/register", creds.clone()).await;

    let (status, body) = post_json(pool, "/auth/login", creds).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_wrong_password_returns_401(pool: PgPool) {
    post_json(
        pool.clone(),
        "/auth/register",
        json!({ "username": "alice", "password": "password123" }),
    )
    .await;

    let (status, _) = post_json(
        pool,
        "/auth/login",
        json!({ "username": "alice", "password": "wrongpassword" }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_unknown_user_returns_401(pool: PgPool) {
    let (status, _) = post_json(
        pool,
        "/auth/login",
        json!({ "username": "nobody", "password": "password123" }),
    )
    .await;

    // ユーザー列挙攻撃を防ぐため、存在しないユーザーもパスワード誤りと同じ401
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}