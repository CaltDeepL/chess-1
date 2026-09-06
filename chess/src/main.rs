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

    let sweep_token = std::env::var("SWEEP_TOKEN").unwrap_or_default();
    if sweep_token.is_empty() {
        tracing::warn!("SWEEP_TOKEN未設定のため /internal/sweep は使用できません");
    }

    let db = PgPool::connect(&database_url).await?;

    // ローカルDBへの適用漏れで column does not exist が起きた。
    // sqlx::test は毎回専用DBに全マイグレーションを当てるため、テストでは
    // 気づけない。起動時に適用すれば、ローカルも本番も漏れが構造的に無くなる。
    //
    // Render の無料枠は単一インスタンスなので、複数プロセスが同時に
    // マイグレーションを走らせる心配はない。将来スケールさせるなら
    // advisory lock で囲む必要がある。
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("マイグレーションの適用に失敗しました");

    tracing::info!("migrations applied");

    let state = AppState {
        games: Arc::new(RwLock::new(HashMap::new())),
        db,
        jwt_secret: Arc::new(jwt_secret),
        game_channels: Arc::new(RwLock::new(HashMap::new())),
        game_connections: Arc::new(RwLock::new(HashMap::new())),
        sweep_token,
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
