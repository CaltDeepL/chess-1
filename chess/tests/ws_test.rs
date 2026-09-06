//! WebSocket のイベント配信の統合テスト。
//!
//! 【重要】購読開始 → イベント発生 の順序を必ず守ること。
//! 配信は broadcast チャネルのため、購読を開始した後に起きたイベントしか
//! 届かない。順序を逆に書くと、実装が正しくてもテストだけが落ちる
//! (task-27 の 404 と同じく「テストの組み立て方」に起因する失敗)。
//!
//! 認証は upgrade 後の最初の Text メッセージ `{"token":"..."}` で行う。
//! `connect_ws` がその送信と購読完了の待機まで面倒を見る。

mod common;

use common::*;
use sqlx::PgPool;

/// 白番・黒番が揃った対局と、両者のトークンを用意する
///
/// ユーザー名は毎回ユニークにする(同じテスト内で複数回呼ぶケースがあり、
/// 固定名だと2回目の登録が既存ユーザーと衝突して409になる)。
async fn setup_game(state: &chess_server::state::AppState) -> (String, String, String) {
    // ユーザー名は32文字までなので、UUID全体(36文字)ではなく先頭8文字だけ使う
    let suffix = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
    let white = register_user(state, &format!("white-{suffix}")).await;
    let black = register_user(state, &format!("black-{suffix}")).await;
    let game_id = create_game(state, &white).await;
    post_auth(state, &format!("/games/{game_id}/join"), &black).await;
    (game_id, white, black)
}

#[sqlx::test(migrations = "./migrations")]
async fn move_is_broadcast_to_both_players(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, white, black) = setup_game(&state).await;

    // 1. 先に両者が購読する
    let mut ws_white = connect_ws(addr, &game_id, &white).await;
    let mut ws_black = connect_ws(addr, &game_id, &black).await;

    // 2. その後に指し手を発生させる(RESTは従来どおり oneshot)
    make_move(&state, &game_id, &white, "e2e4").await;

    // 3. 双方に届く
    let to_white = next_event(&mut ws_white).await;
    let to_black = next_event(&mut ws_black).await;

    assert_eq!(to_white["uci"], "e2e4");
    // 指した本人にも配信される(自分の手も同じ経路で盤面に反映するため)
    assert_eq!(to_white, to_black);
}

#[sqlx::test(migrations = "./migrations")]
async fn moves_are_delivered_in_order(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, white, black) = setup_game(&state).await;

    let mut ws = connect_ws(addr, &game_id, &black).await;

    make_move(&state, &game_id, &white, "e2e4").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g1f3").await;

    let mut ucis = Vec::new();
    for _ in 0..3 {
        let event = next_event(&mut ws).await;
        ucis.push(event["uci"].as_str().unwrap().to_string());
    }

    assert_eq!(ucis, vec!["e2e4", "e7e5", "g1f3"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn resign_is_broadcast(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, white, black) = setup_game(&state).await;

    let mut ws = connect_ws(addr, &game_id, &black).await;

    post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;

    // 相手が投了したことを、対局中の画面がリロードなしで知る必要がある
    let event = next_event(&mut ws).await;
    assert_eq!(event["result"], "black_win");
    assert_eq!(event["end_reason"], "resignation");
}

#[sqlx::test(migrations = "./migrations")]
async fn checkmate_is_broadcast(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, white, black) = setup_game(&state).await;

    let mut ws = connect_ws(addr, &game_id, &white).await;

    // Fool's mate
    make_move(&state, &game_id, &white, "f2f3").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g2g4").await;
    make_move(&state, &game_id, &black, "d8h4").await;

    // 4手分の move イベントを読み、最後のものを見る
    let mut last_move = next_event(&mut ws).await;
    for _ in 0..3 {
        last_move = next_event(&mut ws).await;
    }
    assert_eq!(last_move["uci"], "d8h4");
    assert_eq!(last_move["is_game_over"], true);

    // 終局情報は move とは別の game_over イベントで届く実装なので、もう1回読む
    let game_over = next_event(&mut ws).await;
    assert_eq!(game_over["result"], "black_win");
    assert_eq!(game_over["end_reason"], "checkmate");
}

#[sqlx::test(migrations = "./migrations")]
async fn other_games_do_not_leak_events(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_a, white_a, _) = setup_game(&state).await;
    let (game_b, white_b, _) = setup_game(&state).await;

    // 対局Bを購読した状態で、対局Aで指す
    let mut ws_b = connect_ws(addr, &game_b, &white_b).await;
    make_move(&state, &game_a, &white_a, "e2e4").await;

    // チャネルが対局ごとに分かれていなければ、ここに届いてしまう
    assert_no_event(&mut ws_b).await;

    // 対局Bの指し手はきちんと届く(購読自体が壊れていないことの確認)
    make_move(&state, &game_b, &white_b, "d2d4").await;
    assert_eq!(next_event(&mut ws_b).await["uci"], "d2d4");
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_token_gets_error_and_no_events(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, white, _) = setup_game(&state).await;

    // upgrade 自体は成功する。認証は最初の Text メッセージで行うため、
    // 失敗はエラーメッセージ + 切断という形で現れる
    let mut ws = open_ws(addr, &game_id).await;
    send_auth(&mut ws, "not-a-valid-token").await;

    let event = next_event(&mut ws).await;
    assert!(
        event["error"].is_string(),
        "エラーメッセージが返るはず: {event}"
    );

    // 切断されているので、以降のイベントは届かない
    make_move(&state, &game_id, &white, "e2e4").await;
    assert_no_event(&mut ws).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn third_party_cannot_subscribe(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, white, _) = setup_game(&state).await;
    let stranger = register_user(&state, "stranger").await;

    // トークンは有効だが、この対局の参加者ではない
    let mut ws = open_ws(addr, &game_id).await;
    send_auth(&mut ws, &stranger).await;

    let event = next_event(&mut ws).await;
    assert!(event["error"].is_string(), "参加者チェックで弾かれるはず");

    make_move(&state, &game_id, &white, "e2e4").await;
    assert_no_event(&mut ws).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn malformed_auth_message_is_rejected(pool: PgPool) {
    let state = test_state(pool);
    let addr = spawn_server(&state).await;
    let (game_id, _, _) = setup_game(&state).await;

    let mut ws = open_ws(addr, &game_id).await;
    // token フィールドを持たない JSON
    ws_send_raw(&mut ws, r#"{"foo":"bar"}"#).await;

    let event = next_event(&mut ws).await;
    assert!(event["error"].is_string());
}
