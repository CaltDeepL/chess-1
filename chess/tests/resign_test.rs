mod common;

use axum::http::StatusCode;
use chess_server::state::AppState;
use common::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// games テーブルの status / result / end_reason を直接読む
async fn fetch_game_row(
    state: &AppState,
    game_id: &str,
) -> (String, Option<String>, Option<String>) {
    let row = sqlx::query(
        "SELECT status::text AS status, result::text AS result, end_reason FROM games WHERE id = $1",
    )
    .bind(Uuid::parse_str(game_id).unwrap())
    .fetch_one(&state.db)
    .await
    .unwrap();

    (
        row.get::<String, _>("status"),
        row.get::<Option<String>, _>("result"),
        row.get::<Option<String>, _>("end_reason"),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn resign_reflects_result_in_db(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    let (status, _) = post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;
    assert_eq!(status, StatusCode::OK);

    // APIが200を返すだけでなく、DBが実際に更新されたことを確認する
    // (task-07 で、レスポンスは正常なのにDB更新が失敗していたサイレント障害があった)
    let (db_status, result, end_reason) = fetch_game_row(&state, &game_id).await;
    assert_eq!(db_status, "finished");
    assert_eq!(
        result.as_deref(),
        Some("black_win"),
        "白が投了したので黒の勝ち"
    );
    assert_eq!(end_reason.as_deref(), Some("resignation"));
}

#[sqlx::test(migrations = "./migrations")]
async fn black_resign_makes_white_win(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    post_auth(&state, &format!("/games/{game_id}/resign"), &black).await;

    let (_, result, _) = fetch_game_row(&state, &game_id).await;
    assert_eq!(result.as_deref(), Some("white_win"));
}

#[sqlx::test(migrations = "./migrations")]
async fn resign_before_opponent_joins_returns_409(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let game_id = create_game(&state, &white).await;

    let (status, _) = post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (db_status, result, end_reason) = fetch_game_row(&state, &game_id).await;
    assert_eq!(db_status, "waiting");
    assert!(result.is_none());
    assert!(end_reason.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn resign_twice_returns_409(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    let (first, _) = post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;
    assert_eq!(first, StatusCode::OK);

    let (second, _) = post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;
    assert_eq!(second, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn move_after_resign_returns_409(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;

    // メモリ上の局面の有無ではなく、DB の終了状態を見て 409 に統一する。
    let (status, _) = make_move(&state, &game_id, &white, "e2e4").await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn third_party_cannot_resign(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let stranger = register_user(&state, "stranger").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    let (status, _) = post_auth(&state, &format!("/games/{game_id}/resign"), &stranger).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
