#![allow(dead_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chess_server::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

/// テスト全体で共有する AppState を作る
pub fn test_state(pool: PgPool) -> AppState {
    AppState {
        games: Default::default(),
        db: pool,
        jwt_secret: std::sync::Arc::new("test-secret".to_string()),
        game_channels: Default::default(),
    }
}

async fn send(state: &AppState, req: Request<Body>) -> (StatusCode, Value) {
    // state は Clone で内部の Arc を共有するため、メモリ上の対局が引き継がれる
    let response = chess_server::build_router(state.clone())
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

pub async fn post_json(state: &AppState, path: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(state, req).await
}

pub async fn post_auth(state: &AppState, path: &str, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    send(state, req).await
}

pub async fn post_auth_json(
    state: &AppState,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(state, req).await
}

pub async fn get_auth(state: &AppState, path: &str, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    send(state, req).await
}

/// 認証なしのGET(JSONレスポンス)
pub async fn get_json(state: &AppState, path: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .unwrap();
    send(state, req).await
}

/// HTMLを返すエンドポイント用(ボディはパースせずステータスのみ確認)
pub async fn get_html(state: &AppState, path: &str) -> (StatusCode, String) {
    let response = chess_server::build_router(state.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

pub async fn register_user(state: &AppState, username: &str) -> String {
    let (status, body) = post_json(
        state,
        "/auth/register",
        json!({ "username": username, "password": "password123" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register failed: {body}");
    body["token"].as_str().unwrap().to_string()
}

pub async fn create_game(state: &AppState, token: &str) -> String {
    let (status, body) = post_auth(state, "/games", token).await;
    assert_eq!(status, StatusCode::OK, "create_game failed: {body}");
    body["game_id"].as_str().unwrap().to_string()
}

pub async fn make_move(
    state: &AppState,
    game_id: &str,
    token: &str,
    uci: &str,
) -> (StatusCode, Value) {
    post_auth_json(
        state,
        &format!("/games/{game_id}/move"),
        token,
        json!({ "uci": uci }),
    )
    .await
}
