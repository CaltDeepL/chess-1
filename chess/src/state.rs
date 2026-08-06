use std::{collections::HashMap, sync::Arc};

use shakmaty::Chess;
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

/// アプリ全体で共有する状態。
/// - `games`: 対局ID -> 現在の盤面(shakmatyのChess型)。RwLockで包むことで、
///   複数リクエストから同時に読み書きできるようにする(対局部分は今後DB移行予定)。
/// - `db`: PostgreSQLへの接続プール(ユーザー認証まわりで使用)
/// - `jwt_secret`: JWTの署名・検証に使う鍵
#[derive(Clone)]
pub struct AppState {
    pub games: Arc<RwLock<HashMap<Uuid, Chess>>>,
    pub db: PgPool,
    pub jwt_secret: Arc<String>,
}
