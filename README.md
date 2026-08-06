# Chess Backend

学習用に構築しているオンライン対戦チェスアプリのバックエンドです。
Rust + Axum でAPIサーバーを実装し、[shakmaty](https://github.com/niklasf/shakmaty) でチェスのルール判定を行います。

## 技術スタック

| 領域 | 技術 |
|---|---|
| 言語 | Rust |
| Webフレームワーク | [axum](https://github.com/tokio-rs/axum)(WebSocket含む) |
| 非同期ランタイム | [tokio](https://tokio.rs/) |
| チェスルールエンジン | [shakmaty](https://github.com/niklasf/shakmaty) |
| シリアライズ | serde / serde_json |
| ID生成 | uuid |
| ミドルウェア | tower-http (CORS, ログ) |
| ログ | tracing / tracing-subscriber |
| 永続化 | PostgreSQL + sqlx |
| 認証 | ユーザー登録・ログインベース、JWT(jsonwebtoken) |
| パスワードハッシュ | argon2 |
| フロントエンド | React (Vite) *(別リポジトリ/別ディレクトリで構築予定)* |

## ディレクトリ構成

```
chess/
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── docker-compose.yml
├── migrations/
│   └── 20260805202110_init.sql   # users / games / moves テーブル
└── src/
    ├── main.rs                    # エントリーポイント(env読み込み、DB接続、Router組み立て)
    ├── state.rs                   # AppState、対局ごとのWebSocketブロードキャストチャンネル、GameEvent
    ├── models.rs                  # リクエスト/レスポンス型、DB行の型
    ├── auth.rs                    # register/login/JWT発行・検証、extract_user_id
    └── routes/
        ├── mod.rs
        ├── health.rs              # 疎通確認用
        ├── game.rs                # 対局作成・参加・投了・指し手のHTTP API
        └── ws.rs                  # WebSocketエンドポイント(対局のリアルタイム配信)
```

## 実装状況

### 完了

- [x] Axumサーバー(`/health`エンドポイント)
- [x] shakmatyによるチェスルール判定の組み込み
- [x] 対局セッションの共有状態管理(`Arc<RwLock<HashMap<Uuid, Chess>>>`)
- [x] モジュール分割(`main.rs` / `state.rs` / `models.rs` / `auth.rs` / `routes/*`)
- [x] ユーザー登録・ログイン(`POST /auth/register` / `POST /auth/login`、JWT発行、パスワードはargon2でハッシュ化)
- [x] PostgreSQL永続化(sqlx導入、`users` / `games` / `moves` テーブル)
- [x] 対局作成 API(`POST /games`、JWT認証必須、`games`テーブルへINSERT)
- [x] 対局参加 API(`POST /games/:id/join`、対戦相手を`black_user_id`として登録)
- [x] 対局状態取得 API(`GET /games/:id`)
- [x] 指し手適用 API(`POST /games/:id/move`) — 参加者本人・手番チェックのうえ、UCI形式の指し手を検証して盤面を更新し、`moves`テーブルへ記録
- [x] 投了エンドポイント(`POST /games/:id/resign`)
- [x] 終局判定(チェックメイト・ステイルメイト・駒不足)時の`games`テーブル更新
- [x] WebSocketエンドポイント(`GET /ws/games/:id`、接続後の最初のメッセージでトークン認証 → 参加者確認 → 指し手/投了/終局をリアルタイム配信)

### 未着手

- [ ] フロントエンド(React)との接続
- [ ] 対局一覧・履歴の閲覧API
- [ ] レーティング機能

## 現在のAPIエンドポイント

| メソッド | パス | 認証 | 説明 |
|---|---|---|---|
| `GET` | `/health` | 不要 | 疎通確認。`{"status":"ok"}` を返す |
| `POST` | `/auth/register` | 不要 | ユーザー登録。`{username, password}` → `{user_id, token}` |
| `POST` | `/auth/login` | 不要 | ログイン。`{username, password}` → `{user_id, token}` |
| `POST` | `/games` | 必須 | 新規対局を作成(自分が白番)。対局IDと初期局面(FEN)を返す |
| `GET` | `/games/:id` | 不要 | 対局の現在の盤面(FEN)・チェック状態・終局判定を取得 |
| `POST` | `/games/:id/join` | 必須 | 対戦相手(黒番)として対局に参加 |
| `POST` | `/games/:id/move` | 必須 | UCI形式(例: `"e2e4"`)の指し手を送信し、盤面を更新。参加者本人かつ手番が合っている場合のみ受け付ける |
| `POST` | `/games/:id/resign` | 必須 | 投了。相手の勝ちとして対局を終了する |
| `GET` | `/ws/games/:id` | 必須(WS) | WebSocket接続。接続後の最初のメッセージで認証し、以後その対局の更新をリアルタイム受信 |

認証が必須のHTTPエンドポイントは `Authorization: Bearer <token>` ヘッダーで、WebSocketは接続後の最初のメッセージ `{"token": "..."}` でJWTを送る。

### リクエスト例

```bash
# ユーザー登録
curl -X POST http://localhost:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "password123"}'

# 対局作成(登録時に返るtokenを使用)
curl -X POST http://localhost:3000/games \
  -H "Authorization: Bearer <token>"

# 対局参加(別ユーザーのtokenを使用)
curl -X POST http://localhost:3000/games/{game_id}/join \
  -H "Authorization: Bearer <token>"

# 指し手を送信
curl -X POST http://localhost:3000/games/{game_id}/move \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"uci": "e2e4"}'

# 投了
curl -X POST http://localhost:3000/games/{game_id}/resign \
  -H "Authorization: Bearer <token>"
```

WebSocketは接続後にまず認証メッセージを送る。

```json
{"token": "<token>"}
```

以後、指し手・終局イベントが順次届く。

```json
{"type":"move","fen":"...","uci":"e2e4","is_check":false,"is_game_over":false}
{"type":"game_over","result":"black_win","end_reason":"resignation"}
```

## データベース設計

[migrations/20260805202110_init.sql](chess/migrations/20260805202110_init.sql) で以下のスキーマを定義している。

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TYPE game_status AS ENUM ('waiting', 'in_progress', 'finished');
CREATE TYPE game_result AS ENUM ('white_win', 'black_win', 'draw');

CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    white_user_id UUID NOT NULL REFERENCES users(id),
    black_user_id UUID REFERENCES users(id),
    fen TEXT NOT NULL DEFAULT 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
    status game_status NOT NULL DEFAULT 'waiting',
    result game_result,
    end_reason TEXT, -- 'checkmate' | 'resignation' | 'stalemate' など
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE moves (
    id BIGSERIAL PRIMARY KEY,
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    move_number INTEGER NOT NULL,
    uci TEXT NOT NULL,
    fen_after TEXT NOT NULL,
    played_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_moves_game_id ON moves(game_id);
```

### 設計方針

- **プレイヤー識別**: ユーザー登録・ログインベース(JWT認証)を採用。対局作成者が `white_user_id`、参加者が `black_user_id` に紐付く
- **WebSocket認証**: ブラウザのWebSocket APIは任意ヘッダーを送れないため、接続後の最初のメッセージでトークンを送信して認証する方式を採用
- **引き分け**: shakmaty側のルール判定(ステイルメイト、駒不足など)による結果は `game_result.draw` として残す。一方でプレイヤー同士が対話的に合意する「引き分け提案」機能は実装しない
- **投了**: `POST /games/:id/resign` を用意し、対局の当事者のみが呼び出せるようにする。投了は `moves` テーブルには記録せず、`games.end_reason` に理由(`resignation`)を残す
- **リアルタイム配信**: 対局ごとに `tokio::sync::broadcast` チャンネルを持ち、`make_move` / `resign_game` が指し手・投了・終局のたびにイベントを配信する

## セットアップ

### Docker Composeで一式起動する場合

```bash
docker compose up -d --build
sqlx migrate run
```

`db`(PostgreSQL、ホスト側 `5433` 番)と `app`(`3000` 番)が起動する。

### ローカルで `cargo run` する場合

```bash
# DBだけDockerで起動
docker compose up -d db

# .envのDATABASE_URLがホスト公開ポート(5433)を指していることを確認
cat .env

# マイグレーション適用(初回のみ)
sqlx migrate run

cargo run
```

デフォルトで `0.0.0.0:3000` で起動する。ログレベルは `RUST_LOG` 環境変数で制御できる(未指定時は `chess_server=debug,tower_http=debug`)。`JWT_SECRET` は未設定の場合、開発用の固定値にフォールバックする(本番では必ず設定する)。

## 開発環境メモ

- ローカル開発環境(Mac)は Rust 1.96 を使用
- DockerビルドイメージはRust 1.90以上が必要(依存クレートが`edition2024`を要求するため)
