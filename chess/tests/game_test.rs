mod common;

use axum::http::StatusCode;
use chess_server::state::AppState;
use common::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

async fn fetch_game_status(state: &AppState, game_id: &str) -> String {
    sqlx::query("SELECT status::text AS status FROM games WHERE id = $1")
        .bind(Uuid::parse_str(game_id).unwrap())
        .fetch_one(&state.db)
        .await
        .unwrap()
        .get::<String, _>("status")
}

async fn fetch_black_user_id(state: &AppState, game_id: &str) -> Option<Uuid> {
    sqlx::query("SELECT black_user_id FROM games WHERE id = $1")
        .bind(Uuid::parse_str(game_id).unwrap())
        .fetch_one(&state.db)
        .await
        .unwrap()
        .get::<Option<Uuid>, _>("black_user_id")
}

// ===== join =====

#[sqlx::test(migrations = "./migrations")]
async fn join_sets_black_and_starts_game(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;

    // 参加前は waiting で黒は未設定
    assert_eq!(fetch_game_status(&state, &game_id).await, "waiting");
    assert!(fetch_black_user_id(&state, &game_id).await.is_none());

    let (status, _) = post_auth(&state, &format!("/games/{game_id}/join"), &black).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(fetch_game_status(&state, &game_id).await, "in_progress");
    assert!(fetch_black_user_id(&state, &game_id).await.is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_join_own_game(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let game_id = create_game(&state, &white).await;

    let (status, _) = post_auth(&state, &format!("/games/{game_id}/join"), &white).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_join_full_game(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let third = register_user(&state, "third").await;
    let game_id = create_game(&state, &white).await;

    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    // 既に埋まっている対局への参加は409
    let (status, _) = post_auth(&state, &format!("/games/{game_id}/join"), &third).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

// ===== move =====

#[sqlx::test(migrations = "./migrations")]
async fn move_is_recorded_in_db(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    let (status, _) = make_move(&state, &game_id, &white, "e2e4").await;
    assert_eq!(status, StatusCode::OK);

    let row = sqlx::query("SELECT uci, fen_after FROM moves WHERE game_id = $1")
        .bind(Uuid::parse_str(&game_id).unwrap())
        .fetch_one(&state.db)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("uci"), "e2e4");
    assert!(
        row.get::<String, _>("fen_after").contains(" b "),
        "手番が黒に移っている"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn third_party_cannot_move(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let stranger = register_user(&state, "stranger").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    let (status, _) = make_move(&state, &game_id, &stranger, "e2e4").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_move_out_of_turn(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    // 初手は白番なのに黒が指そうとする
    let (status, _) = make_move(&state, &game_id, &black, "e7e5").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn illegal_move_is_rejected(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    // ポーンは初手で3マス進めない
    let (status, _) = make_move(&state, &game_id, &white, "e2e5").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn malformed_uci_is_rejected(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    let (status, _) = make_move(&state, &game_id, &white, "zzzz").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn alternating_moves_are_recorded_in_order(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    make_move(&state, &game_id, &white, "e2e4").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g1f3").await;

    let rows = sqlx::query("SELECT uci FROM moves WHERE game_id = $1 ORDER BY id")
        .bind(Uuid::parse_str(&game_id).unwrap())
        .fetch_all(&state.db)
        .await
        .unwrap();

    let ucis: Vec<String> = rows.iter().map(|r| r.get::<String, _>("uci")).collect();
    assert_eq!(ucis, vec!["e2e4", "e7e5", "g1f3"]);
}
