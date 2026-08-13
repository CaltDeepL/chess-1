use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::State, http::HeaderMap, Json};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuthResponse, Claims, LoginRequest, RegisterRequest, UserRow};
use crate::state::AppState;

/// POST /auth/register
/// ユーザー名とパスワードを受け取り、Argon2でハッシュ化してDBに保存する。
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    if payload.username.trim().is_empty() || payload.password.len() < 8 {
        return Err(AppError::BadRequest(
            "ユーザー名は必須、パスワードは8文字以上にしてください".to_string(),
        ));
    }

    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("パスワードのハッシュ化に失敗しました: {}", e)))?
        .to_string();

    let user_id = Uuid::new_v4();

    let result = sqlx::query("INSERT INTO users (id, username, password_hash) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&payload.username)
        .bind(&password_hash)
        .execute(&state.db)
        .await;

    if let Err(e) = result {
        tracing::warn!(error = %e, "register failed");
        return Err(AppError::Conflict("そのユーザー名は既に使われています".to_string()));
    }

    let token = issue_token(user_id, &state.jwt_secret)?;

    tracing::info!(%user_id, username = %payload.username, "user registered");

    Ok(Json(AuthResponse { user_id, token }))
}

/// POST /auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user = sqlx::query_as::<_, UserRow>("SELECT id, password_hash FROM users WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("ユーザー名またはパスワードが違います".to_string()))?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("保存済みハッシュの読み取りに失敗しました: {}", e)))?;

    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized("ユーザー名またはパスワードが違います".to_string()))?;

    let token = issue_token(user.id, &state.jwt_secret)?;

    tracing::info!(user_id = %user.id, "user logged in");

    Ok(Json(AuthResponse { user_id: user.id, token }))
}

/// 指定ユーザーIDに対するJWTを発行するヘルパー。有効期限は24時間。
pub fn issue_token(user_id: Uuid, jwt_secret: &str) -> Result<String, AppError> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("有効なタイムスタンプの計算に失敗しました")
        .timestamp() as usize;

    let claims = Claims { sub: user_id, exp: expiration };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret.as_bytes()))
        .map_err(|e| AppError::Internal(format!("トークン発行に失敗しました: {}", e)))
}

/// JWTを検証してユーザーIDを取り出すヘルパー。
pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Uuid, AppError> {
    decode::<Claims>(token, &DecodingKey::from_secret(jwt_secret.as_bytes()), &Validation::default())
        .map(|data| data.claims.sub)
        .map_err(|e| AppError::Unauthorized(format!("トークンが無効です: {}", e)))
}

/// Authorizationヘッダー(Bearer方式)からユーザーIDを取り出すヘルパー
pub fn extract_user_id(headers: &HeaderMap, jwt_secret: &str) -> Result<Uuid, AppError> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("認証トークンがありません".to_string()))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        AppError::Unauthorized(
            "Authorizationヘッダーの形式が不正です(Bearer <token>の形式で送ってください)".to_string(),
        )
    })?;

    verify_token(token, jwt_secret)
}