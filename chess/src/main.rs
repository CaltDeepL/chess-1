use std::{collections::HashMap, sync::Arc};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shakmaty::{fen::Fen, uci::UciMove, Chess, Color, EnPassantMode, Position};
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

/// gamesテーブルの行に対応する型(join_gameで使用)
#[derive(sqlx::FromRow)]
struct GameRow {
    white_user_id: Uuid,
    black_user_id: Option<Uuid>,
    #[allow(dead_code)]
    status: String,
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
let db = sqlx::postgres::PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

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
        .route("/games/:id/join", post(join_game))
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

/// Authorizationヘッダー(Bearer方式)からユーザーIDを取り出すヘルパー
fn extract_user_id(headers: &HeaderMap, jwt_secret: &str) -> Result<Uuid, (StatusCode, String)> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "認証トークンがありません".to_string(),
        ))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or((
        StatusCode::UNAUTHORIZED,
        "Authorizationヘッダーの形式が不正です(Bearer <token>の形式で送ってください)".to_string(),
    ))?;

    verify_token(token, jwt_secret)
}

/// POST /games
/// 新規対局を作成し、初期盤面(標準チェスの開始局面)を返す。
/// 呼び出しにはJWT認証が必要(Authorization: Bearer <token>)。
async fn create_game(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GameCreatedResponse>, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let game_id = Uuid::new_v4();
    let position = Chess::default(); // 標準の初期局面
    let fen = position_to_fen(&position);

    sqlx::query("INSERT INTO games (id, white_user_id, fen) VALUES ($1, $2, $3)")
        .bind(game_id)
        .bind(user_id)
        .bind(&fen)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to insert game");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "対局の作成に失敗しました".to_string(),
            )
        })?;

    // 書き込みロックを取得して対局を登録(進行中の対局はメモリ上で管理)
    state.games.write().await.insert(game_id, position);

    tracing::info!(%game_id, %user_id, "new game created");

    Ok(Json(GameCreatedResponse { game_id, fen }))
}

/// POST /games/:id/join
/// 対局に対戦相手(黒番)として参加する。
/// 既に対戦相手がいる場合や、自分の対局には参加できない。
async fn join_game(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    // 対局の現在の状態を取得
    let game = sqlx::query_as::<_, GameRow>(
        "SELECT white_user_id, black_user_id, status::text FROM games WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DBエラー: {}", e),
        )
    })?
    .ok_or((StatusCode::NOT_FOUND, "対局が見つかりません".to_string()))?;

    if game.white_user_id == user_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "自分が作成した対局には参加できません".to_string(),
        ));
    }

    if game.black_user_id.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "この対局には既に対戦相手がいます".to_string(),
        ));
    }

    // black_user_idがNULLの場合のみ更新(同時参加のレースコンディション対策)
    let result = sqlx::query(
        "UPDATE games SET black_user_id = $1, status = 'in_progress', updated_at = now() \
         WHERE id = $2 AND black_user_id IS NULL",
    )
    .bind(user_id)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DBエラー: {}", e),
        )
    })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::CONFLICT,
            "この対局には既に対戦相手がいます".to_string(),
        ));
    }

    tracing::info!(%id, %user_id, "player joined game");

    Ok(Json(json!({ "game_id": id, "status": "in_progress" })))
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
/// 呼び出しにはJWT認証が必要で、対局の参加者本人かつ手番が合っている場合のみ受け付ける。
async fn make_move(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<MoveRequest>,
) -> Result<Json<GameStateResponse>, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    // 対局の参加者情報をDBから取得(参加者チェック・手番チェックに使う)
    let game = sqlx::query_as::<_, GameRow>(
        "SELECT white_user_id, black_user_id, status::text AS status FROM games WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DBエラー: {}", e),
        )
    })?
    .ok_or((StatusCode::NOT_FOUND, "対局が見つかりません".to_string()))?;

    // 参加者本人かどうかのチェック(白番・黒番どちらでもない第三者は弾く)
    if user_id != game.white_user_id && Some(user_id) != game.black_user_id {
        return Err((
            StatusCode::FORBIDDEN,
            "この対局の参加者ではありません".to_string(),
        ));
    }

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

    // 手番チェック: 現在の局面の手番と、リクエストしたユーザーが一致するか
    let expected_user = match position.turn() {
        Color::White => game.white_user_id,
        Color::Black => game.black_user_id.ok_or((
            StatusCode::CONFLICT,
            "対戦相手がまだ参加していません".to_string(),
        ))?,
    };
    if user_id != expected_user {
        return Err((
            StatusCode::FORBIDDEN,
            "あなたの手番ではありません".to_string(),
        ));
    }

    // 3. UCIの指し手を現在の局面における具体的な指し手(Move)に変換
    let mv = uci_move
        .to_move(position)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("不正な指し手です: {}", e)))?;

    // 4. 指し手を適用して局面を更新(playは合法手のみ受け付ける)
    match position.clone().play(&mv) {
        Ok(new_position) => {
            *position = new_position;

            let fen_after = position_to_fen(position);
            let move_number = position.fullmoves().get() as i32;

            // 指し手を moves テーブルへ記録(棋譜として毎手保存)
            if let Err(e) = sqlx::query(
                "INSERT INTO moves (game_id, move_number, uci, fen_after) VALUES ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(move_number)
            .bind(&payload.uci)
            .bind(&fen_after)
            .execute(&state.db)
            .await
            {
                // 棋譜保存に失敗しても対局自体は続行できるようログのみ残す
                tracing::error!(error = %e, %id, "failed to insert move");
            }

            // 対局が終了していたら games テーブルを更新
            if position.is_game_over() {
                let (result, end_reason) = determine_outcome(position);

                if let Err(e) = sqlx::query(
                    "UPDATE games SET status = 'finished', result = $1, end_reason = $2, updated_at = now() \
                     WHERE id = $3",
                )
                .bind(result)
                .bind(end_reason)
                .bind(id)
                .execute(&state.db)
                .await
                {
                    tracing::error!(error = %e, %id, "failed to update game result");
                }

                tracing::info!(%id, result, end_reason, "game finished");
            }

            tracing::info!(%id, %user_id, uci = %payload.uci, "move applied");

            Ok(Json(GameStateResponse {
                game_id: id,
                fen: fen_after,
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

/// 終局した局面から (result, end_reason) を判定するヘルパー
fn determine_outcome(position: &Chess) -> (&'static str, &'static str) {
    if position.is_checkmate() {
        let winner = match position.turn() {
            Color::White => "black_win", // 手番側が詰まされている = 相手の勝ち
            Color::Black => "white_win",
        };
        (winner, "checkmate")
    } else if position.is_stalemate() {
        ("draw", "stalemate")
    } else if position.is_insufficient_material() {
        ("draw", "insufficient_material")
    } else {
        ("draw", "other")
    }
}

/// shakmatyのChess局面をFEN文字列に変換するヘルパー
fn position_to_fen(position: &Chess) -> String {
    Fen::from_position(position.clone(), EnPassantMode::Legal).to_string()
}