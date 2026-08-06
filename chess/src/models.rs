use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWTのペイロード(クレーム)。有効期限(exp)はjsonwebtokenの規約上必須。
#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid, // ユーザーID
    pub exp: usize,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub token: String,
}

/// usersテーブルの行に対応する型
#[derive(sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub password_hash: String,
}

/// gamesテーブルの行に対応する型(join_game/make_moveで使用)
#[derive(sqlx::FromRow)]
pub struct GameRow {
    pub white_user_id: Uuid,
    pub black_user_id: Option<Uuid>,
    #[allow(dead_code)]
    pub status: String,
}

#[derive(Deserialize)]
pub struct MoveRequest {
    /// UCI形式の指し手(例: "e2e4", プロモーションは "e7e8q")
    pub uci: String,
}

#[derive(Serialize)]
pub struct GameCreatedResponse {
    pub game_id: Uuid,
    pub fen: String,
}

#[derive(Serialize)]
pub struct GameStateResponse {
    pub game_id: Uuid,
    pub fen: String,
    pub is_check: bool,
    pub is_game_over: bool,
}
