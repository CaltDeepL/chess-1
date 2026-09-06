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
/// - `game_connections`: (対局ID, ユーザーID) -> 接続本数(切断による対局放棄の判定に使う)
/// - `sweep_token`: /internal/sweep の共有シークレット
#[derive(Clone)]
pub struct AppState {
    pub games: Arc<RwLock<HashMap<Uuid, Chess>>>,
    pub db: PgPool,
    pub jwt_secret: Arc<String>,
    pub game_channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<GameEvent>>>>,
    /// (game_id, user_id) ごとの WebSocket 接続数。
    /// 同じユーザーが複数タブを開いている場合に、1枚閉じただけで
    /// 切断扱いにしないために数える。
    pub game_connections: Arc<RwLock<HashMap<(Uuid, Uuid), usize>>>,
    pub sweep_token: String,
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
    OpponentJoined {
        user_id: Uuid,
    },
    PlayerDisconnected {
        user_id: Uuid,
        /// 切断が確定するまでの残り秒数。
        /// 時刻ではなく残り秒数にすることで、クライアントとサーバーの
        /// 時計のずれがカウントダウンに影響しない
        remaining_seconds: i64,
    },
    PlayerReconnected {
        user_id: Uuid,
    },
}
