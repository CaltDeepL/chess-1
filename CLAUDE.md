# CLAUDE.md

AI エージェント（Claude Code・Cursor など）向けの作業ガイド。人向けの詳細は README.md を正とする。

## プロジェクト

WebSocket でリアルタイム対戦できるオンラインチェス。本番は Render（API・フロント）+ Neon（PostgreSQL）で稼働中。

- バックエンド: `chess/`（Rust / Axum / Tokio / SQLx / shakmaty）
- フロントエンド: `frontend/` はソースのみ。`package.json`・`vite.config.ts` はリポジトリ直下（React / TypeScript / Vite）
- Rust のバージョンは `chess/rust-toolchain.toml` で一元管理（CI・Docker 共通）

## よく使うコマンド

```bash
# バックエンド（chess/ で実行）
cd chess
docker compose up -d db                     # 統合テスト用の PostgreSQL（ホスト 5434）
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
docker compose up --build -d && curl -fsS http://localhost:3000/health

# フロントエンド（リポジトリ直下で実行）
npm ci
npx tsc -b --force
npm run lint
npm run build
```

ローカルのポート: API `3000`、PostgreSQL `5434`（ホスト）/ `5432`（コンテナ内）。
ホストの 5432 / 5433 は他プロジェクトが使う。`.env` のポートがずれると別プロジェクトの DB に接続し「パスワード認証エラー」に見える。

## 守ること

- エラー応答は RFC 9457（Problem Details）。内部エラーの詳細を返さない
- ドメインロジックは `chess/src/domain/` の純粋関数に置き、ユニットテストする
- マイグレーションのファイル名はタイムスタンプ形式（`YYYYMMDDHHMMSS_説明.sql`）。連番（`0001_`）を混ぜない
- Rust を上げるときは `rust-toolchain.toml` と `chess/Dockerfile` の `FROM` を同時に変える
- Docker を変えたら `docker compose up -d` と `/health` の疎通まで確認する（ビルド成功だけでは不十分）
- HTTP ステータスだけでなく、DB の状態までテストで確認する
- advisory lock はプールではなく専用の接続で取得・解放する

## 作業の進め方

- タスクは GitHub Issue（ラベル `task`、テンプレートは [CaltDeepL/.github](https://github.com/CaltDeepL/.github)）で管理し、作業メモもその Issue に書く（2026-10-24 以降）
- 過去の作業メモ `chess/docs/Task_*.md` は記録として残す。新しく追加しない
- ブランチ → PR → CI → squash マージ。`main` へ直接 push しない
- コミットは Conventional Commits。PR 本文に `Closes #NN` を書く

## やってはいけないこと

- `.env` や秘密情報（JWT_SECRET・SWEEP_TOKEN・Deploy Hook URL）をコミット・出力しない
- `main` の Ruleset を変更しない、force push しない
- 確認なしに本番へのデプロイ、Issue・PR の作成、GitHub の設定変更をしない
