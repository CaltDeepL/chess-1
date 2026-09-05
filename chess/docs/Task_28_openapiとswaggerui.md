# task-28 OpenAPI 仕様の生成と Swagger UI の配信

## ゴールと完了条件
- コードから OpenAPI 3.1 の仕様を生成し、`/openapi.json` で配信する
- `/docs` で Swagger UI を表示し、ブラウザから全エンドポイントを試せるようにする
- 完了条件: Swagger UI が描画され、Authorize にトークンを入れて認証必須のエンドポイントを実行できること

## 構成
| 追加物 | 内容 |
|---|---|
| `src/openapi.rs` | `ApiDoc`(info / tags / セキュリティスキーム `bearer_auth`) |
| `#[utoipa::path]` | WebSocket を除く全 REST ハンドラ10件に付与 |
| `#[derive(ToSchema)]` | レスポンス/リクエストの各型に付与 |
| `#[derive(IntoParams)]` | `ListGamesQuery`(クエリパラメータ) |
| `GET /openapi.json` | 生成された仕様を配信 |
| `GET /docs` | Swagger UI を表示する静的 HTML |
| `tests/openapi_test.rs` | 仕様とコードの乖離を検出する統合テスト5件 |

## 設計判断の根拠

### なぜ `utoipa-swagger-ui` を使わず、自前の HTML にしたのか
`utoipa-swagger-ui` は**ビルド時に Swagger UI の zip を curl でダウンロード**する。設定は楽だが、以下のコストがある。

- CI で毎回ビルドが走るため、ダウンロード分だけ遅くなる
- ネットワーク依存が増える(`vendored` feature で回避できるがバイナリが太る)
- Render の無料プランはビルドリソースが限られる

このプロジェクトでは `/docs` を **20行程度の静的 HTML** にし、Swagger UI 本体は CDN(unpkg)から読み込む方式にした。依存は `utoipa` + `utoipa-axum` の2つだけで済み、ビルド時間に影響しない。

```rust
async fn docs() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>SwaggerUIBundle({ url: '/openapi.json', dom_id: '#swagger-ui' });</script>
</body>
</html>"#)
}
```

### なぜルート定義とドキュメントを同じ場所に置くのか
`utoipa-axum` の `OpenApiRouter` と `routes!()` マクロを使い、ルート登録と仕様生成をまとめている。

```rust
let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
    .routes(routes!(routes::game::list_games, routes::game::create_game))
    .routes(routes!(routes::game::join_game))
    // ...
    .split_for_parts();
```

`routes!()` に渡したハンドラがそのまま axum のルートになり、同時に `#[utoipa::path]` の情報から仕様が組み立てられる。**ルートを追加したのにドキュメントを書き忘れる、パスを変更したのに仕様が古いまま、といった乖離が構造的に起きにくい。**

`ApiDoc` 側に `paths(...)` や `components(schemas(...))` を手書きする必要もない(`routes!()` が自動収集する)。

### なぜ `ListGamesQuery` は `ToSchema` ではなく `IntoParams` なのか
クエリパラメータはリクエストボディのスキーマではないため。`ToSchema` を付けると components に不要なスキーマとして登録されてしまう。`IntoParams` にして `params(ListGamesQuery)` として宣言するのが正しい形。

### なぜ WebSocket を OpenAPI の対象外にしたのか
OpenAPI は HTTP のリクエスト/レスポンスを記述する仕様で、WebSocket のイベント配信は表現できない。`/ws/games/{id}` は通常の `route()` で登録し、仕様には含めていない(AsyncAPI という別仕様が該当するが、このプロジェクトでは導入しない)。

## つまずいた点と教訓

| 症状 | 原因 | 対応 |
|---|---|---|
| ビルド不能 | `lib.rs` を `OpenApiRouter` ベースに書き換える途中で、**`mod auth;` 等のモジュール宣言と `build_cors_layer` の定義が失われていた** | 復元してから `#[utoipa::path]` の付与に着手 |
| Swagger UI の各行にパスが重複表示される | doc コメントの1行目が summary として使われるが、そこに `POST /auth/login` のようなパス自体を書いていた | 意味のある説明文に変更 |

### 教訓
- ファイルの一部が消える事故が**また起きた**(`docs/task-05`, `task-08`, `task-10`, `task-22` と同じパターン)。今回は `cargo build` が通らないため即座に気づけた
- **utoipa は doc コメントを「空行より前 = summary、空行より後 = description」として解釈する。** 2行あったコメントは空行を挟んで分離し、1行だけのものは summary を書き直した

## 仕様とコードの乖離を防ぐテスト
`tests/openapi_test.rs` の5件で、ドキュメントの陳腐化を仕組みで防いでいる。

| テスト | 検出できる問題 |
|---|---|
| `openapi_json_is_served` | 仕様の配信自体が壊れた |
| `openapi_covers_all_rest_routes` | **エンドポイントを追加して `#[utoipa::path]` を書き忘れた** |
| `openapi_declares_bearer_auth` | セキュリティスキームの定義が消えた |
| `protected_endpoints_require_bearer_auth` | **認証必須のエンドポイントに `security` を書き忘れた** |
| `docs_page_is_served` | Swagger UI のページが壊れた |

`openapi_covers_all_rest_routes` は失敗時に現在のパス一覧を出力するので、増減の内容がすぐ分かる。

```rust
assert_eq!(
    paths.len(),
    EXPECTED_PATH_COUNT,
    "RESTエンドポイントの数が変わっています。... 現在のパス: {:?}",
    paths.keys().collect::<Vec<_>>()
);
```

## 次タスクへの引き継ぎ
- 生成結果は `docs/openapi.json` にコミットしている。API を変更した際は再生成して差分をコミットすると、PR のレビューで変更内容が見える
- テストは計47件(ユニット17 + 統合30)になった

## 再現コマンド
```bash
cd chess
cargo run
# ブラウザで http://localhost:3000/docs

# 仕様の確認
curl -s http://localhost:3000/openapi.json | python3 -m json.tool | head -40

# 仕様ファイルの再生成
cargo run &
sleep 3
curl -s http://localhost:3000/openapi.json | python3 -m json.tool > docs/openapi.json
kill %1

cargo test --test openapi_test
```