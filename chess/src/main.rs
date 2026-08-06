mod auth;
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
use routes::game::{create_game, get_game, join_game, make_move, resign_game};
use routes::health::health_check;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "chess_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("環境変数 DATABASE_URL が設定されていません(.envを確認してください)");
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!("JWT_SECRET未設定のため開発用の固定値を使用します(本番では必ず設定してください)");
        "dev-secret-change-me".to_string()
    });

    let db = PgPool::connect(&database_url).await?;

    let state = AppState {
        games: Arc::new(RwLock::new(HashMap::new())),
        db,
        jwt_secret: Arc::new(jwt_secret),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/games", post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/join", post(join_game))
        .route("/games/:id/move", post(make_move))
        .route("/games/:id/resign", post(resign_game))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}