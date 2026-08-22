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

# chess-app-2 開発ドキュメント

オンライン対戦チェスアプリの開発記録を、タスク単位で記録したものです。各ドキュメントは以下の構成で統一しています。

- **ゴールと完了条件**
- **設計判断の根拠**
- **つまずいた点と教訓**
- **次タスクへの引き継ぎ**
- **再現コマンド**

## タスク一覧

### 設計・バックエンド基盤
| # | ドキュメント | 概要 |
|---|---|---|
| 01 | [設計・技術選定](task-01-設計・技術選定.md) | 実装範囲の確定、shakmaty採用、JWT認証・WS認証方式の決定 |
| 02 | [最小サーバーとチェスロジック統合](task-02-最小サーバーとチェスロジック統合.md) | `/health` から対局API4本まで |
| 03 | [DB設計とDocker環境構築](task-03-DB設計とDocker環境構築.md) | スキーマ確定、ポート競合・接続先誤りの解決 |
| 04 | [マイグレーション実行](task-04-マイグレーション実行.md) | ホストから `sqlx migrate run` する運用の確立 |

### バックエンド機能実装
| # | ドキュメント | 概要 |
|---|---|---|
| 05 | [認証実装](task-05-認証実装.md) | Argon2 + JWT、ユーザー列挙攻撃対策 |
| 06 | [対局API](task-06-対局API.md) | 作成・参加・指し手・状態取得、レースコンディション対策 |
| 07 | [投了エンドポイント](task-07-投了エンドポイント.md) | ENUMキャスト漏れによるサイレント障害の発見 |
| 08 | [WebSocketエンドポイント](task-08-WebSocketエンドポイント.md) | broadcastチャンネル、接続後トークン認証 |
| 09 | [モジュール分割リファクタリング](task-09-モジュール分割リファクタリング.md) | main.rs単一構成からの分割 |

### フロントエンド実装
| # | ドキュメント | 概要 |
|---|---|---|
| 10 | [フロントエンド設計と土台実装](task-10-フロントエンド設計と土台実装.md) | 画面構成・状態管理方針・型定義・ルーティング |
| 11 | [対局一覧エンドポイント追加](task-11-対局一覧エンドポイント追加.md) | `GET /games` の追加 |
| 12 | [認証画面](task-12-認証画面.md) | ログイン・新規登録フォーム |
| 13 | [ロビー画面](task-13-ロビー画面.md) | 一覧・作成・参加。引数順序バグの初出 |
| 14 | [useGameSocketフック](task-14-useGameSocketフック.md) | 分離設計の判断、指数バックオフ再接続 |
| 15 | [ChessBoardコンポーネント](task-15-ChessBoardコンポーネント.md) | react-chessboard v5 API への対応 |
| 16 | [GamePage本体](task-16-GamePage本体.md) | 引数順序バグ3件、GameDetailResponse新設 |

### UI・UX・品質
| # | ドキュメント | 概要 |
|---|---|---|
| 17 | [ガラス調デザインへの統一](task-17-ガラス調デザインへの統一.md) | デザインシステムの確立、HomePage追加 |
| 18 | [対局画面UI強化](task-18-対局画面UI強化.md) | LED・メニュー・トースト・オーバーレイ、cqw肥大化バグ |
| 19 | [合法手ハイライトとスマホ対応](task-19-合法手ハイライトとスマホ対応.md) | viewportタグ位置による白画面バグ |
| 20 | [プロモーションバグの解決](task-20-プロモーションバグの解決.md) | 再発したバグへの構造的アプローチ |
| 21 | [ロビーのタイル化と自動更新](task-21-ロビーのタイル化と自動更新.md) | タイルグリッド、visibilitychange対応ポーリング |
| 22 | [棋譜サイドバー](task-22-棋譜サイドバー.md) | UCI→SAN変換表示 |
| 23 | [エラー系の作り込み](task-23-エラー系の作り込み.md) | fetch失敗の正規化、401の全体検知、ErrorBoundary |

### デプロイ
| # | ドキュメント | 概要 |
|---|---|---|
| 24 | [本番デプロイ](task-24-本番デプロイ.md) | Render + Neon、6件のトラブルシューティング |

## プロジェクト全体を通じた教訓

繰り返し発生したバグは4つのパターンに整理できます。

### 1. ファイル内容の誤混入・保存漏れ(最頻出)
関数定義が丸ごと消えて呼び出し側だけ残る、別ファイル用の内容が書き込まれる、といった事故が全工程で発生しました。

| 発生箇所 | 内容 |
|---|---|
| task-05 | `verify_token` の定義が消失 |
| task-06 | `position_to_fen` の定義が消失(呼び出し3箇所は残存) |
| task-08 | `state.rs` にWSハンドラのコードが書き込まれ `AppState` が消失 |
| task-10 | `main.tsx` に4ページ分のコンポーネントが書き込まれエントリーポイントが消失 |
| task-18 | `GlassPieces.tsx` が空で重複ファイルに実装、`ConnectionLED.tsx` に `export default` が2つ |
| task-22 | `moves` state と JSX が誤ったスコープに配置 |

**対策**: 編集後は必ず `cargo build` / `tsc -b` を通す。`cannot find function` はタイプミスより先に**定義の消失**を疑う。

### 2. API関数の引数順序の取り違え
`token` と `id` の位置が逆になるバグが**4関数すべてで発生**しました(`getGame` / `makeMove` / `resignGame` / `joinGame`)。全引数が `string` 型のため `tsc` をすり抜け、ブラウザで実行して初めて発覚します。

**対策**: 意味の異なる文字列は newtype で型を分けるか、オブジェクト引数にして順序依存をなくす。

### 3. 型システムがカバーしない境界
- PostgreSQLのENUM型 ↔ Rustの `String`(`sqlx::query` は動的クエリのため実行時に判明。SELECT/バインドの**両方向**でキャストが必要)
- TypeScriptの `verbatimModuleSyntax`(型のみのシンボルは `import type` が必須。**3回踏んだ**)
- CSSの `cqw`(コンテナクエリ単位)の基準コンテナ。portalされる要素は**DOMツリー上の位置とReactツリー上の位置が一致しない**

### 4. 「動いているように見える」ことの罠
- `if let Err(e) = ... { tracing::error!(...) }` の**ログにしか出ないエラー**は、APIレスポンスが200でもDB反映が失敗しているサイレント障害を生む(task-07)
- フロントで握りつぶされたエラーは**無限ローディング**としてユーザーに現れる(task-19)
- 本番環境特有の問題(環境変数、ビルド設定、ブランチ管理)はローカル開発では気づけない(task-24)

**総括**: 型チェックとビルドが通ることは必要条件でしかありません。このプロジェクトで発見できたバグの大半は、**実際にブラウザで動かし、DBの中身を確認し、DevToolsでネットワークとDOMを見た**ことによるものでした。

## ライセンス

学習目的の個人プロジェクトです。