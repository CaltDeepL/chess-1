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

/// 投了で終了した対局も GET /games/:id で取得できる。
///
/// 進行中の対局はメモリ上の局面を返すが、終局時にメモリから削除されるため、
/// 以前は DB に行があるのに 404 になっていた。棋譜の再生画面と、
/// 終局後のリロードの両方がこれで壊れていた。
#[sqlx::test(migrations = "./migrations")]
async fn finished_game_is_still_retrievable(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    make_move(&state, &game_id, &white, "e2e4").await;
    post_auth(&state, &format!("/games/{game_id}/resign"), &white).await;

    let (status, body) = get_json(&state, &format!("/games/{game_id}")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "finished");
    assert_eq!(body["result"], "black_win");
    // 最終局面が DB の FEN から復元されている
    assert!(
        body["fen"]
            .as_str()
            .unwrap()
            .starts_with("rnbqkbnr/pppppppp"),
        "初期局面のままではなく、1手指した後の局面が返る: {}",
        body["fen"]
    );
}

/// チェックメイトで終了した対局も取得でき、終局判定が復元される
#[sqlx::test(migrations = "./migrations")]
async fn checkmated_game_reports_game_over(pool: PgPool) {
    let state = test_state(pool);
    let white = register_user(&state, "white").await;
    let black = register_user(&state, "black").await;
    let game_id = create_game(&state, &white).await;
    post_auth(&state, &format!("/games/{game_id}/join"), &black).await;

    // Fool's mate
    make_move(&state, &game_id, &white, "f2f3").await;
    make_move(&state, &game_id, &black, "e7e5").await;
    make_move(&state, &game_id, &white, "g2g4").await;
    make_move(&state, &game_id, &black, "d8h4").await;

    let (status, body) = get_json(&state, &format!("/games/{game_id}")).await;

    assert_eq!(status, StatusCode::OK);
    // FEN から復元した局面でも終局・王手の判定が効く
    assert_eq!(body["is_game_over"], true);
    assert_eq!(body["is_check"], true);
    assert_eq!(
        body["end_reason"].is_null(),
        true,
        "この応答に end_reason は含まれない"
    );
}

/// 存在しない対局は従来どおり404
#[sqlx::test(migrations = "./migrations")]
async fn unknown_game_returns_404(pool: PgPool) {
    let state = test_state(pool);
    let missing = uuid::Uuid::new_v4();

    let (status, _) = get_json(&state, &format!("/games/{missing}")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
