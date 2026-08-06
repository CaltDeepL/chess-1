use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use shakmaty::Chess;
use sqlx::PgPool;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// アプリ全体で共有する状態。
/// - `games`: 対局ID -> 現在の盤面(shakmatyのChess型)。RwLockで包むことで、
///   複数リクエストから同時に読み書きできるようにする(対局部分は今後DB移行予定)。
/// - `db`: PostgreSQLへの接続プール(ユーザー認証まわりで使用)
/// - `jwt_secret`: JWTの署名・検証に使う鍵
/// - `game_channels`: 対局ID -> WebSocketブロードキャスト用チャンネル(指し手/投了/終局を配信)
#[derive(Clone)]
pub struct AppState {
    pub games: Arc<RwLock<HashMap<Uuid, Chess>>>,
    pub db: PgPool,
    pub jwt_secret: Arc<String>,
    pub game_channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<GameEvent>>>>,
}

impl AppState {
    /// 指定した対局のブロードキャストチャンネルを取得する(無ければ新規作成)。
    pub async fn game_channel(&self, game_id: Uuid) -> broadcast::Sender<GameEvent> {
        let mut channels = self.game_channels.write().await;
        channels
            .entry(game_id)
            .or_insert_with(|| broadcast::channel(16).0)
            .clone()
    }
}

/// WebSocket経由でクライアントへ配信する対局イベント。
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    Move {
        fen: String,
        uci: String,
        is_check: bool,
        is_game_over: bool,
    },
    GameOver {
        result: String,
        end_reason: String,
    },
}
