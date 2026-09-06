mod common;

use axum::http::StatusCode;
use chess_server::abandon;
use common::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// 切断時刻を過去に書き換える。
///
/// 実際に60秒待つとテストが遅くなりすぎる。判定そのものは
/// domain::abandon の単体テストで守っているので、ここでは
/// 「DBの値を読んで正しく終了処理まで進むか」を確認する。
async fn set_disconnected(
    state: &chess_server::state::AppState,
    game_id: &str,
    column: &str,
    seconds_ago: i64,
) {
    let sql =
        format!("UPDATE games SET {column} = now() - make_interval(secs => $2) WHERE id = $1");
    sqlx::query(&sql)
        .bind(Uuid::parse_str(game_id).unwrap())
        .bind(seconds_ago as f64)
        .execute(&state.db)
        .await
        .unwrap();
}

async fn game_row(
    state: &chess_server::state::AppState,
    game_id: &str,
) -> (String, Option<String>, Option<String>) {
    let row = sqlx::query(
        "SELECT status::text AS status, result::text AS result, end_reason FROM games WHERE id = $1",
    )
    .bind(Uuid::parse_str(game_id).unwrap())
    .fetch_one(&state.db)
    .await
    .unwrap();
    (row.get("status"), row.get("result"), row.get("end_reason"))
}

async fn setup_game(state: &chess_server::state::AppState, white: &str, black: &str) -> String {
    let game_id = create_game(state, white).await;
    post_auth(state, &format!("/games/{game_id}/join"), black).await;
    game_id
}

#[sqlx::test(migrations = "./migrations")]
async fn disconnected_past_grace_loses(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "white_disconnected_at", 61).await;

    let finished = abandon::finish_if_abandoned(&state, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    assert!(finished);
    let (status, result, end_reason) = game_row(&state, &game_id).await;
    assert_eq!(status, "finished");
    assert_eq!(result.as_deref(), Some("black_win"));
    assert_eq!(end_reason.as_deref(), Some("disconnection"));
}

#[sqlx::test(migrations = "./migrations")]
async fn within_grace_is_not_finished(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "white_disconnected_at", 30).await;

    let finished = abandon::finish_if_abandoned(&state, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    assert!(!finished, "猶予内で終了させてしまった");
    let (status, _, _) = game_row(&state, &game_id).await;
    assert_eq!(status, "in_progress");
}

