# task-29 エラーレスポンスの RFC 9457 化

## ゴールと完了条件
- エラーレスポンスを RFC 9457 (Problem Details for HTTP APIs) 準拠の形式に統一する
- OpenAPI 仕様にもエラー形式を記載し、Swagger UI から確認できるようにする
- 完了条件: バックエンド・OpenAPI・フロントの3層が揃い、`cargo test` 全51件が通ること

## 変更前後

```
# 変更前
HTTP/1.1 400 Bad Request
content-type: text/plain

ユーザー名は必須、パスワードは8文字以上にしてください

# 変更後
HTTP/1.1 400 Bad Request
content-type: application/problem+json

{
  "type": "/problems/bad-request",
  "title": "Bad request",
  "status": 400,
  "detail": "ユーザー名は必須、パスワードは8文字以上にしてください"
}
```

## 実装

| 層 | 内容 |
|---|---|
| `src/errors.rs` | `AppError` の `IntoResponse` を差し替え。`ProblemDetails` 構造体を追加 |
| `src/openapi.rs` | `components(schemas(ProblemDetails))` に登録 |
| `src/auth.rs` / `routes/user.rs` / `routes/game.rs` | 全4xx/5xx **15箇所**に `body = ProblemDetails, content_type = "application/problem+json"` |
| `src/lib.rs` | `build_router` から `openapi_router()` を切り出し、`openapi_spec()` を追加 |
| `frontend/src/api/client.ts` | `body.detail` を優先。`ApiError` に `type` を追加 |
| `tests/problem_details_test.rs` | 3件（Content-Type・`type`・`status` を assert） |
| `tests/openapi_test.rs` | 1件追加（全4xx/5xx が `ProblemDetails` を参照しているか静的検証） |

**ハンドラのコードには一切手を入れていない。** `AppError` への移行（`(StatusCode, String)` からの脱却）は task-26 以前に済んでいたため、今回は `IntoResponse` の出力形式を変えるだけで全エンドポイントに反映された。

## 設計判断の根拠

### なぜ独自形式ではなく RFC 9457 か
独自の `{"message": "..."}` でも動作はする。標準に乗る利点は、**`Content-Type` を見るだけでクライアントが「これは構造化されたエラーだ」と判断できる**こと。`application/problem+json` は「本文が Problem Details である」という契約そのものなので、将来別のクライアント（CLI、他サービス）が繋がっても解釈方法を個別に伝える必要がない。

### `type` を変種単位にした理由
`/problems/not-your-turn` のような細かいコードにするには、`AppError` の全変種に `code` フィールドを足し、全呼び出し箇所を書き換える必要がある。

まず `/problems/forbidden` のような**変種単位**でRFC準拠にしておき、**フロントが実際に分岐したくなった箇所だけ後から細分化する**方針にした。`type` は追加できる設計なので、粗い状態から始めても後戻りにならない。

### `instance` を省略した理由
リクエストパスを取得するにはミドルウェア層で情報を渡す必要があり、`IntoResponse` だけでは完結しない。RFC 9457 でも `instance` は任意メンバーなので、必要になるまで入れない判断とした。

### `openapi_spec()` を `build_router` から切り出した理由
テスト用に OpenAPI 仕様を組み立て直すと、**「テストは緑だが実際に配信される仕様は別物」**という状態がありえる。配信ルートと同じ組み立てから `openapi_spec()` を切り出して定義元を一本化することで、テストが実物を検証している状態を保った。

### エラーレスポンスの `body` 付け忘れを静的に検知する
15箇所への `body` 追加は完全に手作業なので、書き漏れが出やすい。仕様JSONを走査して「全4xx/5xxが `#/components/schemas/ProblemDetails` を参照しているか」を検証するテストを**先に書いて赤くしてから**埋めていくことで、残りがリストで表示される状態にした。

## 副産物: 500 の情報漏れが塞がった

変更前は `AppError::Internal(format!("DBエラー: {e}"))` の中身が**そのままクライアントに返っていた**。DBのテーブル名やSQLの断片が外部に出うる状態だったことになる。

Problem Details 化にあたり、`Internal` だけは `public_detail()` で固定文言に差し替え、原因は `tracing::error!` でログにのみ残す形にした。**形式を整える作業のついでに、セキュリティ上の問題が1つ解消された。**

## つまずいた点と教訓

### `Json` のヘッダ上書きに依存している
`axum::Json` は `application/json` を付ける。これを上書きするために以下の形にしている。

```rust
(
    status,
    [(header::CONTENT_TYPE, "application/problem+json")],
    Json(body),
)
    .into_response()
```

タプルの後段が、内側のレスポンスのヘッダを上書きする挙動に依存している。**この依存関係はコードを読んでも自明ではない**ため、`problem_details_test.rs` で `Content-Type` を明示的に assert している。上書きが外れても本文自体は返るので、テストが無ければ壊れたことに気づけない。

### `tests/common/mod.rs` の意図しない置き換え（同種パターン3回目）
`post_json_raw` を「追加」したつもりが、既存の `post_json` を**置換**していた。`register_user`（common自身）・`auth_test.rs`・`problem_details_test.rs` が `post_json` を呼んでいたため、コンパイルエラーで即座に発覚。

| # | タスク | 症状 |
|---|---|---|
| 1 | 序盤 | `state.rs` の内容が丸ごと別ファイルの内容に置き換わる |
| 2 | task-22 | `<MoveHistory>` の JSX が `handlePieceDrop` 関数の中に紛れ込む |
| 3 | task-28 | 「ヘルパーを追加」が既存関数の置換になる |

**教訓**: 追加のつもりの編集が追加になっているかは、差分で確認する。今回はコンパイルエラーで済んだが、#2 は「コンポーネントは存在するのに画面に出ない」という分かりにくい形で現れた。

## 次タスクへの引き継ぎ
- `type` は変種単位のまま。フロントが `type` で分岐したくなった時点で細分化する
- `instance` は未実装。リクエストパスを載せるならミドルウェア層での対応が必要
- WebSocket（`/ws/games/:id`）の認証失敗はHTTPボディを返す経路ではないため、今回の統一の対象外

## 再現コマンド
```bash
cd chess
docker compose up -d db
cargo test                              # 全51件（unittests 17 + 統合 34）
cargo test --test problem_details_test  # 形式の検証のみ
cargo clippy --all-targets -- -D warnings

# 実物の確認
cargo run &
curl -i -X POST localhost:3000/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username":"a","password":"short"}'
curl -s localhost:3000/openapi.json | jq '.components.schemas.ProblemDetails'
```