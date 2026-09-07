use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::State, http::HeaderMap, Json};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::password::{validate_password, validate_username};
use crate::errors::{AppError, ProblemDetails};
use crate::models::{AuthResponse, Claims, LoginRequest, RegisterRequest, UserRow};
use crate::state::AppState;

/// ユーザーを新規登録し、JWTトークンを発行する
///
/// ユーザー名とパスワードを受け取り、Argon2でハッシュ化してDBに保存する。
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "登録成功。ユーザーIDとJWTトークンを返す", body = AuthResponse),
        (status = 400, description = "ユーザー名またはパスワードが要件を満たしていない",
            body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "そのユーザー名は既に使われている",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    validate_username(&payload.username).map_err(|e| AppError::BadRequest(e.detail()))?;
    validate_password(&payload.password, &payload.username)
        .map_err(|e| AppError::BadRequest(e.detail()))?;

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
        return Err(AppError::Conflict(
            "そのユーザー名は既に使われています".to_string(),
        ));
    }

    let token = issue_token(user_id, &state.jwt_secret)?;

    tracing::info!(%user_id, username = %payload.username, "user registered");

    Ok(Json(AuthResponse { user_id, token }))
}

/// ログインしてJWTを発行する
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "ログイン成功", body = AuthResponse),
        (status = 401, description = "ユーザー名またはパスワードが違う",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user =
        sqlx::query_as::<_, UserRow>("SELECT id, password_hash FROM users WHERE username = $1")
            .bind(&payload.username)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized("ユーザー名またはパスワードが違います".to_string())
            })?;

    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|e| {
        AppError::Internal(format!("保存済みハッシュの読み取りに失敗しました: {}", e))
    })?;

    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized("ユーザー名またはパスワードが違います".to_string()))?;

    let token = issue_token(user.id, &state.jwt_secret)?;

    tracing::info!(user_id = %user.id, "user logged in");

    Ok(Json(AuthResponse {
        user_id: user.id,
        token,
    }))
}

/// ログアウトする
///
/// JWT はサーバー側に状態を持たないため、トークンの無効化は行わない
/// （フロントが保持をやめるだけ）。このエンドポイントの役割は、
/// **進行中の対局を終わらせること**。
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "ログアウト完了。終了させた対局数を返す", body = LogoutResponse),
        (status = 401, description = "認証が必要",
            body = ProblemDetails, content_type = "application/problem+json"),
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LogoutResponse>, AppError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let forfeited = crate::abandon::forfeit_active_games(&state, user_id).await?;

    tracing::info!(%user_id, forfeited, "user logged out");

    Ok(Json(LogoutResponse { forfeited }))
}

#[derive(Serialize, ToSchema)]
pub struct LogoutResponse {
    /// 終了させた対局の数
    pub forfeited: usize,
}

/// 指定ユーザーIDに対するJWTを発行するヘルパー。有効期限は24時間。
pub fn issue_token(user_id: Uuid, jwt_secret: &str) -> Result<String, AppError> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("有効なタイムスタンプの計算に失敗しました")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("トークン発行に失敗しました: {}", e)))
}

/// JWTを検証してユーザーIDを取り出すヘルパー。
pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Uuid, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
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
            "Authorizationヘッダーの形式が不正です(Bearer <token>の形式で送ってください)"
                .to_string(),
        )
    })?;

    verify_token(token, jwt_secret)
}


#[cfg(test)]
mod tests {
    use super::*;
 
    const SECRET: &str = "test-secret-for-unit-tests";
 
    /// 発行したトークンから同じユーザーIDが取り出せる
    ///
    /// jsonwebtoken v11 は暗号バックエンドの明示的な選択（rust_crypto /
    /// aws_lc_rs）が必須で、指定を忘れると署名・検証時に CryptoProvider が
    /// 見つからず **panic する**。コンパイルは通るため、この経路を通る
    /// テストが無いと本番で初めて落ちる。
    #[test]
    fn valid_token_round_trips() {
        let user_id = Uuid::new_v4();
        let token = issue_token(user_id, SECRET).unwrap();
 
        assert_eq!(verify_token(&token, SECRET).unwrap(), user_id);
    }
 
    /// 期限切れのトークンは拒否される
    ///
    /// verify_token は Validation::default() を使っており、有効期限の
    /// 検証はその既定値に依存している。jsonwebtoken を上げたときに
    /// 既定値が変わって検証が緩くなっても、コンパイルエラーにはならない。
    #[test]
    fn expired_token_is_rejected() {
        let claims = Claims {
            sub: Uuid::new_v4(),
            exp: (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();
 
        assert!(
            verify_token(&token, SECRET).is_err(),
            "期限切れのトークンが通ってしまった"
        );
    }
 
    /// 有効期限内なら受け入れる
    ///
    /// 上の expired_token_is_rejected だけだと「常に拒否している」実装でも
    /// 通ってしまうため、対になる確認を置く
    #[test]
    fn token_within_expiry_is_accepted() {
        let user_id = Uuid::new_v4();
        let claims = Claims {
            sub: user_id,
            exp: (chrono::Utc::now() + chrono::Duration::minutes(1)).timestamp() as usize,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();
 
        assert_eq!(verify_token(&token, SECRET).unwrap(), user_id);
    }
 
    /// 別の鍵で署名されたトークンは拒否される
    #[test]
    fn token_signed_with_another_secret_is_rejected() {
        let token = issue_token(Uuid::new_v4(), "secret-a").unwrap();
 
        assert!(
            verify_token(&token, "secret-b").is_err(),
            "署名の検証が効いていない"
        );
    }
 
    /// 改ざんされたトークンは拒否される
    #[test]
    fn tampered_token_is_rejected() {
        let token = issue_token(Uuid::new_v4(), SECRET).unwrap();
        // ペイロード部（2番目のセグメント）の末尾を1文字変える
        let mut parts: Vec<&str> = token.split('.').collect();
        let tampered_payload = format!("{}A", parts[1]);
        parts[1] = &tampered_payload;
        let tampered = parts.join(".");
 
        assert!(verify_token(&tampered, SECRET).is_err());
    }
 
    /// Authorization ヘッダーから取り出せる
    #[test]
    fn extract_user_id_reads_the_bearer_header() {
        let user_id = Uuid::new_v4();
        let token = issue_token(user_id, SECRET).unwrap();
 
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
 
        assert_eq!(extract_user_id(&headers, SECRET).unwrap(), user_id);
    }
 
    /// Bearer 形式でないヘッダーは拒否される
    #[test]
    fn extract_user_id_rejects_a_malformed_header() {
        let token = issue_token(Uuid::new_v4(), SECRET).unwrap();
 
        let mut headers = HeaderMap::new();
        // Bearer プレフィックスなし
        headers.insert(
            axum::http::header::AUTHORIZATION,
            token.parse().unwrap(),
        );
 
        assert!(extract_user_id(&headers, SECRET).is_err());
    }
 
    /// ヘッダーが無ければ拒否される
    #[test]
    fn extract_user_id_rejects_a_missing_header() {
        let headers = HeaderMap::new();
 
        assert!(extract_user_id(&headers, SECRET).is_err());
    }
}
 