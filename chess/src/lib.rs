pub mod abandon;
mod auth;
mod domain;
mod errors;
mod models;
mod openapi;
pub mod rating;
mod routes;
pub mod state;

use axum::{
    http::{header, Method},
    response::Html,
    routing::get,
    Json, Router,
};
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

use openapi::ApiDoc;
use state::AppState;

/// #[utoipa::path]が付いたハンドラからルーターとOpenAPI仕様を組み立てる。
/// build_router()とopenapi_spec()の両方から呼ばれる唯一の定義元にすることで、
/// 実際に配信されるAPIとOpenAPIドキュメントが食い違わないようにする。
fn openapi_router() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(routes::health::health_check))
        .routes(routes!(auth::register))
        .routes(routes!(auth::login))
        // /users/ranking は /users/:id より先に評価される必要はない(axumは
        // 静的セグメントを優先する)が、読みやすさのため /users/:id の前に置く。
        .route(
            "/users/ranking",
            axum::routing::get(crate::routes::ranking::get_ranking),
        )
        .routes(routes!(routes::user::get_user))
        .route(
            "/users/me/games",
            axum::routing::get(crate::routes::history::list_my_games),
        )
        // 同じパスの GET/POST は1つの routes!() にまとめる
        .routes(routes!(routes::game::list_games, routes::game::create_game))
        .routes(routes!(routes::game::get_game))
        .routes(routes!(routes::game::get_moves))
        .routes(routes!(routes::game::join_game))
        .routes(routes!(routes::game::make_move))
        .routes(routes!(routes::game::resign_game))
        .route("/auth/logout", axum::routing::post(crate::auth::logout))
        .route(
            "/games/:id/claim-abandonment",
            axum::routing::post(crate::routes::game::claim_abandonment),
        )
        .split_for_parts()
}

/// 実際に配信されるOpenAPI仕様をHTTP/DBを経由せず直接取得する。
/// ProblemDetailsの参照漏れなど、静的な仕様チェックのテストで使う。
pub fn openapi_spec() -> utoipa::openapi::OpenApi {
    openapi_router().1
}

pub fn build_router(state: AppState) -> Router {
    let (router, api) = openapi_router();

    router
        // WebSocketは仕様の対象外(OpenAPIはHTTPのみ)なので通常のrouteで追加
        .route("/ws/games/:id", get(routes::ws::ws_handler))
        // 運用用の内部APIなのでOpenAPIには載せない
        .route(
            "/internal/sweep",
            axum::routing::post(crate::routes::internal::sweep),
        )
        .route(
            "/openapi.json",
            get(move || async move { Json(api.clone()) }),
        )
        .route("/docs", get(docs))
        .layer(build_cors_layer())
        .with_state(state)
}

// 本番のフロントエンドのオリジンはFRONTEND_ORIGIN環境変数で指定する(カンマ区切りで複数可)。
// 例: FRONTEND_ORIGIN=https://chess-frontend.onrender.com,https://chess.example.com
// 未設定時はローカル開発用のVite開発サーバーのポートをフォールバックとして許可しておく
// (5173が既に使用中だと5174、5175...とViteが自動でずれていくため複数残してある)。
const DEFAULT_DEV_ORIGINS: &str =
    "http://localhost:5173,http://localhost:5174,http://localhost:5175";

fn build_cors_layer() -> CorsLayer {
    let allowed_origins: Vec<axum::http::HeaderValue> = std::env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| DEFAULT_DEV_ORIGINS.to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
}

async fn docs() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>chess-app API</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    SwaggerUIBundle({ url: '/openapi.json', dom_id: '#swagger-ui' });
  </script>
</body>
</html>"#,
    )
}
