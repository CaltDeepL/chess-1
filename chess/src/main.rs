use std::collections::HashMap;
use std::sync::Arc;

use chess_server::{build_router, state::AppState};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    let app = build_router(state);

    // Renderなどのホスティング先はリッスンすべきポートをPORT環境変数で渡してくる。
    // ローカル開発では未設定なので、その場合は従来通り3000番にフォールバックする。
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
