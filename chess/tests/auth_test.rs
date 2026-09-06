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
        json!({ "username": "alice", "password": "password123456" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert!(body["user_id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn register_duplicate_username_returns_409(pool: PgPool) {
    let state = test_state(pool);
    let body = json!({ "username": "alice", "password": "password123456" });

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
    let creds = json!({ "username": "alice", "password": "password123456" });

    post_json(&state, "/auth/register", creds.clone()).await;
    let (status, body) = post_json(&state, "/auth/login", creds).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_wrong_password_returns_401(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "password123456" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "事前の登録が失敗している");

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

/// 12文字未満は拒否される（最小文字数を8→12に引き上げた分）
#[sqlx::test(migrations = "./migrations")]
async fn register_rejects_password_shorter_than_12(pool: PgPool) {
    let state = test_state(pool);

    // 従来は通っていた9文字
    let (status, body) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "abcdefghi" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    // 何文字必要かが分かる文言であること
    assert!(
        body["detail"].as_str().unwrap().contains("12"),
        "必要な文字数が伝わらない: {}",
        body["detail"]
    );
}

/// 記号や数字を含まない長いパスフレーズは受け入れる
#[sqlx::test(migrations = "./migrations")]
async fn register_accepts_a_long_passphrase(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "my cat sleeps on the keyboard" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

/// 長さは文字数で数える（バイト数ではない）
#[sqlx::test(migrations = "./migrations")]
async fn register_counts_password_length_in_chars(pool: PgPool) {
    let state = test_state(pool);

    // 7文字だが21バイト。バイト数で数えていると通ってしまう
    let (status, _) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "正しい馬の電池" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// ありふれたパスワードは拒否される
#[sqlx::test(migrations = "./migrations")]
async fn register_rejects_a_common_password(pool: PgPool) {
    let state = test_state(pool);

    let (status, body) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "password1234" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["type"], "/problems/bad-request");
}

/// 極端に長いパスワードは拒否される（Argon2 でのDoSを防ぐ）
#[sqlx::test(migrations = "./migrations")]
async fn register_rejects_an_absurdly_long_password(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "a".repeat(10_000) }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// ユーザー名を含むパスワードは拒否される
#[sqlx::test(migrations = "./migrations")]
async fn register_rejects_password_containing_username(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(
        &state,
        "/auth/register",
        json!({ "username": "alice", "password": "aliceisgreat123" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 既存ユーザーは短いパスワードでもログインできる
///
/// 要件の引き上げは「これから作るパスワード」に適用するもの。
/// login でも検証すると、8〜11文字で登録済みのユーザーが
/// 自分のアカウントに入れなくなる。
#[sqlx::test(migrations = "./migrations")]
async fn existing_short_password_can_still_log_in(pool: PgPool) {
    let state = test_state(pool);

    // 新要件を通らない長さのパスワードを、DBに直接作る
    // （register 経由では作れないため）
    let user_id = uuid::Uuid::new_v4();
    let salt = argon2::password_hash::SaltString::generate(&mut rand::thread_rng());
    let hash =
        argon2::PasswordHasher::hash_password(&argon2::Argon2::default(), b"oldpass1", &salt)
            .unwrap()
            .to_string();

    sqlx::query("INSERT INTO users (id, username, password_hash) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("legacy")
        .bind(&hash)
        .execute(&state.db)
        .await
        .unwrap();

    let (status, _) = post_json(
        &state,
        "/auth/login",
        json!({ "username": "legacy", "password": "oldpass1" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "既存ユーザーが締め出されている");
}
