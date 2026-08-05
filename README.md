# Chess App

Rustのバックエンド開発練習として作成した、オンライン対戦可能なチェスWebアプリです。

## プロジェクト概要

Rustでのサーバーサイド開発を学ぶことを目的に開発しました。特に以下の習得を目指しています。

- Axumを使ったWeb APIサーバーの構築
- WebSocketによるリアルタイム通信の実装(オンライン対戦)
- PostgreSQL + sqlxを使ったデータ永続化
- フロントエンド(React)とバックエンドを分離した構成での開発

## 使用技術

### バックエンド
- Rust
- Axum
- sqlx
- PostgreSQL
- WebSocket (tokio-tungstenite など)
- shakmaty (チェスのルール・盤面管理)
- docker

### フロントエンド
- React
- Vite
- HTML / CSS

## セットアップ手順

### 前提条件
- Rust (cargo)
- Node.js / npm
- PostgreSQL

### バックエンド

```bash
# .envを用意し、DATABASE_URLなどを設定
cp .env.example .env

# DBマイグレーション実行
sqlx migrate run

# サーバー起動
cargo run
```

### フロントエンド

```bash
cd frontend
npm install
npm run dev
```

## ディレクトリ構成

```
.
├── src/                # Rust(Axum)バックエンドのソースコード
│   ├── main.rs
│   ├── routes/          # APIルーティング
│   ├── models/          # DBモデル定義
│   └── ws/               # WebSocket関連処理
├── migrations/       # sqlxマイグレーションファイル
├── frontend/           # React(Vite)フロントエンド
│   ├── src/
│   └── public/
├── Cargo.toml
└── README.md
```

※ 実際の構成に合わせて適宜書き換えてください。

## 今後の実装予定(TODO)

- [ ] チェスの基本ルール実装(合法手判定、詰み判定)
- [ ] WebSocketによるオンライン対戦機能
- [ ] 対局履歴の保存・閲覧機能
- [ ] ユーザー認証機能
- [ ] レーティング機能
- [ ] 観戦モード
- [ ] レスポンシブ対応(モバイル表示)
- [ ] テストコードの整備(単体・結合テスト)
- [ ] CI/CDパイプラインの構築