#[sqlx::test(migrations = "./migrations")]
async fn abandonment_updates_rating(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "white_disconnected_at", 61).await;
    abandon::finish_if_abandoned(&state, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    // 投了・チェックメイトと同じくレーティングが動く。
    // 終局の経路が増えるたびに apply_rating を呼び忘れる危険があるため
    let bob_rating: i32 = sqlx::query_scalar("SELECT rating FROM users WHERE username = 'bob'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(bob_rating, 1516);
}

#[sqlx::test(migrations = "./migrations")]
async fn both_disconnected_is_a_draw(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "white_disconnected_at", 120).await;
    set_disconnected(&state, &game_id, "black_disconnected_at", 90).await;

    abandon::finish_if_abandoned(&state, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    let (_, result, end_reason) = game_row(&state, &game_id).await;
    assert_eq!(result.as_deref(), Some("draw"));
    assert_eq!(end_reason.as_deref(), Some("abandonment"));
}

#[sqlx::test(migrations = "./migrations")]
async fn finished_game_is_not_touched(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;
    set_disconnected(&state, &game_id, "black_disconnected_at", 300).await;

    let finished = abandon::finish_if_abandoned(&state, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    assert!(!finished);
    let (_, result, end_reason) = game_row(&state, &game_id).await;
    // 投了の結果が上書きされていないこと
    assert_eq!(result.as_deref(), Some("black_win"));
    assert_eq!(end_reason.as_deref(), Some("resignation"));
}

#[sqlx::test(migrations = "./migrations")]
async fn unjoined_game_is_not_abandoned(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let game_id = create_game(&state, &alice).await;

    set_disconnected(&state, &game_id, "white_disconnected_at", 300).await;

    let finished = abandon::finish_if_abandoned(&state, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    assert!(!finished, "相手がいない対局を放棄で終わらせた");
}

#[sqlx::test(migrations = "./migrations")]
async fn finishing_twice_does_not_double_apply(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;
    let uuid = Uuid::parse_str(&game_id).unwrap();

    set_disconnected(&state, &game_id, "white_disconnected_at", 61).await;

    assert!(abandon::finish_if_abandoned(&state, uuid).await.unwrap());
    assert!(
        !abandon::finish_if_abandoned(&state, uuid).await.unwrap(),
        "2回目も終了処理が走った"
    );

    let bob_rating: i32 = sqlx::query_scalar("SELECT rating FROM users WHERE username = 'bob'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(bob_rating, 1516, "レーティングが二重に適用された");
}

#[sqlx::test(migrations = "./migrations")]
async fn sweep_finishes_multiple_games(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let expired_a = setup_game(&state, &alice, &bob).await;
    let expired_b = setup_game(&state, &alice, &bob).await;
    let live = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &expired_a, "white_disconnected_at", 61).await;
    set_disconnected(&state, &expired_b, "black_disconnected_at", 200).await;
    set_disconnected(&state, &live, "white_disconnected_at", 10).await;

    let finished = abandon::sweep(&state).await.unwrap();

    assert_eq!(finished, 2);
    assert_eq!(game_row(&state, &live).await.0, "in_progress");
}

/// sweep が使う advisory lock を、終了後にきちんと解放しているか。
///
/// lock/unlock を `&state.db`(プール)越しに呼ぶと、unlock が lock を
/// 取ったのとは別のコネクションに飛び、lock を取った側がプールの中で
/// ロックを持ったまま残ってしまうことがある(このバグを一度実際に混入させた)。
/// そうなると `pg_locks` にエントリが残り続け、以後の sweep は
/// 「別プロセスが実行中」として永久にスキップされる。
#[sqlx::test(migrations = "./migrations")]
async fn sweep_releases_the_lock_for_the_next_run(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game1 = setup_game(&state, &alice, &bob).await;
    set_disconnected(&state, &game1, "white_disconnected_at", 61).await;
    assert_eq!(abandon::sweep(&state).await.unwrap(), 1);

    // sweepの排他ロックのID(src/abandon.rsのSWEEP_LOCK_IDと同じ値)。
    // 解放されていれば、sweep終了後にpg_locksへエントリは残らないはず。
    //
    // pg_locksはクラスタ全体のロックを表示するビューなので、database列で
    // 自分のテスト用DBに絞り込む必要がある。絞り込まないと、`sqlx::test`が
    // 並列実行する他のテスト(別DBでsweepを叩いているもの)のロックを
    // 誤って「自分のロックが残っている」と検知してしまう。
    const SWEEP_LOCK_ID: i64 = 1_398_228_293;
    let still_locked: bool = sqlx::query_scalar(
        "SELECT EXISTS (\
           SELECT 1 FROM pg_locks \
           WHERE locktype = 'advisory' AND objid = $1 \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database())\
         )",
    )
    .bind(SWEEP_LOCK_ID as i32)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert!(
        !still_locked,
        "sweep終了後もロックが残っている(unlockが別コネクションに飛んだ疑い)"
    );

    // ロックが本当に解放されていれば、次のsweepも正常に処理できる
    let game2 = setup_game(&state, &alice, &bob).await;
    set_disconnected(&state, &game2, "white_disconnected_at", 61).await;
    assert_eq!(
        abandon::sweep(&state).await.unwrap(),
        1,
        "ロックが解放されておらず2回目のsweepが機能していない"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sweep_endpoint_requires_the_token(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = post_json(&state, "/internal/sweep", serde_json::json!({})).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn reconnecting_clears_the_disconnect_time(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;
    let uuid = Uuid::parse_str(&game_id).unwrap();

    let alice_id = user_id_of(&state, "alice").await;

    set_disconnected(&state, &game_id, "white_disconnected_at", 50).await;
    abandon::mark_connected(&state, uuid, alice_id)
        .await
        .unwrap();

    // 猶予を過ぎる時刻になっても、復帰済みなので終了しない
    let finished = abandon::finish_if_abandoned(&state, uuid).await.unwrap();
    assert!(!finished);
}

#[sqlx::test(migrations = "./migrations")]
async fn closing_one_of_two_tabs_does_not_mark_disconnected(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;
    let uuid = Uuid::parse_str(&game_id).unwrap();
    let alice_id = user_id_of(&state, "alice").await;

    // 2タブ開いて1枚閉じる
    abandon::mark_connected(&state, uuid, alice_id)
        .await
        .unwrap();
    abandon::mark_connected(&state, uuid, alice_id)
        .await
        .unwrap();
    abandon::mark_disconnected(&state, uuid, alice_id)
        .await
        .unwrap();

    let at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT white_disconnected_at FROM games WHERE id = $1")
            .bind(uuid)
            .fetch_one(&state.db)
            .await
            .unwrap();

    assert!(at.is_none(), "タブを1枚閉じただけで切断扱いになった");

    // 2枚目を閉じたら記録される
    abandon::mark_disconnected(&state, uuid, alice_id)
        .await
        .unwrap();
    let at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT white_disconnected_at FROM games WHERE id = $1")
            .bind(uuid)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert!(at.is_some());
}

// ---------------------------------------------------------------------------
// tests/common/mod.rs に無ければ追加
// ---------------------------------------------------------------------------
//
// pub async fn user_id_of(state: &AppState, username: &str) -> uuid::Uuid {
//     sqlx::query_scalar("SELECT id FROM users WHERE username = $1")
//         .bind(username)
//         .fetch_one(&state.db)
//         .await
//         .unwrap()
// }

/// ログアウトすると、進行中の対局は猶予なしで負けになる
#[sqlx::test(migrations = "./migrations")]
async fn logout_forfeits_active_games_immediately(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    // 切断時刻を設定していない = 猶予は始まっていない
    let (status, _) = post_auth(&state, "/auth/logout", &alice).await;
    assert_eq!(status, StatusCode::OK);

    let (game_status, result, end_reason) = game_row(&state, &game_id).await;
    assert_eq!(game_status, "finished");
    assert_eq!(result.as_deref(), Some("black_win"));
    assert_eq!(end_reason.as_deref(), Some("logout"));
}

/// ログアウトによる敗北でもレーティングが動く
#[sqlx::test(migrations = "./migrations")]
async fn logout_updates_rating(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    setup_game(&state, &alice, &bob).await;

    post_auth(&state, "/auth/logout", &alice).await;

    let bob_rating: i32 = sqlx::query_scalar("SELECT rating FROM users WHERE username = 'bob'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(bob_rating, 1516);
}

/// 複数の対局に参加していれば、すべて終了する
#[sqlx::test(migrations = "./migrations")]
async fn logout_forfeits_every_active_game(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let carol = register_user(&state, "carol").await;

    let vs_bob = setup_game(&state, &alice, &bob).await;
    let vs_carol = setup_game(&state, &alice, &carol).await;

    post_auth(&state, "/auth/logout", &alice).await;

    assert_eq!(game_row(&state, &vs_bob).await.0, "finished");
    assert_eq!(game_row(&state, &vs_carol).await.0, "finished");
}

/// 相手が未参加の対局は、勝敗ではなく取り消しになる
///
/// 放置するとロビーに参加できない対局が残り続ける
#[sqlx::test(migrations = "./migrations")]
async fn logout_cancels_a_game_with_no_opponent(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let game_id = create_game(&state, &alice).await;

    post_auth(&state, "/auth/logout", &alice).await;

    let (status, result, end_reason) = game_row(&state, &game_id).await;
    assert_eq!(status, "finished");
    assert!(result.is_none(), "相手がいないのに勝敗がついた");
    assert_eq!(end_reason.as_deref(), Some("cancelled"));

    // レーティングは動かない
    let rating: i32 = sqlx::query_scalar("SELECT rating FROM users WHERE username = 'alice'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(rating, 1500);
}

/// 終了済みの対局はログアウトの影響を受けない
#[sqlx::test(migrations = "./migrations")]
async fn logout_does_not_touch_finished_games(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    post_auth(&state, &format!("/games/{game_id}/resign"), &bob).await;
    post_auth(&state, "/auth/logout", &alice).await;

    let (_, result, end_reason) = game_row(&state, &game_id).await;
    // bob の投了で alice が勝った状態が保たれている
    assert_eq!(result.as_deref(), Some("white_win"));
    assert_eq!(end_reason.as_deref(), Some("resignation"));
}

/// 猶予内に勝ちを主張しても、対局は終わらない
///
/// クライアントの時計が進んでいても勝手に勝ちにならないことの確認
#[sqlx::test(migrations = "./migrations")]
async fn claiming_within_grace_does_nothing(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "black_disconnected_at", 30).await;

    let (status, body) = post_auth(
        &state,
        &format!("/games/{game_id}/claim-abandonment"),
        &alice,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["finished"], false);
    assert_eq!(game_row(&state, &game_id).await.0, "in_progress");
}

/// 猶予を過ぎていれば、主張で即座に決着する
///
/// 残っている側は既に接続済みなので、これが無いと次の sweep まで待たされる
#[sqlx::test(migrations = "./migrations")]
async fn claiming_after_grace_finishes_the_game(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "black_disconnected_at", 61).await;

    let (_, body) = post_auth(
        &state,
        &format!("/games/{game_id}/claim-abandonment"),
        &alice,
    )
    .await;

    assert_eq!(body["finished"], true);
    let (_, result, end_reason) = game_row(&state, &game_id).await;
    assert_eq!(result.as_deref(), Some("white_win"));
    assert_eq!(end_reason.as_deref(), Some("disconnection"));
}

/// 参加者でなければ主張できない
#[sqlx::test(migrations = "./migrations")]
async fn a_stranger_cannot_claim(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let stranger = register_user(&state, "stranger").await;
    let game_id = setup_game(&state, &alice, &bob).await;

    set_disconnected(&state, &game_id, "black_disconnected_at", 61).await;

    let (status, _) = post_auth(
        &state,
        &format!("/games/{game_id}/claim-abandonment"),
        &stranger,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(game_row(&state, &game_id).await.0, "in_progress");
}

/// 相手の残り秒数を取得できる
#[sqlx::test(migrations = "./migrations")]
async fn opponent_grace_remaining_counts_down(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let game_id = setup_game(&state, &alice, &bob).await;
    let uuid = Uuid::parse_str(&game_id).unwrap();
    let alice_id = user_id_of(&state, "alice").await;
    let bob_id = user_id_of(&state, "bob").await;

    set_disconnected(&state, &game_id, "black_disconnected_at", 20).await;

    let remaining = chess_server::abandon::opponent_grace_remaining(&state, uuid, alice_id)
        .await
        .unwrap();

    let (opponent_id, seconds) = remaining.expect("相手の切断が検出されていない");
    assert_eq!(opponent_id, bob_id);
    assert!((39..=41).contains(&seconds), "残り約40秒のはず: {seconds}");

    // 切断している本人から見れば、相手（alice）は接続中
    let none = chess_server::abandon::opponent_grace_remaining(&state, uuid, bob_id)
        .await
        .unwrap();
    assert!(none.is_none());
}
