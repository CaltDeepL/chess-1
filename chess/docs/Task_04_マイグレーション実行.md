# task-04 マイグレーション実行

## ゴールと完了条件
- task-03で確定したスキーマをマイグレーションとして適用する
- 完了条件: `users` / `games` / `moves` / `_sqlx_migrations` の4テーブルが存在すること

## 設計判断の根拠
- **マイグレーションはホスト(Mac)から直接実行する運用**: `sqlx-cli` は開発用ツールであり、アプリの実行イメージに含めない。appコンテナには意図的にインストールしていない
- `.env` の `DATABASE_URL`(ホストからの接続、ポート5433)を使う。docker-compose側の `db:5432` はコンテナ内部からの接続用で別物

## 実行環境の整理
```
[Mac本体(ホスト)]
  ├─ cargo, sqlx-cli がインストール済み
  ├─ .env の DATABASE_URL → localhost:5433 経由でDB接続
  └─ [Docker Desktop]
        ├─ dbコンテナ (chess-postgres)  ← ホストからは5433番
        │     内部的には5432番で待受
        └─ appコンテナ (chess-server)
              ├─ DATABASE_URL → db:5432 (内部ネットワーク)
              └─ sqlxコマンドは入っていない(意図的に未インストール)
```

## つまずいた点と教訓
| 症状 | 原因 | 対応 |
|---|---|---|
| マイグレーションが何も作らない | ファイルが `-- Add migration script here` のみの空テンプレートだった | 確定済みスキーマを書き込む |
| `docker compose exec app sqlx migrate run` が `executable file not found in $PATH` | appコンテナに `sqlx-cli` を入れていない | 方針どおりホストから実行する |

- **教訓**: 同じコマンド名でも、実行場所(ホスト/コンテナ)によって使うツールも接続経路も違う。「どこで実行しているか」を常に意識する

## 次タスクへの引き継ぎ
- スキーマが揃ったので task-05 で認証(users テーブルへの登録)に進める
- `sqlx-cli` は後の本番デプロイ時にTLS対応版へ入れ直しが必要になる(task-24 参照)

## 再現コマンド
```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx database create
sqlx migrate run
docker compose exec db psql -U chess -d chess_db -c '\dt'
```