#![allow(dead_code)]

use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use chess_server::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// DBやHTTPを経由せず、生成されたOpenAPI仕様を直接取得する
/// (ProblemDetailsの参照漏れチェックなど、純粋に静的なテストで使う)
pub fn openapi_json() -> Value {
    serde_json::to_value(chess_server::openapi_spec()).unwrap()
}

/// テスト全体で共有する AppState を作る
pub fn test_state(pool: PgPool) -> AppState {
    AppState {
        games: Default::default(),
        db: pool,
        jwt_secret: std::sync::Arc::new("test-secret".to_string()),
        game_channels: Default::default(),
    }
}

async fn send(state: &AppState, req: Request<Body>) -> (StatusCode, Value) {
    // state は Clone で内部の Arc を共有するため、メモリ上の対局が引き継がれる
    let response = chess_server::build_router(state.clone())
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

pub async fn post_json(state: &AppState, path: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(state, req).await
}

/// post_json のヘッダ付き版。Content-Type を検証したいときに使う。
pub async fn post_json_raw(
    state: &AppState,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let response = chess_server::build_router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);

    (status, headers, json)
}

pub async fn post_auth(state: &AppState, path: &str, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    send(state, req).await
}

pub async fn post_auth_json(
    state: &AppState,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(state, req).await
}

/// 認証付き GET。クエリ文字列を含むパスをそのまま渡せる。
pub async fn get_auth(
    state: &AppState,
    path: &str,
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let response = chess_server::build_router(state.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);

    (status, json)
}

/// 認証なしのGET(JSONレスポンス)
pub async fn get_json(state: &AppState, path: &str) -> (StatusCode, serde_json::Value) {
    let response = chess_server::build_router(state.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);

    (status, json)
}

