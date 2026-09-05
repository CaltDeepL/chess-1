mod auth;
mod domain;
mod errors;
mod models;
mod routes;
pub mod state;


use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use auth::{login, register};
use axum::http::{header, Method};
use routes::game::{
    create_game, get_game, get_moves, join_game, list_games, make_move, resign_game,
};
use routes::health::health_check;
use routes::user::get_user;
use routes::ws::ws_handler;
use state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/games", get(list_games).post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/join", post(join_game))
        .route("/games/:id/move", post(make_move))
        .route("/games/:id/resign", post(resign_game))
        .route("/ws/games/:id", get(ws_handler))
        .route("/users/:id", get(get_user))
        .route("/games/:id/moves", get(get_moves))
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// 本番のフロントエンドのオリジンはFRONTEND_ORIGIN環境変数で指定する(カンマ区切りで複数可)。
// 例: FRONTEND_ORIGIN=https://chess-frontend.onrender.com,https://chess.example.com
// 未設定時はローカル開発用のVite開発サーバーのポートをフォールバックとして許可しておく
// (5173が既に使用中だと5174、5175...とViteが自動でずれていくため複数残してある)。
const DEFAULT_DEV_ORIGINS: &str =
    "http://localhost:5173,http://localhost:5174,http://localhost:5175";

pub fn build_cors_layer() -> CorsLayer {
    let allowed_origins: Vec<axum::http::HeaderValue> = std::env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| DEFAULT_DEV_ORIGINS.to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
}
