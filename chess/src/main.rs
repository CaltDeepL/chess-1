use std::{collections::HashMap, sync::Arc};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shakmaty::{fen::Fen, uci::UciMove, Chess, EnPassantMode, Position};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

/// アプリ全体で共有する状態。
/// - `games`: 対局ID -> 現在の盤面(shakmatyのChess型)。RwLockで包むことで、
///   複数リクエストから同時に読み書きできるようにする(対局部分は今後DB移行予定)。
/// - `db`: PostgreSQLへの接続プール(ユーザー認証まわりで使用)
/// - `jwt_secret`: JWTの署名・検証に使う鍵
#[derive(Clone)]
struct AppState {
    games: Arc<RwLock<HashMap<Uuid, Chess>>>,
    db: PgPool,
    jwt_secret: Arc<String>,
}

/// JWTのペイロード(クレーム)。有効期限(exp)はjsonwebtokenの規約上必須。
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: Uuid, // ユーザーID
    exp: usize,
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    user_id: Uuid,
    token: String,
}

/// usersテーブルの行に対応する型
#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    password_hash: String,
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
    // .envからDATABASE_URLなどを読み込む(存在しなくてもエラーにしない)
    dotenvy::dotenv().ok();

    // ログ設定
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

/// POST /auth/register
/// ユーザー名とパスワードを受け取り、Argon2でハッシュ化してDBに保存する。
async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    if payload.username.trim().is_empty() || payload.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            "ユーザー名は必須、パスワードは8文字以上にしてください".to_string(),
        ));
    }

    // パスワードをArgon2でハッシュ化(ランダムなソルトを都度生成)
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("パスワードのハッシュ化に失敗しました: {}", e),
            )
        })?
        .to_string();

    let user_id = Uuid::new_v4();

    let result = sqlx::query("INSERT INTO users (id, username, password_hash) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&payload.username)
        .bind(&password_hash)
        .execute(&state.db)
        .await;

    if let Err(e) = result {
        // ユーザー名重複などはDB側のUNIQUE制約違反として返ってくる
        tracing::warn!(error = %e, "register failed");
        return Err((
            StatusCode::CONFLICT,
            "そのユーザー名は既に使われています".to_string(),
        ));
    }

    let token = issue_token(user_id, &state.jwt_secret)?;

    tracing::info!(%user_id, username = %payload.username, "user registered");

    Ok(Json(AuthResponse { user_id, token }))
}

/// POST /auth/login
/// ユーザー名とパスワードを照合し、正しければJWTを発行する。
async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let user = sqlx::query_as::<_, UserRow>("SELECT id, password_hash FROM users WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DBエラー: {}", e),
            )
        })?
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "ユーザー名またはパスワードが違います".to_string(),
        ))?;

    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("保存済みハッシュの読み取りに失敗しました: {}", e),
        )
    })?;

    // パスワード照合。ここで一致しなければ401を返す(ユーザー不在の場合と同じメッセージにして
    // 「ユーザー名が存在するかどうか」が外部から推測できないようにしている)
    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "ユーザー名またはパスワードが違います".to_string(),
            )
        })?;

    let token = issue_token(user.id, &state.jwt_secret)?;

    tracing::info!(user_id = %user.id, "user logged in");

    Ok(Json(AuthResponse {
        user_id: user.id,
        token,
    }))
}

/// 指定ユーザーIDに対するJWTを発行するヘルパー。有効期限は24時間。
fn issue_token(user_id: Uuid, jwt_secret: &str) -> Result<String, (StatusCode, String)> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("有効なタイムスタンプの計算に失敗しました")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("トークン発行に失敗しました: {}", e),
        )
    })
}

/// JWTを検証してユーザーIDを取り出すヘルパー。
/// 次のステップ(moveエンドポイントへの認証組み込み、WebSocket認証)で使う予定。
#[allow(dead_code)]
fn verify_token(token: &str, jwt_secret: &str) -> Result<Uuid, (StatusCode, String)> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims.sub)
    .map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            format!("トークンが無効です: {}", e),
        )
    })
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