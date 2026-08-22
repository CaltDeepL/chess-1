# task-03 DB設計とDocker環境構築

## ゴールと完了条件
- PostgreSQLのスキーマ(users / games / moves)を確定する
- docker-composeでDBとアプリを起動できる
- 完了条件: `docker compose up -d --build` でAPIが起動し、ホストからDBに接続できること

## 確定したスキーマ
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
    end_reason TEXT,
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

## 設計判断の根拠
- **PostgreSQL + sqlx**: 構造化データに適し、コンパイル時SQLチェック(`query!` マクロ)が使える。Rustエコシステムとの相性も良い
- **進行中の対局はメモリ、確定情報のみDB**: 1手ごとに局面をDBへ書き戻すと、対局中のレイテンシがDBに支配される。棋譜(`moves`)は追記のみなので永続化する
- **`status` / `result` はPostgreSQLのENUM型**: 不正な値をDBレベルで弾ける。ただし後の task で Rust の文字列型とのキャスト漏れが繰り返し問題になる(task-06 参照)

## つまずいた点と教訓
| 症状 | 原因 | 対応 |
|---|---|---|
| `Bind for 0.0.0.0:5432 failed: port is already allocated` | 別プロジェクト(ポーカーアプリ)の `poker_postgres` がホストの5432番を専有していた | chess側のホスト公開ポートを **5433** に変更(コンテナ内部は5432のまま) |
| `cargo build --release` が `feature edition2024 is required` で失敗 | Dockerビルダーイメージが `rust:1.82` で、依存クレート `home` が要求するRustバージョンに届いていない | `rust:1.85` → 最終的に `rust:1.90` に更新 |
| `password authentication failed for user "chess"` | **真因は `.env` の `DATABASE_URL` がポート5432のままだったこと。** 無関係な `poker_postgres`(chessユーザーが存在しない)へ接続していた | `.env` のポートを5433に修正 |

- **教訓**: 「認証エラー」は必ずしも認証の問題ではない。当初は `pg_hba.conf` のIPフォールバック(scram-sha-256)を疑い、docker-compose内部ネットワークへの移行まで計画したが、実際は**接続先が違っていた**だけだった。エラーメッセージが指す層より一つ手前(どこに繋いでいるか)を先に確認するほうが早い

## 次タスクへの引き継ぎ
- マイグレーションファイル `chess/migrations/20260805202110_init.sql` は作成したが中身は空テンプレート。task-04 で記述する
- ローカルからDBへ接続する際のポートは **5433**(コンテナ内部は5432)

## 再現コマンド
```bash
docker compose up -d db
docker compose build app
docker compose up -d --build
docker compose ps
docker compose exec db psql -U chess -d chess_db -c '\dt'
```