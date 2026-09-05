use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

/// RFC 9457 Problem Details for HTTP APIs
#[derive(Serialize, ToSchema)]
pub struct ProblemDetails {
    /// 問題の種類を識別するURI参照
    #[serde(rename = "type")]
    #[schema(example = "/problems/forbidden")]
    pub type_uri: String,
    /// 種類の短い説明(この種類に対して常に同じ)
    #[schema(example = "Forbidden")]
    pub title: String,
    /// HTTPステータスコード
    #[schema(example = 403)]
    pub status: u16,
    /// この発生事例の説明(利用者に見せる文言)
    #[schema(example = "あなたの手番ではありません")]
    pub detail: String,
}

impl AppError {
    /// 変種ごとの (ステータス, typeのコード, title)
    fn meta(&self) -> (StatusCode, &'static str, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad-request", "Bad request"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized", "Unauthorized"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden", "Forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not-found", "Not found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict", "Conflict"),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal server error",
            ),
        }
    }

    /// クライアントに見せる detail
    ///
    /// Internal だけは内部情報(DBのエラー文など)を含みうるため、
    /// 中身を出さず固定文言に差し替える。原因はログにのみ残す。
    fn public_detail(&self) -> String {
        match self {
            Self::Internal(_) => "サーバー内部でエラーが発生しました".to_string(),
            Self::BadRequest(m)
            | Self::Unauthorized(m)
            | Self::Forbidden(m)
            | Self::NotFound(m)
            | Self::Conflict(m) => m.clone(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let Self::Internal(m) = &self {
            tracing::error!(detail = %m, "internal server error");
        }

        let (status, code, title) = self.meta();
        let body = ProblemDetails {
            type_uri: format!("/problems/{code}"),
            title: title.to_string(),
            status: status.as_u16(),
            detail: self.public_detail(),
        };

        // Json が付ける application/json を、後段のヘッダで上書きする
        (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(body),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(format!("DBエラー: {e}"))
    }
}
