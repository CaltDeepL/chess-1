mod common;

use axum::http::StatusCode;
use common::*;
use sqlx::PgPool;

/// OpenAPI仕様に載っているRESTエンドポイントの数
///
/// エンドポイントを追加・削除したら、この数を更新すると同時に
/// `#[utoipa::path]` の付与漏れがないか確認すること。
/// (WebSocket の /ws/games/{id} は OpenAPI の対象外)
const EXPECTED_PATH_COUNT: usize = 12;

#[sqlx::test(migrations = "./migrations")]
async fn openapi_json_is_served(pool: PgPool) {
    let state = test_state(pool);
    let (status, body) = get_json(&state, "/openapi.json").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["openapi"].is_string(), "OpenAPIのバージョンが含まれる");
    assert_eq!(body["info"]["title"], "chess-app API");
}

#[sqlx::test(migrations = "./migrations")]
async fn openapi_covers_all_rest_routes(pool: PgPool) {
    let state = test_state(pool);
    let (_, body) = get_json(&state, "/openapi.json").await;

    let paths = body["paths"].as_object().expect("paths が存在する");

    assert_eq!(
        paths.len(),
        EXPECTED_PATH_COUNT,
        "RESTエンドポイントの数が変わっています。\
         #[utoipa::path] の付与漏れがないか確認し、EXPECTED_PATH_COUNT を更新してください。\
         現在のパス: {:?}",
        paths.keys().collect::<Vec<_>>()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn openapi_declares_bearer_auth(pool: PgPool) {
    let state = test_state(pool);
    let (_, body) = get_json(&state, "/openapi.json").await;

    let scheme = &body["components"]["securitySchemes"]["bearer_auth"];
    assert_eq!(scheme["type"], "http");
    assert_eq!(scheme["scheme"], "bearer");
    assert_eq!(scheme["bearerFormat"], "JWT");
}

#[sqlx::test(migrations = "./migrations")]
async fn protected_endpoints_require_bearer_auth(pool: PgPool) {
    let state = test_state(pool);
    let (_, body) = get_json(&state, "/openapi.json").await;

    // 認証が必要なエンドポイントに security が宣言されているか
    for (path, method) in [
        ("/games", "post"),
        ("/games", "get"),
        ("/games/{id}/join", "post"),
        ("/games/{id}/move", "post"),
        ("/games/{id}/resign", "post"),
        ("/games/{id}/moves", "get"),
    ] {
        let security = &body["paths"][path][method]["security"];
        assert!(
            security.is_array() && !security.as_array().unwrap().is_empty(),
            "{method} {path} に security の宣言がありません"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn docs_page_is_served(pool: PgPool) {
    let state = test_state(pool);
    let (status, _) = get_html(&state, "/docs").await;
    assert_eq!(status, StatusCode::OK);
}

/// エラーレスポンスは全て ProblemDetails スキーマを参照していなければならない。
/// 手作業で各 #[utoipa::path] に body を足すため、書き漏れを検知する。
#[test]
fn error_responses_reference_problem_details() {
    let doc = openapi_json(); // 既存テストのヘルパーに合わせる
    let paths = doc["paths"].as_object().unwrap();

    let mut missing = Vec::new();

    for (path, item) in paths {
        for (method, op) in item.as_object().unwrap() {
            let Some(responses) = op["responses"].as_object() else {
                continue;
            };
            for (status, response) in responses {
                // 4xx/5xx だけを対象にする
                if !status.starts_with('4') && !status.starts_with('5') {
                    continue;
                }
                let schema_ref =
                    response["content"]["application/problem+json"]["schema"]["$ref"].as_str();
                if schema_ref != Some("#/components/schemas/ProblemDetails") {
                    missing.push(format!("{method} {path} -> {status}"));
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "ProblemDetails を参照していないエラーレスポンス: {missing:#?}"
    );
}