/// HTMLを返すエンドポイント用(ボディはパースせずステータスのみ確認)
pub async fn get_html(state: &AppState, path: &str) -> (StatusCode, String) {
    let response = chess_server::build_router(state.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

pub async fn register_user(state: &AppState, username: &str) -> String {
    let (status, body) = post_json(
        state,
        "/auth/register",
        json!({ "username": username, "password": "password123" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register failed: {body}");
    body["token"].as_str().unwrap().to_string()
}

pub async fn create_game(state: &AppState, token: &str) -> String {
    let (status, body) = post_auth(state, "/games", token).await;
    assert_eq!(status, StatusCode::OK, "create_game failed: {body}");
    body["game_id"].as_str().unwrap().to_string()
}

pub async fn make_move(
    state: &AppState,
    game_id: &str,
    token: &str,
    uci: &str,
) -> (StatusCode, Value) {
    post_auth_json(
        state,
        &format!("/games/{game_id}/move"),
        token,
        json!({ "uci": uci }),
    )
    .await
}

/// テストサーバーに接続した WebSocket ストリーム
pub type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// テスト用サーバーを空きポートで起動し、待ち受けアドレスを返す。
///
/// WebSocket は `101 Switching Protocols` を伴う双方向通信のため、
/// 既存の `oneshot` ヘルパー(HTTP 1往復の模擬)では検証できず、
/// 実際に TCP を listen する必要がある。
///
/// REST 側は従来どおり `post_json` などの oneshot ヘルパーを使えばよい。
/// 同じ `state` を共有しており `AppState` のフィールドは `Arc` で
/// ラップされているため、oneshot 経由で作った対局も
/// このサーバーの WS ハンドラから見える(task-27 参照)。
pub async fn spawn_server(state: &AppState) -> SocketAddr {
    // ポート0を指定するとOSが空きポートを割り当てる。
    // 固定ポートだとテストの並列実行や `cargo run` 中のサーバーと衝突する。
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = chess_server::build_router(state.clone());

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    addr
}

/// 認証を行わずに WebSocket を開くだけのヘルパー。
///
/// このアプリの WS は upgrade 自体は常に成功し、認証は upgrade 後の
/// 最初の Text メッセージ `{"token":"..."}` で行う。認証失敗の検証には
/// 「接続はできるがエラーメッセージが返る」ことを見る必要があるため、
/// 認証前の状態を取り出せるようにしている。
pub async fn open_ws(addr: SocketAddr, game_id: &str) -> WsStream {
    let url = format!("ws://{addr}/ws/games/{game_id}");
    let (ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("WebSocket の upgrade に失敗した");
    ws
}

/// 認証メッセージを送る
pub async fn send_auth(ws: &mut WsStream, token: &str) {
    let msg = serde_json::json!({ "token": token }).to_string();
    ws.send(Message::Text(msg))
        .await
        .expect("認証メッセージの送信に失敗した");
}

/// 接続して認証まで済ませ、イベントを受け取れる状態のストリームを返す。
///
/// サーバーは認証・参加者チェック・DB照会・`subscribe()` を終えた直後に
/// `{"type":"connected"}` を送ってくるので、それを読み切ってから返す。
/// これにより「購読が始まる前にRESTを叩いてイベントを取りこぼす」という
/// レースコンディションが、固定時間のsleepに頼らずに解消される。
pub async fn connect_ws(addr: SocketAddr, game_id: &str, token: &str) -> WsStream {
    let mut ws = open_ws(addr, game_id).await;
    send_auth(&mut ws, token).await;
    let event = next_event(&mut ws).await;
    assert_eq!(
        event["type"], "connected",
        "購読開始の通知が届くはず: {event}"
    );
    ws
}

/// WebSocket から次のフレームを1件受け取り、JSON として返す。
///
/// タイムアウトを入れないと、イベントが配信されない不具合のときに
/// 永久に待ち続け、CI のジョブ枠を占有する。
pub async fn next_event(ws: &mut WsStream) -> serde_json::Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("2秒以内にイベントが届かなかった")
            .expect("ストリームが閉じられている")
            .expect("WebSocket のエラー");

        match msg {
            // 制御フレームは JSON ではないので読み飛ばす
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Text(text) => {
                return serde_json::from_str(&text).expect("イベントがJSONではない")
            }
            Message::Close(frame) => panic!("接続が閉じられた: {frame:?}"),
            other => panic!("想定外のフレーム: {other:?}"),
        }
    }
}

/// 一定時間イベントが届かないことを確認する(配信されないはずの相手の検証用)
///
/// タイムアウトだけでなく、接続が閉じられた場合も「イベントは届いていない」
/// として許容する。認証・参加者チェックに失敗した接続はハンドラが
/// WebSocketをそのままdropして終了するため、正規のCloseハンドシェイクを
/// 経由せずに切断される。tungstenite側ではこれがCloseフレームではなく
/// エラー(`Connection reset without closing handshake`)として現れるため、
/// ストリーム終了(None)・Closeフレーム・エラーのいずれも「イベントではない」
/// として扱う。Text/Binaryなど実際のメッセージが届いた場合のみ失敗させる。
pub async fn assert_no_event(ws: &mut WsStream) {
    match tokio::time::timeout(Duration::from_millis(300), ws.next()).await {
        Err(_) => {}                          // タイムアウト = 何も届かなかった
        Ok(None) => {}                        // ストリーム終了 = 接続が閉じられた
        Ok(Some(Ok(Message::Close(_)))) => {} // Closeフレーム = 接続が閉じられた
        Ok(Some(Err(_))) => {}                // ハンドシェイク無しの切断も同様
        Ok(Some(Ok(other))) => panic!("届かないはずのイベントが配信された: {other:?}"),
    }
}

/// 任意の文字列をそのまま送る(不正な認証メッセージの検証用)
pub async fn ws_send_raw(ws: &mut WsStream, text: &str) {
    ws.send(Message::Text(text.to_string()))
        .await
        .expect("送信に失敗した");
}
