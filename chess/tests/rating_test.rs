mod common;

use common::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

async fn rating_of(state: &chess_server::state::AppState, username: &str) -> i32 {
    sqlx::query("SELECT rating FROM users WHERE username = $1")
        .bind(username)
        .fetch_one(&state.db)
        .await
        .unwrap()
        .get("rating")
}

async fn deltas(
    state: &chess_server::state::AppState,
    game_id: &str,
) -> (Option<i32>, Option<i32>) {
    let row = sqlx::query("SELECT white_rating_delta, black_rating_delta FROM games WHERE id = $1")
        .bind(Uuid::parse_str(game_id).unwrap())
        .fetch_one(&state.db)
        .await
        .unwrap();
    (row.get("white_rating_delta"), row.get("black_rating_delta"))
}

async fn setup_game(state: &chess_server::state::AppState, white: &str, black: &str) -> String {
    let game_id = create_game(state, white).await;
    post_auth(state, &format!("/games/{game_id}/join"), black).await;
    game_id
}

#[sqlx::test(migrations = "./migrations")]
async fn new_user_starts_at_1500(pool: PgPool) {
    let state = test_state(pool);
    register_user(&state, "alice").await;

    assert_eq!(rating_of(&state, "alice").await, 1500);
}

#[sqlx::test(migrations = "./migrations")]
async fn resign_moves_ratings_in_opposite_directions(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game_id = setup_game(&state, &alice, &bob).await;
    // 白（alice）が投了 → 黒（bob）の勝ち
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;

    // 同格同士なので K/2 = 16 動く
    assert_eq!(rating_of(&state, "alice").await, 1484);
    assert_eq!(rating_of(&state, "bob").await, 1516);

    let (white_delta, black_delta) = deltas(&state, &game_id).await;
    assert_eq!(white_delta, Some(-16));
    assert_eq!(black_delta, Some(16));
}

#[sqlx::test(migrations = "./migrations")]
async fn checkmate_applies_rating(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game_id = setup_game(&state, &alice, &bob).await;

    // Fool's mate: 黒（bob）の勝ち
    make_move(&state, &game_id, &alice, "f2f3").await;
    make_move(&state, &game_id, &bob, "e7e5").await;
    make_move(&state, &game_id, &alice, "g2g4").await;
    make_move(&state, &game_id, &bob, "d8h4").await;

    // 投了と同じ関数を通ることを、実際の値で確認する
    // （経路ごとに書くと片方だけ漏れる。task-07 の再来を防ぐ）
    assert_eq!(rating_of(&state, "alice").await, 1484);
    assert_eq!(rating_of(&state, "bob").await, 1516);
}

#[sqlx::test(migrations = "./migrations")]
async fn total_rating_is_preserved(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    // 何局か戦わせても、2人の合計は 3000 のまま動かない
    for _ in 0..3 {
        let game_id = setup_game(&state, &alice, &bob).await;
        post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;
    }

    let total = rating_of(&state, "alice").await + rating_of(&state, "bob").await;
    assert_eq!(total, 3000, "レーティングの総和が保存されていない");
}

#[sqlx::test(migrations = "./migrations")]
async fn rating_is_not_applied_twice(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game_id = setup_game(&state, &alice, &bob).await;
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;

    let after_first = rating_of(&state, "alice").await;

    // 2回目の投了は 409 だが、万一処理が走っても二重適用されないこと
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;
    chess_server::rating::apply_rating(&state.db, Uuid::parse_str(&game_id).unwrap())
        .await
        .unwrap();

    assert_eq!(rating_of(&state, "alice").await, after_first);
}

#[sqlx::test(migrations = "./migrations")]
async fn unjoined_game_does_not_change_rating(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;

    let game_id = create_game(&state, &alice).await;
    // 相手がいないまま投了（実装が許すなら）
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;

    assert_eq!(
        rating_of(&state, "alice").await,
        1500,
        "相手がいない対局でレーティングが動いた"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn stronger_player_gains_less_by_winning(pool: PgPool) {
    let state = test_state(pool);
    let strong = register_user(&state, "strong").await;
    let weak = register_user(&state, "weak").await;

    sqlx::query("UPDATE users SET rating = 1900 WHERE username = 'strong'")
        .execute(&state.db)
        .await
        .unwrap();

    // strong が白。weak が投了して strong の勝ち
    let game_id = setup_game(&state, &strong, &weak).await;
    post_auth(&state, &format!("/games/{game_id}/resign"), &weak).await;

    let gain = rating_of(&state, "strong").await - 1900;
    assert!(
        (1..=5).contains(&gain),
        "格下に勝っても伸びは小さいはず: +{gain}"
    );
}
