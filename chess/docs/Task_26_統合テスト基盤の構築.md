# task-26 統合テスト基盤の構築

## ゴールと完了条件
- `tests/` からアプリのRouterを呼び出し、HTTPサーバーを起動せずにハンドラ〜リポジトリを通しで検証できる状態にする
- 認証系のテストを揃える
- 完了条件: `cargo test` で認証6件が通り、CIでも同じテストが走ること

## ステップ1: lib.rs への切り出し
`main.rs` に `Router::new()...` が直接書かれており `lib.rs` が存在しなかったため、統合テストから Router を参照できなかった。

```
src/lib.rs           # pub fn build_router(state: AppState) -> Router、build_cors_layer
src/main.rs          # AppStateの組み立てとサーバー起動のみの薄いエントリポイント
```

`mod` 宣言は `state` だけ `pub` にし(`main.rs` が `AppState` を構築するため)、`auth` / `errors` / `models` / `routes` は lib.rs 内部でのみ使うため非公開のまま。統合テストで `routes` 配下の型を直接参照したくなった時点で `pub` にすればよい。

## ステップ2: テストの実装
`tests/auth_test.rs` に6件。

| テスト | 検証内容 |
|---|---|
| `register_returns_token` | 200 と `token` / `user_id` の返却 |
| `register_duplicate_username_returns_409` | ユーザー名重複 |
| `register_short_password_is_rejected` | パスワード長のバリデーション |
| `login_succeeds_with_correct_password` | 正常ログイン |
| `login_with_wrong_password_returns_401` | パスワード誤り |
| `login_with_unknown_user_returns_401` | **存在しないユーザーもパスワード誤りと同じ401**(ユーザー列挙攻撃対策の検証) |

## 設計判断の根拠

### なぜ `#[sqlx::test]` を使うのか
テストごとに**独立した一時DB**を自動作成し、マイグレーションを適用して、終了後に破棄してくれる。

```rust
#[sqlx::test(migrations = "./migrations")]
async fn register_returns_token(pool: PgPool) { ... }
```

これにより、複数のテストが同じ `alice` というユーザー名を使っても干渉しない。テスト間の状態共有がないため、並列実行しても順序に依存した失敗が起きない。共有DBを使ってテストごとに `TRUNCATE` する方式に比べ、クリーンアップ漏れの心配がない。

### なぜ HTTPサーバーを起動せず `oneshot` を使うのか
`tower::ServiceExt::oneshot` で Router へ直接リクエストを投げる方式にした。

実際にサーバーを立てて `reqwest` で叩く方式に比べ、ポートの取り合いが起きず、起動待ちも不要。それでいてルーティング・ミドルウェア・ハンドラ・リポジトリ・DBまでを実際に通るため、統合テストとしての検証範囲は変わらない。

### なぜ `sqlx migrate run` のステップをCIに入れないのか
`#[sqlx::test(migrations = "./migrations")]` が**テストごとにマイグレーションを適用する**ため。CIで事前に流しても、テストが作る一時DBには関係がない。

## つまずいた点と教訓

| 症状 | 原因 | 対応 |
|---|---|---|
| 全6件が `password authentication failed for user "chess"` で失敗 | **chess用のDBコンテナが起動しておらず**、`.env` が指す5433番を別プロジェクト(`ops-hub-db-1`)が専有していた。chessユーザーが存在しないPostgresに接続していた | chess の compose 公開ポートを **5434** に変更し、`docker compose up -d db` |

### 教訓
これは **task-03 とまったく同じ構図の再発**。あのときは5432を `poker_postgres` が専有していたため5433へ逃がしたが、その5433も別プロジェクトに取られていた。

切り分けの手順は以下が速い。

```bash
docker compose ps                                        # 自分のDBが起動しているか
docker ps --format 'table {{.Names}}\t{{.Ports}}'        # そのポートを誰が使っているか
lsof -nP -iTCP:<port> -sTCP:LISTEN                        # ホスト側で待ち受けているプロセス
```

**「認証エラー」は必ずしも認証の問題ではない。**エラーメッセージが指す層より一つ手前(どこに繋いでいるか)を先に確認する。

### ローカルのポート割り当て(記録)
繰り返し衝突しているため、プロジェクトごとに固定して記録することにした。

| プロジェクト | Postgres | API |
|---|---|---|
| shisan-api | 5432 | 8080 |
| ops-hub | 5433 | 8081 |
| chess | **5434** | 3000 |

## CIへの反映
テストがDBを使うようになったため、`ci.yml` の backend ジョブに Postgres サービスを追加した。

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_USER: chess
      POSTGRES_PASSWORD: chess
      POSTGRES_DB: chess_db
    ports:
      - 5432:5432
    options: >-
      --health-cmd pg_isready
      --health-interval 10s
      --health-timeout 5s
      --health-retries 5

env:
  DATABASE_URL: postgres://chess:chess@localhost:5432/chess_db
```

CIコンテナ内は他プロジェクトと隔離されているため、**ポートは5432で問題ない**(ローカルの5434とは無関係)。CIの実行時間は約20秒から56秒に増えたが、Postgresの起動と実際のテスト実行を含む妥当な範囲。

## 次タスクへの引き継ぎ
テストヘルパー(`app(pool)` / `post_json` / `register_and_login`)を `tests/common/mod.rs` に切り出すと、対局APIのテストが書きやすくなる。

次に書く価値が高いのは、**過去に実際に踏んだバグを検出できる範囲**。

| 優先 | 対象 | 検出できるバグ |
|---|---|---|
| 1 | `resign`(正常・再投了409・投了後move404) | `::game_result` キャスト漏れによるサイレント障害(task-07) |
| 2 | 終局判定で `result` / `status` がDBに反映されるか | 同上。**APIが200を返すことだけでなくDBの中身をassertする** |
| 3 | `join`(自作400・重複409) | レースコンディション対策の退行 |
| 4 | `move`(第三者403・手番違反403) | 権限チェックの退行 |

## 再現コマンド
```bash
# DBを起動してからテスト
cd chess
docker compose up -d db
docker compose ps                 # STATUS が Up であることを確認
sqlx migrate run                  # 初回のみ
cargo test
cargo test --all-targets          # CIと同じオプション

# ポート衝突の切り分け
docker ps --format 'table {{.Names}}\t{{.Ports}}'
lsof -nP -iTCP:5434 -sTCP:LISTEN
```