use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shakmaty::{fen::Fen, uci::UciMove, Chess, EnPassantMode, Position};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

/// アプリ全体で共有する状態。
/// - `games`: 対局ID -> 現在の盤面(shakmatyのChess型)
/// RwLockで包むことで、複数リクエストから同時に読み書きできるようにする。
#[derive(Clone)]
struct AppState {
    games: Arc<RwLock<HashMap<Uuid, Chess>>>,
}

#[derive(Deserialize)]
struct MoveRequest {
    /// UCI形式の指し手(例: "e2e4", プロモーションは "e7e8q")
    uci: String,
}

#[derive(Serialize)]
struct GameCreatedResponse {
    game_id: Uuid,
    fen: String,
}

#[derive(Serialize)]
struct GameStateResponse {
    game_id: Uuid,
    fen: String,
    is_check: bool,
    is_game_over: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ログ設定
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "chess_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = AppState {
        games: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/games", post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/move", post(make_move))
        .layer(CorsLayer::permissive()) // 開発中は緩め。本番では絞る
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// POST /games
/// 新規対局を作成し、初期盤面(標準チェスの開始局面)を返す。
async fn create_game(State(state): State<AppState>) -> Json<GameCreatedResponse> {
    let game_id = Uuid::new_v4();
    let position = Chess::default(); // 標準の初期局面

    let fen = position_to_fen(&position);

    // 書き込みロックを取得して対局を登録
    state.games.write().await.insert(game_id, position);

    tracing::info!(%game_id, "new game created");

    Json(GameCreatedResponse { game_id, fen })
}

/// GET /games/:id
/// 指定した対局IDの現在の盤面情報を返す。
async fn get_game(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GameStateResponse>, (StatusCode, String)> {
    let games = state.games.read().await;

    let position = games
        .get(&id)
        .ok_or((StatusCode::NOT_FOUND, "対局が見つかりません".to_string()))?;

    Ok(Json(GameStateResponse {
        game_id: id,
        fen: position_to_fen(position),
        is_check: position.is_check(),
        is_game_over: position.is_game_over(),
    }))
}

/// POST /games/:id/move
/// UCI形式の指し手を受け取り、合法手であれば盤面を更新して返す。
/// 不正な指し手や存在しない対局IDは400/404で理由を返す。
async fn make_move(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<MoveRequest>,
) -> Result<Json<GameStateResponse>, (StatusCode, String)> {
    // 1. UCI文字列をパース(例: "e2e4" が正しい形式かどうか)
    let uci_move: UciMove = payload
        .uci
        .parse()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("指し手の形式が不正です: {}", e)))?;

    let mut games = state.games.write().await;

    // 2. 対局を取得(書き込み用の可変参照)
    let position = games
        .get_mut(&id)
        .ok_or((StatusCode::NOT_FOUND, "対局が見つかりません".to_string()))?;

    // 3. UCIの指し手を現在の局面における具体的な指し手(Move)に変換
    //    ここで「その局面で本当に指せる手か」も含めて検証される
    let mv = uci_move
        .to_move(position)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("不正な指し手です: {}", e)))?;

    // 4. 指し手を適用して局面を更新(playは合法手のみ受け付ける)
    match position.clone().play(&mv) {
        Ok(new_position) => {
            *position = new_position;

            tracing::info!(%id, uci = %payload.uci, "move applied");

            Ok(Json(GameStateResponse {
                game_id: id,
                fen: position_to_fen(position),
                is_check: position.is_check(),
                is_game_over: position.is_game_over(),
            }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            format!("指し手を適用できません: {}", e),
        )),
    }
}

/// shakmatyのChess局面をFEN文字列に変換するヘルパー
fn position_to_fen(position: &Chess) -> String {
    Fen::from_position(position.clone(), EnPassantMode::Legal).to_string()
}