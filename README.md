# Chess App

学習・練習用のオンライン対戦チェスアプリ。Rust(Axum)製バックエンドとReact(SPA)製フロントエンドで、WebSocketによるリアルタイム対局を実現しています。

## 特徴

- ユーザー登録・ログイン(JWT認証)
- 対局の作成・参加・一覧表示(ロビー、タイル表示・自動更新)
- リアルタイム対局(WebSocketで指し手・投了・対局終了・相手参加を即時通知)
- 合法手判定はshakmatyクレートに一任、フロントでも移動可能マスをハイライト表示
- 投了、プロモーション(歩の昇格)、棋譜(指し手履歴)のサイドバー表示
- ガラス調(グラスモルフィズム)デザイン、ダークモード、スマホ対応
- 接続断・再接続の可視化(LEDインジケーター)と自動再接続、セッション切れの自動検知

## 技術スタック

### バックエンド
- Rust + Axum + tokio
- [shakmaty](https://github.com/niklasf/shakmaty) — チェスルール・合法手・チェックメイト判定
- sqlx + PostgreSQL — 対局・ユーザー・棋譜の永続化
- argon2 — パスワードハッシュ化
- jsonwebtoken — JWT発行・検証
- Docker / docker-compose

### フロントエンド
- React 19 + Vite + TypeScript
- react-router-dom — ルーティング
- react-chessboard v5 — 盤面UI(ガラス調のカスタム駒セット)
- 素の`fetch`ラッパー(APIクライアント自作)

## ディレクトリ構成

```
chess/                  # バックエンド(Rust)
├── src/
│   ├── main.rs          # 起動処理
│   ├── state.rs          # AppState、GameEvent型
│   ├── models.rs          # リクエスト/レスポンス型
│   ├── auth.rs            # 認証ロジック
│   └── routes/
│       ├── health.rs
│       ├── game.rs        # 対局関連ハンドラ
│       ├── user.rs        # ユーザー公開情報
│       └── ws.rs          # WebSocketハンドラ
├── migrations/
└── docker-compose.yml

vite-project/            # フロントエンド(React)
└── src/
    ├── api/               # client.ts / auth.ts / games.ts / users.ts
    ├── context/            # AuthContext / ToastContext
    ├── hooks/              # useGameSocket
    ├── pages/              # Home / Login / Register / Lobby / Game
    ├── components/         # ChessBoard / GameList / MoveHistory / GameMenu / 他
    └── styles/             # global.css / glass-board.css
```

## セットアップ

### 前提
- Docker / Docker Desktop
- Node.js
- Rust(ローカルでマイグレーションを実行する場合)

### 1. バックエンド起動

```bash
cd chess
cp .env.example .env   # DATABASE_URL / JWT_SECRET を設定
docker compose up -d --build
```

### 2. マイグレーション適用

```bash
# ホストから、.envのDATABASE_URL(ポート5433)経由で実行
sqlx migrate run
```

### 3. フロントエンド起動

```bash
cd vite-project
npm install
npm run dev
```

`http://localhost:5174`(Viteのデフォルトから変更している場合あり)でアクセスできます。

## API一覧

| メソッド | パス | 認証 | 概要 |
|---|---|---|---|
| GET | `/health` | 不要 | 疎通確認 |
| POST | `/auth/register` | 不要 | ユーザー登録 |
| POST | `/auth/login` | 不要 | ログイン、JWT発行 |
| GET | `/users/:id` | 必須 | ユーザー公開情報(id/username)取得 |
| POST | `/games` | 必須 | 対局作成 |
| GET | `/games` | 必須 | 対局一覧(`status`で絞り込み) |
| GET | `/games/:id` | 必須 | 対局詳細取得 |
| GET | `/games/:id/moves` | 必須 | 棋譜(指し手履歴)取得 |
| POST | `/games/:id/join` | 必須 | 対局参加 |
| POST | `/games/:id/move` | 必須 | 指し手送信(UCI形式) |
| POST | `/games/:id/resign` | 必須 | 投了 |
| GET | `/ws/games/:id` | 接続後メッセージで認証 | WebSocket、リアルタイム対局通知 |

WebSocketは接続後、最初のメッセージで`{"token": "..."}`を送信して認証します(クエリパラメータ方式は不採用)。配信されるイベントは`Move` / `GameOver` / `OpponentJoined`の3種類です。

## データベーススキーマ

`users` / `games` / `moves` の3テーブル構成。`games.status`(`waiting`/`in_progress`/`finished`)と`games.result`(`white_win`/`black_win`/`draw`)はPostgreSQLのENUM型です。詳細は`chess/migrations/`を参照してください。

## 実装状況

### バックエンド
- [x] ユーザー登録・ログイン(Argon2 + JWT)
- [x] 対局作成・一覧・詳細取得
- [x] 対局参加、指し手送信、投了
- [x] 棋譜(moves)のDB記録・取得
- [x] 終局判定・結果のDB反映
- [x] WebSocketによるリアルタイム通知(指し手・投了・終局・相手参加)
- [x] ユーザー公開情報エンドポイント
- [x] モジュール分割済みのコード構成

### フロントエンド
- [x] 認証(ログイン・新規登録)
- [x] ロビー(タイル表示・自動更新・作成・参加)
- [x] 対局画面(盤面・指し手送信・投了・手番表示)
- [x] 合法手ハイライト表示
- [x] プロモーション(歩の昇格)対応
- [x] 棋譜サイドバー
- [x] 接続状態のリアルタイム表示(LED)・自動再接続
- [x] 対戦相手名の表示・参加通知トースト
- [x] 勝敗決定時のオーバーレイ表示
- [x] エラーハンドリング(ネットワーク断の正規化、セッション切れの自動検知・ログアウト、ErrorBoundary)
- [x] ガラス調デザイン・ダークモード・スマホ対応

### 今後の課題
- [ ] 本番デプロイ(構成案: バックエンド+フロントをRender、DBをNeonの無料枠で運用)
- [ ] 2ブラウザでの通しE2Eの継続的な確認体制
- [ ] 棋譜のSAN(標準代数記法)表示への対応(現状UCI表記)

## 開発上の教訓

開発を通じて繰り返し発生したバグには大きく2つの傾向があります。

1. **ファイル内容の誤混入・保存漏れ**: 編集中に関数定義そのものが消える、別ファイル用のコードが誤って書き込まれる、といった構文崩れ。`tsc`/`cargo build`や実際のブラウザ確認でのみ発見できるケースが多い
2. **API関数の引数順序の取り違え**: 特に全引数が`string`型の場合、型チェックをすり抜けて実行時バグとして顕在化する

いずれも「実際にビルドし、ブラウザで動かして確認する」ことが発見の決め手になっています。

## ライセンス

学習目的の個人プロジェクトです。