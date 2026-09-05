# chess-app

[![CI](https://github.com/CaltDeepL/chess-1/actions/workflows/ci.yml/badge.svg)](https://github.com/CaltDeepL/chess-1/actions/workflows/ci.yml)

**Rust + WebSocket で実装した、オンライン対戦チェスアプリ。**

対局の合法手判定・手番管理・終局判定をサーバー側の権威として持ち、指し手・投了・終局・対戦相手の参加を WebSocket でリアルタイムに配信します。

**デモ**: https://chess-frontend-0van.onrender.com

`/register` からアカウントを作成し、ロビーで対局を作成すると、別のユーザーが参加した時点で対局が始まります。

> 無料プランで稼働しているため、アクセスがない間はインスタンスが停止します。最初のリクエストは応答まで数十秒かかることがあります。

> **ポートフォリオプロジェクトです。** 全26タスクを完了し、本番環境（Render + Neon）で稼働しています。CI が green のときだけデプロイが走る構成です。

---

## なぜ作ったか

オンラインのチェスアプリ自体は数多くありますが、「リアルタイム対戦を成立させるために何を設計する必要があるのか」を自分の手で確かめたいと考えました。特に次の3点が、作ってみないと判断できない領域でした。

**どこまでをサーバーの権威とするか。** クライアント側で合法手を判定すればレスポンスは速くなりますが、改ざんされた指し手を防げません。逆にすべてサーバーに問い合わせると、駒を掴んだ瞬間の「動かせるマス」の表示すら往復が必要になります。**権威と表示を分離する**設計が要ります。

**リアルタイム通信の状態をどう扱うか。** WebSocket は「繋がっている / 切れている」の二値ではありません。初回接続中・再接続中・切断・エラーを区別しないと、ユーザーには「なぜか操作できない」としか見えません。

**対局の状態をどこに置くか。** 進行中の局面を1手ごとに DB へ書き戻すと、対局中のレイテンシが DB に支配されます。一方で棋譜と対局結果は永続化しなければ意味がありません。**揮発してよいものと、してはいけないものの線引き**が必要です。

これらを扱うには、チェスのルール自体よりも「状態の所在と権威」をどう設計するかが主題になります。ルール判定は [shakmaty](https://github.com/niklasf/shakmaty) に任せ、**その周辺の設計にコストを払う**方針にしました。

---

## Features

| 機能 | エンドポイント |
|---|---|
| 認証（JWT） | `POST /auth/register` `POST /auth/login` |
| ユーザー公開情報 | `GET /users/{id}` |
| 対局の作成・一覧 | `POST /games` `GET /games` |
| 対局詳細 | `GET /games/{id}` |
| 対局への参加 | `POST /games/{id}/join` |
| 指し手（UCI 形式） | `POST /games/{id}/move` |
| 投了 | `POST /games/{id}/resign` |
| 棋譜（指し手履歴） | `GET /games/{id}/moves` |
| リアルタイム対局通知 | `GET /ws/games/{id}` |
| 疎通確認 | `GET /health` |

フロントエンドは、ロビーのタイル表示と自動更新、合法手ハイライト、プロモーション（歩の昇格）、棋譜の SAN 表示、接続状態の LED インジケーター、勝敗オーバーレイ、スマホ対応を実装しています。

---

## Architecture

```
        React SPA（Render Static Site）
           │  REST / WebSocket
           ▼
        Rust + axum（Render, Docker）
        ├── routes/     HTTP・WS のエンドポイント
        ├── auth.rs     JWT 発行・検証
        ├── models.rs   リクエスト / レスポンス型・DB 行型
        ├── state.rs    AppState（対局のメモリ状態・broadcast チャンネル）
        └── shakmaty    合法手・チェックメイト判定（権威）
           │
           ▼
        PostgreSQL（Neon）
        users / games / moves
```

進行中の局面は `Arc<RwLock<HashMap<Uuid, Chess>>>` でメモリに、確定した情報（ユーザー・対局結果・棋譜）は PostgreSQL に置いています。

### ディレクトリ構成

```
chess-app/
├── chess/                    # バックエンド（Rust）
│   ├── Dockerfile            # マルチステージビルド
│   ├── compose.yaml          # Postgres + API
│   ├── .env                  # sqlx CLI 用の DATABASE_URL（gitignore 対象）
│   ├── migrations/           # sqlx マイグレーション
│   ├── docs/                 # タスクごとの設計メモ（全24件）
│   └── src/
│       ├── main.rs           # 起動処理・ルータ組み立て
│       ├── state.rs          # AppState / GameEvent
│       ├── models.rs         # 各種 DTO・DB 行型
│       ├── auth.rs           # register / login / JWT
│       └── routes/
│           ├── health.rs
│           ├── game.rs       # 対局関連ハンドラ
│           ├── user.rs       # ユーザー公開情報
│           └── ws.rs         # WebSocket ハンドラ
└── src/                      # フロントエンド（Vite + React + TypeScript）
    ├── api/                  # fetch ラッパ / エラー正規化
    ├── context/              # AuthContext / ToastContext
    ├── hooks/                # useGameSocket（接続管理・再接続）
    ├── pages/                # Home / Login / Register / Lobby / Game
    ├── components/           # ChessBoard / GameList / MoveHistory / 他
    └── styles/               # global.css / glass-board.css
```

---

## Domain Model

### 対局の状態（`game_status` ENUM）

| 値 | 意味 |
|---|---|
| `waiting` | 対局作成済み、対戦相手の参加待ち |
| `in_progress` | 対局中 |
| `finished` | 終了 |

### 対局結果（`game_result` ENUM）

| 値 | 意味 |
|---|---|
| `white_win` / `black_win` | 勝敗が決した |
| `draw` | ステイルメイト・駒不足など |

終了理由は `end_reason TEXT`（`checkmate` / `stalemate` / `insufficient_material` / `resignation`）に別途保持します。

### WebSocket イベント

| イベント | 配信タイミング |
|---|---|
| `Move` | 指し手ごと（FEN・チェック状態・終局判定） |
| `GameOver` | 終局時（チェックメイト経由・投了経由の両方） |
| `OpponentJoined` | 対戦相手が参加したとき |

---

## Key Design Decisions

設計判断の詳細は [`chess/docs/`](chess/docs/) にタスクごとのメモとして残しています。以下は主要なものの要約です。

### なぜ合法手判定をサーバーとクライアントで二重に持つのか

**権威はサーバー（shakmaty）のみが持ち、クライアント（chess.js）は表示専用**という役割分担にしています。

当初は `GET /games/{id}/legal_moves` を追加してサーバーの判定結果をそのまま表示に使う案で実装しましたが、削除しました。手番が回るたびに API を1往復するのはレイテンシとして無駄であり、何より「サーバー権威・フロントは表示のみ」という方針に対して中途半端な二重化になります。

判定ロジックが2箇所に存在するのは一般には避けたい形ですが、**役割が非対称**であれば問題になりません。フロントの誤判定は「ハイライトが出ない / 余分に出る」だけで、不正な手は必ずサーバーが弾きます。ライブラリが別（shakmaty / chess.js）でも、正しさの保証はサーバー側に一本化されています。

### なぜ WebSocket 認証をクエリパラメータではなく最初のメッセージで行うのか

ブラウザの WebSocket API は任意のヘッダーを送れないため、`Authorization: Bearer` が使えません。残る選択肢はクエリパラメータ（`?token=...`）か、接続確立後の最初のメッセージです。

クエリパラメータ方式は実装が単純ですが、**トークンが URL に乗るためアクセスログやリバースプロキシのログに残ります**。接続後のメッセージで送る方式を採り、サーバー側は最初の1通を認証メッセージとして扱い、検証に失敗すれば接続を閉じます。

```json
{"token": "..."}
```

`AuthMessage { token: String }` には serde の rename 属性を付けていません。フィールド名がそのまま JSON キーになるため、フロントの型定義と突き合わせる際に変換規則が挟まりません。

### なぜ進行中の対局をメモリに置き、DB を正本にしないのか

1手ごとに局面を DB へ書き戻すと、対局中のレスポンスが DB のラウンドトリップに支配されます。チェスの局面は数百バイトで、同時進行数もこの規模のアプリでは限られるため、メモリ保持が現実的です。

ただし**棋譜（`moves`）と対局結果（`games`）は必ず永続化**します。ここが分岐点で、「揮発してよいのは再現可能な派生データだけ」という基準を置いています。局面（FEN）は棋譜から再生できますが、棋譜そのものは失われたら復元できません。

投了時はメモリ上のマップから対局を削除します。以降その対局への `move` は「メモリに存在しない」ため自然に 404 になり、終了フラグを見て弾く分岐を書かずに済みます。

### なぜ `useGameSocket` にゲームロジックを持たせないのか

このフックは**接続管理と生イベントの中継に専念**し、盤面 state（FEN / 手番 / 結果）の更新は `GamePage` 側で行います。

フック内で FEN 管理まで行う案（呼び出し側は `const { fen, turn } = useGameSocket()` で済む）も検討しましたが、採用しませんでした。その場合フックが「接続管理」と「ゲーム状態管理」の2責務を持ち、`GameEvent` の種類が増えるたびに内部の分岐を触ることになります。

現在の分離なら、`OpponentJoined` イベントを追加したときもフック側は無変更で済みました。再利用（観戦画面など）が必要になった時点で `useChessGame` として切り出す、2段階のリファクタ方針を取っています。

### なぜ接続状態を5値で持つのか

`connecting` / `reconnecting` / `open` / `closed` / `error` の5つを区別しています。

当初は `connecting` のみで初回接続と再接続を区別していませんでしたが、UI 側で「接続が切れました。再接続しています」と出したいケースで破綻しました。**状態を表す列挙は「実装が区別できるか」ではなく「UI が区別して見せたいか」で設計する**という判断です。

再接続は指数バックオフ（1秒 → 2秒 → 4秒 …最大30秒）で行い、アンマウント時のクリーンアップと意図しない再接続をフラグで区別しています。

### なぜ 401 を CustomEvent でアプリ全体に通知するのか

トークン期限切れの検知を各 API 呼び出し箇所に書くと、新しいエンドポイントを追加するたびに書き漏れが発生します。

`client.ts` の1箇所で 401 を検知し `CustomEvent` を発火、`App.tsx` の `SessionExpiredListener` が受け取って自動ログアウト・トースト表示・ログイン画面への遷移を行う構成にしました。**検知を一元化しておけば、API を追加しても自動的に効きます。**

ログイン / 登録フォーム自体の 401 は対象外です（トークンを送っていないため、通常の「パスワードが違います」表示のままにする）。

### なぜ対戦相手の参加を WebSocket イベントにしたのか

対局作成者が相手を待っている間、参加を検知する手段としてポーリングと WebSocket イベントの2案がありました。

ポーリングはバックエンド変更が不要という利点がありますが、検知まで数秒のラグが出ます。「今、相手が来ました」という即時性のある通知としては弱く、加えて対局画面に WebSocket とポーリングの二重の仕組みが並存することになります。

すでに `broadcast` チャンネルと `GameEvent` の仕組みが整っていたため、`OpponentJoined` バリアントを追加するほうが既存設計との一貫性が高く、後で技術的負債になりにくいと判断しました。

### なぜ対戦相手名を対局レスポンスに含めず、別エンドポイントにしたのか

`GameDetailResponse` に `white_username` / `black_username` を足す案のほうが実装は簡単で、API の往復も減ります。

それでも `GET /users/{id}` を新設したのは、汎用エンドポイントにしておけば将来のプロフィール表示や対局履歴一覧にも再利用できるためです。対局情報とユーザー情報の責務も分離され、「観戦者にも見せるか」といった権限設計が必要になったときに、ルールを別々に定義できます。

公開情報（id / username）のみを返し、内部で使う情報は含めていません。

### なぜレースコンディションをアプリのロックではなく SQL で防ぐのか

対局への参加（`join`）と投了（`resign`）は、同時リクエストで不整合が起きうる箇所です。

`SELECT` してから `UPDATE` する二段構えだと、その間に別リクエストが割り込みます。条件付き `UPDATE` の**更新行数**で判定する形にすれば、DB のトランザクションが一貫性を保証してくれます。

```sql
UPDATE games SET black_user_id = $1, status = 'in_progress'
WHERE id = $2 AND black_user_id IS NULL
```

更新行数が 0 なら「既に誰かが参加済み」として 409 を返します。アプリ側でロックを持つより、DB に判断を委ねるほうが確実です。

---

## 技術スタック

| 領域 | 技術 |
|---|---|
| バックエンド | Rust 1.96 / axum 0.7 |
| チェスルール判定 | shakmaty |
| DB | PostgreSQL 16 |
| DB アクセス | sqlx（ORM 不使用） |
| 認証 | argon2（パスワードハッシュ）/ jsonwebtoken（JWT） |
| フロントエンド | Vite / React 19 / TypeScript |
| 盤面 UI | react-chessboard v5（カスタム駒セット） |
| コンテナ | Docker（マルチステージビルド） |
| ホスティング | Render（Docker / Static Site）/ Neon（Postgres） |
| CI / CD | GitHub Actions |

---

## 実装上の工夫

### PostgreSQL の ENUM 型と Rust の橋渡し

`game_status` / `game_result` を DB の ENUM 型として定義しているため、Rust 側との変換で明示キャストが必要です。`sqlx::query` は動的クエリのためコンパイル時に検証されず、**実行時に初めて型不一致が判明します**。

| 方向 | 書き方 |
|---|---|
| SELECT（デコード） | `SELECT status::text FROM games` |
| バインド（エンコード） | `SET result = $1::game_result` |

バインド側のキャスト漏れは特に厄介でした。`make_move` のチェックメイト時 UPDATE でこれが漏れており、しかもエラーが `tracing::error!` でログ出力されるだけで HTTP レスポンスに伝播しない実装だったため、**API が 200 を返しているのに DB が更新されていない**というサイレント障害になっていました。

エラーを握りつぶす箇所は、意図的にそうしているのか伝播させ忘れているのかを区別する、という教訓として `docs/task-07` に記録しています。

### コンテナクエリ単位（cqw）と React Portal

駒のサイズをマス幅に追従させるため CSS のコンテナクエリ単位（`cqw`）を使っていますが、**ドラッグ中の駒だけが肥大化する**現象が起きました。

`node_modules/react-chessboard` のソースを読んだところ、ドラッグ中の駒は `@dnd-kit` の `DragOverlay` 経由で `document.body` 直下に portal されていました。React のツリー上は盤面の子でも、**DOM ツリー上はコンテナの外に出ている**ため、cqw の基準を見失っていたわけです。

盤面のマスと、portal される駒本体の両方に `container-type: inline-size` を設定して解決しました。検証は DOM に `pointerdown` / `pointermove` を直接発火させて `DragOverlay` の複製要素を生成し、`getComputedStyle` で font-size を実測しています（通常駒 ~51.78px に対しドラッグ中 51.765px）。

### エラーレスポンスの正規化

`fetch` 自体が失敗する場合（サーバー未起動・ネットワーク切断・CORS）、素の `TypeError` が投げられて呼び出し側の `status` 判定が壊れます。これを `ApiError { status: 0 }` に正規化し、「サーバーに接続できません」と表示するようにしました。

加えて、JSON のエラーボディが返らない場合（プロキシ越しの異常応答など）に備え、主要な HTTP ステータス（400 / 401 / 403 / 404 / 409 / 429 / 500 / 502 / 503）の日本語フォールバックメッセージを持たせています。

`ErrorBoundary` は `App.tsx` の**最外層**、プロバイダ層よりさらに外側に配置しています。内側に置くと `AuthProvider` / `ToastProvider` 自身の例外を捕まえられません。

### CORS

フロントエンド（Render Static Site）と API（Web Service）を別オリジンで運用するため、`tower-http` の `CorsLayer` を適用しています。

許可オリジンは `FRONTEND_ORIGIN` 環境変数から**カンマ区切りで複数指定**でき、ローカル開発用の `localhost` と本番ドメインを同時に許可できます。未設定時はローカル開発用ポートにフォールバックします。

---

## Setup

### 必要なもの

- Docker / Docker Compose
- Node.js
- Rust 1.90 以降（ローカルでビルドする場合）
- sqlx-cli（マイグレーションを実行する場合）

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

> `--features rustls` は必須です。TLS なしでビルドすると、Neon などのマネージド Postgres へ接続する際に `TLS upgrade required by connect options` で失敗します。

### 起動

```bash
git clone https://github.com/CaltDeepL/chess-1.git
cd chess-1/chess
cp .env.example .env
```

`.env` の `JWT_SECRET` は必ず変更してください。

```bash
openssl rand -hex 32
```

```bash
docker compose up --build -d
curl http://localhost:3000/health
```

### マイグレーション

sqlx CLI はホスト側で実行します。接続先は `chess/.env` の `DATABASE_URL` から読まれます。

```bash
cd chess
sqlx migrate run
docker compose exec db psql -U chess -d chess_db -c '\dt'
```

> **接続先ポートに注意**
>
> ホストの 5432 / 5433 は他プロジェクトのコンテナが使っているため、このプロジェクトの Postgres は **5434** で公開しています。
>
> | 実行主体 | 接続先 |
> |---|---|
> | ホストのシェル（sqlx CLI・`cargo test`） | `localhost:5434` |
> | api コンテナ | `db:5432`（Compose ネットワーク内部） |
>
> `.env` のポートが公開ポートとずれていると、**無関係な別プロジェクトの Postgres に接続してしまい、「パスワード認証エラー」として現れます**。同じ罠に2回はまりました（`docs/task-03`, `docs/task-26`）。

### フロントエンド

```bash
cd ..            # リポジトリルート
npm install
npm run dev      # http://localhost:5174
```

本番ビルド時の接続先は `.env.production` の `VITE_API_URL` から読まれます。

---

## Testing

```bash
cd chess
docker compose up -d db     # 統合テストは実際の Postgres を使います
cargo test
```

統合テストは `#[sqlx::test(migrations = "./migrations")]` により、**テストごとに独立した一時 DB** を作成してマイグレーションを適用します。テスト間の状態共有がないため、複数のテストが同じユーザー名を使っても干渉せず、並列実行しても順序に依存した失敗が起きません。

`tower::ServiceExt::oneshot` でルータへ直接リクエストを投げる方式を採り、HTTP サーバを起動せずにルーティングからハンドラ・リポジトリ・DB までを通しで検証しています。ポートの取り合いや起動待ちがない分、実行が速く安定します。

現在は認証系（登録の成否・ユーザー名重複・パスワード長・ログインの成否・ユーザー列挙攻撃対策）をカバーしています。

API の手動確認は curl で行えます。

```bash
curl -X POST http://localhost:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}'

curl -X POST http://localhost:3000/games -H "Authorization: Bearer <token>"

curl -X POST http://localhost:3000/games/<id>/move \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" -d '{"uci":"e2e4"}'
```

WebSocket は Python + `websockets` の簡易クライアントで、接続 → 認証メッセージ送信 → イベント受信を検証しました。

```bash
python3 -m venv wsenv && wsenv/bin/pip install websockets
GAME_ID=<game_id> TOKEN=<token> wsenv/bin/python ws_listen.py
```

---

## CI / CD

```
PR
 ↓
CI
 ├─ backend:  cargo fmt --check / clippy -D warnings / cargo test
 └─ frontend: tsc -b / eslint / vite build

main merge
 ↓
CI green
 ↓
Render Deploy Hook（backend / frontend）
```

| ワークフロー | トリガー | 内容 |
|---|---|---|
| CI | push（main）/ pull_request / 手動 | fmt・clippy・テスト（Postgres サービス付き）・フロントの型チェックとビルド |
| Deploy | CI の成功（main のみ） | Render の Deploy Hook を起動 |

**Render の auto-deploy は無効にしています。** auto-deploy は `main` への push を検知して即座にビルドを始めるため、テストの結果を待ちません。`workflow_run` イベントで CI の完了と結果を受け取り、成功時に限って Deploy Hook を叩く構成にすることで、CI が green のときだけデプロイが走ることを保証しています。

CI では Postgres をサービスコンテナとして起動し、`DATABASE_URL` を渡しています。`sqlx::query!` マクロを使っていないため、`.sqlx` オフラインキャッシュや `cargo sqlx prepare` は不要です。

## Deployment

```
git push（main）
 ↓
CI（GitHub Actions）
 ↓
CI green のときのみ Deploy Hook
 ├─ バックエンド: Docker Web Service（Root Directory: chess）
 └─ フロントエンド: Static Site（npm run build → dist）
 ↓
Neon PostgreSQL
```

| 環境変数 | 設定先 | 内容 |
|---|---|---|
| `DATABASE_URL` | バックエンド | Neon の接続文字列 |
| `JWT_SECRET` | バックエンド | `openssl rand -hex 32` の出力 |
| `FRONTEND_ORIGIN` | バックエンド | 許可オリジン（カンマ区切り） |
| `VITE_API_URL` | フロントエンド | `.env.production` にコミット（ビルド時に埋め込まれる） |

> `PORT` は **設定してはいけません**。Render が自動注入する値と競合し、`invalid port value` で起動に失敗します（`docs/task-24`）。

SPA のため、Static Site 側で `/*` → `/index.html` の Rewrite ルールを設定しています。これがないと `/lobby` への直接アクセスやリロードが 404 になります。

---

## Implementation Status

### バックエンド（完了）

| # | タスク |
|---|---|
| 1 | 設計・技術選定 |
| 2 | 最小サーバー / shakmaty 統合 |
| 3 | DB 設計 / Docker 環境構築 |
| 4 | マイグレーション |
| 5 | 認証（register / login / JWT） |
| 6 | 対局 API（作成・参加・指し手・状態取得） |
| 7 | 投了エンドポイント |
| 8 | WebSocket エンドポイント |
| 9 | モジュール分割リファクタリング |
| 11 | 対局一覧エンドポイント |

### フロントエンド（完了）

| # | タスク |
|---|---|
| 10 | 設計・土台実装（型定義 / API 層 / 認証状態 / ルーティング） |
| 12 | 認証画面 |
| 13 | ロビー画面 |
| 14 | useGameSocket フック |
| 15 | ChessBoard コンポーネント |
| 16 | GamePage 本体 |
| 17 | ガラス調デザインへの統一 |
| 18 | 対局画面 UI 強化（LED / メニュー / トースト / オーバーレイ） |
| 19 | 合法手ハイライト・スマホ対応 |
| 20 | プロモーション対応 |
| 21 | ロビーのタイル化・自動更新 |
| 22 | 棋譜サイドバー（SAN 表示） |
| 23 | エラー系の作り込み |

### デプロイ・CI/CD（完了）

| # | タスク |
|---|---|
| 24 | Render + Neon への本番デプロイ |
| 25 | GitHub Actions による CI と、CI 成功時のみのデプロイ |
| 26 | 統合テスト基盤（lib.rs 切り出し・`#[sqlx::test]`・認証系6件） |

---

## Future Work

| 優先 | 項目 | 内容 |
|---|---|---|
| 1 | 対局 API の統合テスト | 投了・終局時に **DB の中身まで assert** する。API が 200 を返すのに DB が更新されていないサイレント障害（`docs/task-07`）を再発させないため |
| 2 | `domain` 層の切り出し | 手番判定・終局判定・勝者決定を I/O なしの純粋関数にし、DB 不要でユニットテストできる形へ |
| 3 | エラーレスポンスの RFC 9457 化 | 現在は `(StatusCode, String)`。`AppError` + Problem Details に統一する |
| 4 | OpenAPI | utoipa による仕様生成と Swagger UI の配信 |
| 5 | 対局履歴の閲覧 | 棋譜は DB にあるが、過去対局を見返す UI が未実装 |
| 6 | レーティング | Elo による対局結果の反映 |

---

## 開発記録

全26タスクの設計判断・つまずいた点・再現コマンドを [`chess/docs/`](chess/docs/) に記録しています。特に、型チェックをすり抜けたバグの傾向は横断的な教訓としてまとめました。

- **API 関数の引数順序の取り違え** — `token` と `id` の位置が逆になるバグが4関数すべてで発生。全引数が `string` 型のため `tsc` をすり抜け、ブラウザで実行して初めて発覚した
- **ファイル内容の誤混入・保存漏れ** — 関数定義が消えて呼び出し側だけ残る、別ファイル用のコードが書き込まれる、JSX が誤ったスコープに置かれる
- **型システムがカバーしない境界** — Postgres の ENUM、`verbatimModuleSyntax`、CSS のコンテナクエリ基準
- **環境・設定の不一致** — `.env` のポートずれ、PaaS が自動注入する環境変数との衝突、リポジトリ名変更後の remote 未更新。いずれもコードとは無関係なエラーとして現れる

いずれも「ビルドが通ること」では検出できず、**実際にブラウザで動かし、DB の中身を確認し、DevTools でネットワークと DOM を見た**ことで発見に至っています。

---

## License

MIT