mod auth;
mod errors;
mod models;
mod routes;
mod state;

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use auth::{login, register};
use axum::http::{header, Method};
use routes::game::{
    create_game, get_game, get_moves, join_game, list_games, make_move, resign_game,
};
use routes::health::health_check;
use routes::user::get_user;
use routes::ws::ws_handler;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "chess_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("環境変数 DATABASE_URL が設定されていません(.envを確認してください)");
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!(
            "JWT_SECRET未設定のため開発用の固定値を使用します(本番では必ず設定してください)"
        );
        "dev-secret-change-me".to_string()
    });

    let db = PgPool::connect(&database_url).await?;

    let state = AppState {
        games: Arc::new(RwLock::new(HashMap::new())),
        db,
        jwt_secret: Arc::new(jwt_secret),
        game_channels: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
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
        .with_state(state);

    // Renderなどのホスティング先はリッスンすべきポートをPORT環境変数で渡してくる。
    // ローカル開発では未設定なので、その場合は従来通り3000番にフォールバックする。
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

// 本番のフロントエンドのオリジンはFRONTEND_ORIGIN環境変数で指定する(カンマ区切りで複数可)。
// 例: FRONTEND_ORIGIN=https://chess-frontend.onrender.com,https://chess.example.com
// 未設定時はローカル開発用のVite開発サーバーのポートをフォールバックとして許可しておく
// (5173が既に使用中だと5174、5175...とViteが自動でずれていくため複数残してある)。
const DEFAULT_DEV_ORIGINS: &str =
    "http://localhost:5173,http://localhost:5174,http://localhost:5175";

fn build_cors_layer() -> CorsLayer {
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
