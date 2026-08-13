use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// 各ハンドラで共通のエラー型。バリアントごとにHTTPステータスが決まり、
/// IntoResponseでJSONボディ({"message": "..."})に変換される。
/// これによりフロントエンド(api/client.ts)がres.json()でmessageを
/// 正しく取り出せるようになる(以前はtext/plainで返っておりJSONパースに
/// 失敗し、汎用フォールバック文言にすり替わってしまっていた)。
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(ErrorBody { message })).into_response()
    }
}

/// sqlx::Errorから`?`一発でAppError::Internalへ変換できるようにする。
/// これにより各クエリ末尾の `.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DBエラー: {}", e)))?`
/// という重複が丸ごと不要になる。
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Internal(format!("DBエラー: {}", e))
    }
}
