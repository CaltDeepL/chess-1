use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// JWTのペイロード(クレーム)。有効期限(exp)はjsonwebtokenの規約上必須。
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Claims {
    pub sub: Uuid, // ユーザーID
    pub exp: usize,
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub token: String,
}

/// usersテーブルの行に対応する型
#[derive(sqlx::FromRow, ToSchema)]
pub struct UserRow {
    pub id: Uuid,
    pub password_hash: String,
}

/// GET /users/:id のレスポンス(公開してよい情報のみ)
#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct UserPublicResponse {
    pub id: Uuid,
    pub username: String,
}

/// gamesテーブルの行に対応する型(join_game/make_moveで使用)
#[derive(sqlx::FromRow)]
pub struct GameRow {
    pub white_user_id: Uuid,
    pub black_user_id: Option<Uuid>,
    #[allow(dead_code)]
    pub status: String,
}

#[derive(Deserialize, ToSchema)]
pub struct MoveRequest {
    /// UCI形式の指し手(例: "e2e4", プロモーションは "e7e8q")
    pub uci: String,
}

#[derive(Serialize, ToSchema)]
pub struct GameCreatedResponse {
    pub game_id: Uuid,
    pub fen: String,
}

#[derive(Serialize, ToSchema)]
pub struct GameStateResponse {
    pub game_id: Uuid,
    pub fen: String,
    pub is_check: bool,
    pub is_game_over: bool,
}

/// GET /games/:id でDBから取得する参加者・状態情報
#[derive(sqlx::FromRow, ToSchema)]
pub struct GameDetailRow {
    pub white_user_id: Uuid,
    pub black_user_id: Option<Uuid>,
    pub status: String,
    pub result: Option<String>,
}

/// GET /games/:id のレスポンス
#[derive(Serialize, ToSchema)]
pub struct GameDetailResponse {
    pub game_id: Uuid,
    pub white_user_id: Uuid,
    pub black_user_id: Option<Uuid>,
    pub status: String,
    pub result: Option<String>,
    pub fen: String,
    pub is_check: bool,
    pub is_game_over: bool,
}

/// GET /games のクエリパラメータ
#[derive(Deserialize, IntoParams)]
pub struct ListGamesQuery {
    /// 指定時はこのstatusの対局のみに絞り込む(例: "waiting")。未指定なら全件
    pub status: Option<String>,
}

/// GET /games のレスポンス1件分
#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct GameSummary {
    pub id: Uuid,
    pub white_user_id: Uuid,
    pub black_user_id: Option<Uuid>,
    pub status: String,
    pub fen: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct MoveRow {
    pub move_number: i32,
    pub uci: String,
    pub fen_after: String,
}
