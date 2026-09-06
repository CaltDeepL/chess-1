mod common;

use axum::http::StatusCode;
use common::*;
use sqlx::PgPool;

/// 2人が対局し、白（第1引数）が勝った状態にする
async fn play_and_win(
    state: &chess_server::state::AppState,
    winner_token: &str,
    loser_token: &str,
) {
    let game_id = create_game(state, winner_token).await;
    post_auth(state, &format!("/games/{game_id}/join"), loser_token).await;
    post_auth(state, &format!("/games/{game_id}/resign"), loser_token).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn ranking_is_ordered_by_rating(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;

    play_and_win(&state, &alice, &bob).await;

    let (status, body) = get_json(&state, "/users/ranking").await;

    assert_eq!(status, StatusCode::OK);
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["username"], "alice");
    assert_eq!(entries[0]["rank"], 1);
    assert_eq!(entries[0]["rating"], 1516);
    assert_eq!(entries[1]["username"], "bob");
    assert_eq!(entries[1]["rank"], 2);
    assert_eq!(entries[0]["games_played"], 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn users_without_finished_games_are_excluded(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    // 登録しただけのユーザー
    register_user(&state, "lurker").await;

    play_and_win(&state, &alice, &bob).await;

    let (_, body) = get_json(&state, "/users/ranking").await;

    let names: Vec<&str> = body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["username"].as_str().unwrap())
        .collect();

    assert!(
        !names.contains(&"lurker"),
        "1局も終えていないユーザーが載っている: {names:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn same_rating_shares_the_same_rank(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    let carol = register_user(&state, "carol").await;
    let dave = register_user(&state, "dave").await;

    // alice と carol が勝ち（1516）、bob と dave が負け（1484）
    play_and_win(&state, &alice, &bob).await;
    play_and_win(&state, &carol, &dave).await;

    let (_, body) = get_json(&state, "/users/ranking").await;
    let entries = body["entries"].as_array().unwrap();

    // 1位が2人いるので、次は3位（2位ではない）
    assert_eq!(entries[0]["rank"], 1);
    assert_eq!(entries[1]["rank"], 1);
    assert_eq!(entries[2]["rank"], 3);
    assert_eq!(entries[3]["rank"], 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn ranking_is_public(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    play_and_win(&state, &alice, &bob).await;

    // 認証なしでも見られる。ログイン前のトップページから見せられるように。
    let (status, body) = get_json(&state, "/users/ranking").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["me"].is_null(), "認証していないので me は返らない");
}

#[sqlx::test(migrations = "./migrations")]
async fn authenticated_request_includes_own_rank(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    play_and_win(&state, &alice, &bob).await;

    let (_, body) = get_auth(&state, "/users/ranking", &bob).await;

    assert_eq!(body["me"]["username"], "bob");
    assert_eq!(body["me"]["rank"], 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn own_rank_is_returned_even_when_outside_the_limit(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    play_and_win(&state, &alice, &bob).await;

    // 1件しか返さない指定でも、圏外の自分の順位は取れる
    let (_, body) = get_auth(&state, "/users/ranking?limit=1", &bob).await;

    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["me"]["username"], "bob");
    assert_eq!(body["me"]["rank"], 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_token_does_not_break_the_ranking(pool: PgPool) {
    let state = test_state(pool);
    let alice = register_user(&state, "alice").await;
    let bob = register_user(&state, "bob").await;
    play_and_win(&state, &alice, &bob).await;

    // 期限切れトークンで開いても、一覧は見えるべき
    let (status, body) = get_auth(&state, "/users/ranking", "not-a-valid-token").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["entries"].as_array().unwrap().len(), 2);
    assert!(body["me"].is_null());
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_limit_is_rejected(pool: PgPool) {
    let state = test_state(pool);

    let (status, _) = get_json(&state, "/users/ranking?limit=0").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = get_json(&state, "/users/ranking?limit=500").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
