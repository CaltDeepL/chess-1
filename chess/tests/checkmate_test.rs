mod common;

use axum::http::StatusCode;
use chess_server::state::AppState;
use common::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

async fn fetch_outcome(
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

/// 対局を作成して両者が参加した状態にする
async fn setup_game(state: &AppState) -> (String, String, String) {
    let white = register_user(state, "white").await;
    let black = register_user(state, "black").await;
    let game_id = create_game(state, &white).await;
    post_auth(state, &format!("/games/{game_id}/join"), &black).await;
    (game_id, white, black)
}

#[sqlx::test(migrations = "./migrations")]
async fn fools_mate_records_black_win(pool: PgPool) {
    let state = test_state(pool);
    let (game_id, white, black) = setup_game(&state).await;

    // Fool's mate: 最短の詰み(2手)。黒のクイーンがh4に来て白が詰む
    make_move(&state, &game_id, &white, "f2f3").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g2g4").await;
    let (status, body) = make_move(&state, &game_id, &black, "d8h4").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_game_over"], true, "レスポンスが終局を示している");

    // task-07: レスポンスが正常でもDB更新が失敗しているケースがあったため、
    // 必ずDBの中身まで確認する
    let (db_status, result, end_reason) = fetch_outcome(&state, &game_id).await;
    assert_eq!(db_status, "finished");
    assert_eq!(result.as_deref(), Some("black_win"));
    assert_eq!(end_reason.as_deref(), Some("checkmate"));
}

#[sqlx::test(migrations = "./migrations")]
async fn scholars_mate_records_white_win(pool: PgPool) {
    let state = test_state(pool);
    let (game_id, white, black) = setup_game(&state).await;

    // Scholar's mate: 白のクイーンがf7を突いて詰む(4手)
    make_move(&state, &game_id, &white, "e2e4").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "f1c4").await;
    make_move(&state, &game_id, &black, "b8c6").await;
    make_move(&state, &game_id, &white, "d1h5").await;
    make_move(&state, &game_id, &black, "g8f6").await;
    let (status, _) = make_move(&state, &game_id, &white, "h5f7").await;

    assert_eq!(status, StatusCode::OK);

    let (db_status, result, end_reason) = fetch_outcome(&state, &game_id).await;
    assert_eq!(db_status, "finished");
    assert_eq!(result.as_deref(), Some("white_win"));
    assert_eq!(end_reason.as_deref(), Some("checkmate"));
}

#[sqlx::test(migrations = "./migrations")]
async fn move_after_checkmate_is_rejected(pool: PgPool) {
    let state = test_state(pool);
    let (game_id, white, black) = setup_game(&state).await;

    make_move(&state, &game_id, &white, "f2f3").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g2g4").await;
    make_move(&state, &game_id, &black, "d8h4").await;

    // 終局後に指そうとしても受け付けない
    // (checkmate後もstate.gamesから局面は削除されない[resignとは異なる]ため404にはならず、
    // チェックメイト局面ではどの手も合法手判定[shakmaty]に必ず失敗するため400になる)
    let (status, _) = make_move(&state, &game_id, &white, "e2e4").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn resign_after_checkmate_returns_409(pool: PgPool) {
    let state = test_state(pool);
    let (game_id, white, black) = setup_game(&state).await;

    make_move(&state, &game_id, &white, "f2f3").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g2g4").await;
    make_move(&state, &game_id, &black, "d8h4").await;

    // 既に終了しているので投了は409
    let (status, _) = post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn checkmate_records_all_moves(pool: PgPool) {
    let state = test_state(pool);
    let (game_id, white, black) = setup_game(&state).await;

    make_move(&state, &game_id, &white, "f2f3").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g2g4").await;
    make_move(&state, &game_id, &black, "d8h4").await;

    // 終局した手も含めて4手すべてが棋譜に残る
    let rows = sqlx::query("SELECT uci FROM moves WHERE game_id = $1 ORDER BY id")
        .bind(Uuid::parse_str(&game_id).unwrap())
        .fetch_all(&state.db)
        .await
        .unwrap();

    let ucis: Vec<String> = rows.iter().map(|r| r.get::<String, _>("uci")).collect();
    assert_eq!(ucis, vec!["f2f3", "e7e5", "g2g4", "d8h4"]);
}
