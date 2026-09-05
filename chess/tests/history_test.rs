mod common;

use axum::http::StatusCode;
use common::*;
use sqlx::PgPool;

/// 白番・黒番が揃った対局を作る
async fn setup_game(state: &chess_server::state::AppState, white: &str, black: &str) -> String {
    let game_id = create_game(state, white).await;
    post_auth(state, &format!("/games/{game_id}/join"), black).await;
    game_id
}

#[sqlx::test(migrations = "./migrations")]
async fn finished_game_appears_in_history(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game_id = setup_game(&state, &alice, &bob).await;
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;

    let (status, body) = get_auth(&state, "/users/me/games", &alice).await;

    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["game_id"], game_id);
    assert_eq!(items[0]["my_color"], "white");
    assert_eq!(items[0]["opponent_username"], "bob");
    assert_eq!(items[0]["result"], "black_win");
    // 投了した側なので負け
    assert_eq!(items[0]["outcome"], "loss");
    assert_eq!(items[0]["end_reason"], "resignation");
}

#[sqlx::test(migrations = "./migrations")]
async fn outcome_is_relative_to_the_viewer(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game_id = setup_game(&state, &alice, &bob).await;
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;

    // 同じ対局でも、見る人によって win / loss が反転する
    let (_, alice_view) = get_auth(&state, "/users/me/games", &alice).await;
    let (_, bob_view) = get_auth(&state, "/users/me/games", &bob).await;

    assert_eq!(alice_view[0]["outcome"], "loss");
    assert_eq!(alice_view[0]["opponent_username"], "bob");

    assert_eq!(bob_view[0]["outcome"], "win");
    assert_eq!(bob_view[0]["my_color"], "black");
    assert_eq!(bob_view[0]["opponent_username"], "alice");
}

#[sqlx::test(migrations = "./migrations")]
async fn unfinished_games_are_excluded(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    // 募集中の対局
    create_game(&state, &alice).await;
    // 進行中の対局
    let in_progress = setup_game(&state, &alice, &bob).await;
    make_move(&state, &in_progress, &alice, "e2e4").await;

    let (_, body) = get_auth(&state, "/users/me/games", &alice).await;

    assert!(
        body.as_array().unwrap().is_empty(),
        "終了していない対局が履歴に出ている: {body}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn other_users_games_are_excluded(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let stranger = register_user(&state, "stranger").await;

    let game_id = setup_game(&state, &alice, &bob).await;
    post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;

    let (_, body) = get_auth(&state, "/users/me/games", &stranger).await;

    assert!(
        body.as_array().unwrap().is_empty(),
        "無関係なユーザーに他人の対局が見えている"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn checkmate_is_recorded_with_move_count(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let game_id = setup_game(&state, &alice, &bob).await;

    // Fool's mate
    make_move(&state, &game_id, &alice, "f2f3").await;
    make_move(&state, &game_id, &bob, "e7e5").await;
    make_move(&state, &game_id, &alice, "g2g4").await;
    make_move(&state, &game_id, &bob, "d8h4").await;

    let (_, body) = get_auth(&state, "/users/me/games", &bob).await;

    assert_eq!(body[0]["outcome"], "win");
    assert_eq!(body[0]["end_reason"], "checkmate");
    assert_eq!(body[0]["move_count"], 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn history_is_ordered_newest_first(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    let first = setup_game(&state, &alice, &bob).await;
    post_auth(&state, &format!("/games/{first}/resign"), &alice).await;

    let second = setup_game(&state, &alice, &bob).await;
    post_auth(&state, &format!("/games/{second}/resign"), &alice).await;

    let (_, body) = get_auth(&state, "/users/me/games", &alice).await;

    assert_eq!(body[0]["game_id"], second, "新しい対局が先頭に来る");
    assert_eq!(body[1]["game_id"], first);
}

#[sqlx::test(migrations = "./migrations")]
async fn limit_and_offset_work(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    for _ in 0..3 {
        let game_id = setup_game(&state, &alice, &bob).await;
        post_auth(&state, &format!("/games/{game_id}/resign"), &alice).await;
    }

    let (_, page1) = get_auth(&state, "/users/me/games?limit=2", &alice).await;
    let (_, page2) = get_auth(&state, "/users/me/games?limit=2&offset=2", &alice).await;

    assert_eq!(page1.as_array().unwrap().len(), 2);
    assert_eq!(page2.as_array().unwrap().len(), 1);
    // ページ間で重複しない
    assert_ne!(page1[0]["game_id"], page2[0]["game_id"]);
    assert_ne!(page1[1]["game_id"], page2[0]["game_id"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_limit_is_rejected(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;

    let (status, _) = get_auth(&state, "/users/me/games?limit=0", &alice).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = get_auth(&state, "/users/me/games?limit=1000", &alice).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn history_requires_authentication(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = get_auth(&state, "/users/me/games", "not-a-valid-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
