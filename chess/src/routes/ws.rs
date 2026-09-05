use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::verify_token;
use crate::models::GameRow;
use crate::state::AppState;

/// 認証メッセージを待つ最大時間。
/// 接続だけして何も送らないクライアントがタスクを保持し続けるのを防ぐ。
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

/// クライアントが接続直後に送る認証メッセージ
#[derive(Deserialize)]
struct AuthMessage {
    token: String,
}

/// GET /ws/games/:id
/// WebSocket接続のエントリーポイント。実際の処理はhandle_socketに委譲する。
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, id))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, game_id: Uuid) {
    // 1. 最初のメッセージとしてトークンを受け取る。
    // タイムアウトが無いと、接続だけして何も送らないクライアントが
    // recv_task/send_task を起動する前段でタスクを永久に保持し続けてしまう。
    let auth_msg = match tokio::time::timeout(AUTH_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(_) => {
            let _ = socket
                .send(Message::Text(
                    r#"{"error":"認証メッセージが必要です"}"#.into(),
                ))
                .await;
            return;
        }
        Err(_) => {
            let _ = socket
                .send(Message::Text(
                    r#"{"error":"認証メッセージがタイムアウトしました"}"#.into(),
                ))
                .await;
            return;
        }
    };

    let token = match serde_json::from_str::<AuthMessage>(&auth_msg) {
        Ok(msg) => msg.token,
        Err(_) => {
            let _ = socket
                .send(Message::Text(
                    r#"{"error":"認証メッセージの形式が不正です"}"#.into(),
                ))
                .await;
            return;
        }
    };

    let user_id = match verify_token(&token, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(_) => {
            let _ = socket
                .send(Message::Text(r#"{"error":"トークンが無効です"}"#.into()))
                .await;
            return;
        }
    };

    // 2. 対局の参加者かどうかを確認
    let game = match sqlx::query_as::<_, GameRow>(
        "SELECT white_user_id, black_user_id, status::text AS status FROM games WHERE id = $1",
    )
    .bind(game_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(g)) => g,
        _ => {
            let _ = socket
                .send(Message::Text(r#"{"error":"対局が見つかりません"}"#.into()))
                .await;
            return;
        }
    };

    if user_id != game.white_user_id && Some(user_id) != game.black_user_id {
        let _ = socket
            .send(Message::Text(
                r#"{"error":"この対局の参加者ではありません"}"#.into(),
            ))
            .await;
        return;
    }

    // 3. この対局用のブロードキャストチャンネルを取得(無ければ作成)
    let mut receiver = state.game_channel(game_id).await.subscribe();

    tracing::info!(%game_id, %user_id, "websocket connected");

    // 購読が実際に始まったことをクライアントに知らせる。
    // クライアント側はこれを「接続完了」の合図として使える
    // (upgrade成功時点ではまだ認証・参加者チェック・購読が済んでいないため、
    // それより後のこのタイミングの方が実態に即している)。
    if socket
        .send(Message::Text(r#"{"type":"connected"}"#.into()))
        .await
        .is_err()
    {
        return;
    }

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // 4. 受信タスク: クライアントからの切断検知のみ(今回はメッセージ送信は不要)
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            if msg.is_err() {
                break;
            }
        }
    });

    // 5. 送信タスク: ブロードキャストされたイベントをクライアントへ配信
    let send_task = tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            let json = serde_json::to_string(&event).unwrap_or_default();
            if ws_sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // どちらかのタスクが終了したら接続を閉じる
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }

    tracing::info!(%game_id, %user_id, "websocket disconnected");
}
