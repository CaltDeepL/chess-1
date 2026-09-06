# chess-app

[![CI](https://github.com/CaltDeepL/chess-1/actions/workflows/ci.yml/badge.svg)](https://github.com/CaltDeepL/chess-1/actions/workflows/ci.yml)

**Rust + WebSocket で実装した、オンライン対戦チェスアプリ。**

対局の合法手判定・手番管理・終局判定をサーバー側の権威として持ち、指し手・投了・終局・対戦相手の参加・切断をリアルタイムに配信します。Elo レーティング、対局履歴と棋譜再生、ランキングを備えています。

**デモ**: https://chess-frontend-0van.onrender.com
**API ドキュメント**: https://test1-9t4t.onrender.com/docs

`/register` からアカウントを作成し、ロビーで対局を作成すると、別のユーザーが参加した時点で対局が始まります。

API は Swagger UI からブラウザ上で試せます。`POST /auth/register` でアカウントを作成し、返却されたトークンを右上の **Authorize** に入力してください。

> 無料プランで稼働しているため、アクセスがない間はインスタンスが停止します。最初のリクエストは応答まで数十秒かかることがあります。

> **ポートフォリオプロジェクトです。** 全36タスクを完了し、本番環境（Render + Neon）で稼働しています。CI が green のときだけデプロイが走る構成です。

---

## なぜ作ったか

オンラインのチェスアプリ自体は数多くありますが、「リアルタイム対戦を成立させるために何を設計する必要があるのか」を自分の手で確かめたいと考えました。特に次の3点が、作ってみないと判断できない領域でした。

**どこまでをサーバーの権威とするか。** クライアント側で合法手を判定すればレスポンスは速くなりますが、改ざんされた指し手を防げません。逆にすべてサーバーに問い合わせると、駒を掴んだ瞬間の「動かせるマス」の表示すら往復が必要になります。**権威と表示を分離する**設計が要ります。

**リアルタイム通信の状態をどう扱うか。** WebSocket は「繋がっている / 切れている」の二値ではありません。初回接続中・再接続中・切断・エラーを区別しないと、ユーザーには「なぜか操作できない」としか見えません。切断したまま戻らない相手をどう扱うかも、通信の一時的な断絶と区別して設計する必要があります。

**対局の状態をどこに置くか。** 進行中の局面を1手ごとに DB へ書き戻すと、対局中のレイテンシが DB に支配されます。一方で棋譜と対局結果は永続化しなければ意味がありません。**揮発してよいものと、してはいけないものの線引き**が必要です。

これらを扱うには、チェスのルール自体よりも「状態の所在と権威」をどう設計するかが主題になります。ルール判定は [shakmaty](https://github.com/niklasf/shakmaty) に任せ、**その周辺の設計にコストを払う**方針にしました。

---

## Features

| 機能 | エンドポイント |
|---|---|
| 認証（JWT） | `POST /auth/register` `POST /auth/login` `POST /auth/logout` |
| ユーザー公開情報 | `GET /users/{id}` |
| 対局の作成・一覧 | `POST /games` `GET /games` |
| 対局詳細 | `GET /games/{id}` |
| 対局への参加 | `POST /games/{id}/join` |
| 指し手（UCI 形式） | `POST /games/{id}/move` |
| 投了 | `POST /games/{id}/resign` |
| 棋譜（指し手履歴） | `GET /games/{id}/moves` |
| 切断による勝ちの確定 | `POST /games/{id}/claim-abandonment` |
| 対局履歴 | `GET /users/me/games` |
| ランキング | `GET /users/ranking` |
| リアルタイム対局通知 | `GET /ws/games/{id}` |
| 疎通確認 | `GET /health` |
| API 仕様 | `GET /openapi.json` `/docs` |

このほか、運用用に `POST /internal/sweep`（共有シークレット認証、OpenAPI 非公開）があります。

フロントエンドは、ロビーのタイル表示と自動更新、合法手ハイライト、プロモーション（歩の昇格）、棋譜の SAN 表示、接続状態の LED インジケーター、勝敗オーバーレイ、対局履歴と棋譜再生、ランキング表、切断カウントダウン、スマホ対応を実装しています。

---

## Architecture

```
        React SPA（Render Static Site）
           │  REST / WebSocket
           ▼
        Rust + axum（Render, Docker）
        ├── routes/     HTTP・WS のエンドポイント
        ├── domain/     I/O を持たない純粋なルール判定
        ├── auth.rs     JWT 発行・検証
        ├── rating.rs   Elo の適用
        ├── abandon.rs  切断・離脱の判定と一括処理
        ├── errors.rs   AppError → RFC 9457 Problem Details
        ├── state.rs    AppState（対局のメモリ状態・broadcast チャンネル）
        └── shakmaty    合法手・チェックメイト判定（権威）
           │
           ▼
        PostgreSQL（Neon）
        users / games / moves
           ▲
           │  POST /internal/sweep（10分間隔）
        GitHub Actions
```

進行中の局面は `Arc<RwLock<HashMap<Uuid, Chess>>>` でメモリに、確定した情報（ユーザー・対局結果・棋譜・レーティング）は PostgreSQL に置いています。

### ディレクトリ構成

```
chess-app/
├── .github/workflows/        # ci.yml / deploy.yml / sweep.yml
├── chess/                    # バックエンド（Rust）
│   ├── Dockerfile            # マルチステージビルド
│   ├── compose.yaml          # Postgres + API
│   ├── migrations/           # sqlx マイグレーション
│   ├── docs/                 # タスクごとの設計メモ
│   └── src/
│       ├── main.rs           # 起動処理・マイグレーション適用
│       ├── lib.rs            # ルータ組み立て・OpenAPI 仕様
│       ├── state.rs          # AppState / GameEvent
│       ├── errors.rs         # AppError / ProblemDetails
│       ├── rating.rs         # Elo の適用（トランザクション）
│       ├── abandon.rs        # 切断・離脱の判定と sweep
│       ├── domain/           # 純粋関数（outcome / player / history / elo / password / abandon）
│       └── routes/           # health / game / user / history / ranking / ws / internal
└── frontend/                 # Vite + React + TypeScript
    ├── api/                  # fetch ラッパ / エラー正規化
    ├── context/              # AuthContext / ToastContext
    ├── hooks/                # useGameSocket（接続管理・再接続）
    ├── pages/                # Home / Login / Register / Lobby / Game / History / Review / Ranking
    ├── components/           # ChessBoard / GameList / MoveHistory / DisconnectCountdown / 他
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
| `draw` | ステイルメイト・駒不足・両者離席 |

終了理由は `end_reason TEXT` に別途保持します。

| 値 | 意味 |
|---|---|
| `checkmate` / `stalemate` / `insufficient_material` | 盤上で決着 |
| `resignation` | 投了 |
| `disconnection` | 一方が切断したまま猶予（60秒）を過ぎた |
| `abandonment` | 両者が離席したまま猶予を過ぎた（引き分け） |
| `logout` | 対局中に明示的にログアウトした |
| `cancelled` | 相手が参加しないまま作成者が離脱した（結果なし） |

### WebSocket イベント

| イベント | 配信タイミング |
|---|---|
| `Connected` | 購読の開始が完了したとき（接続した本人にのみ） |
| `Move` | 指し手ごと（FEN・チェック状態・終局判定） |
| `GameOver` | 終局時（チェックメイト・投了・切断・ログアウト） |
| `OpponentJoined` | 対戦相手が参加したとき |
| `PlayerDisconnected` | 一方の接続が0本になったとき（残り秒数つき） |
| `PlayerReconnected` | 切断していた側が戻ったとき |

---

## Key Design Decisions

設計判断の詳細は [`chess/docs/`](chess/docs/) にタスクごとのメモとして残しています。以下は主要なものの要約です。

### なぜ合法手判定をサーバーとクライアントで二重に持つのか

**権威はサーバー（shakmaty）のみが持ち、クライアント（chess.js）は表示専用**という役割分担にしています。

当初は `GET /games/{id}/legal_moves` を追加してサーバーの判定結果をそのまま表示に使う案で実装しましたが、削除しました。手番が回るたびに API を1往復するのはレイテンシとして無駄であり、何より「サーバー権威・フロントは表示のみ」という方針に対して中途半端な二重化になります。

判定ロジックが2箇所に存在するのは一般には避けたい形ですが、**役割が非対称**であれば問題になりません。フロントの誤判定は「ハイライトが出ない / 余分に出る」だけで、不正な手は必ずサーバーが弾きます。

### なぜ WebSocket 認証をクエリパラメータではなく最初のメッセージで行うのか

ブラウザの WebSocket API は任意のヘッダーを送れないため、`Authorization: Bearer` が使えません。残る選択肢はクエリパラメータ（`?token=...`）か、接続確立後の最初のメッセージです。

クエリパラメータ方式は実装が単純ですが、**トークンが URL に乗るためアクセスログやリバースプロキシのログに残ります**。接続後のメッセージで送る方式を採り、サーバー側は最初の1通を認証メッセージとして扱い、検証に失敗すれば接続を閉じます。認証メッセージ待ちには10秒のタイムアウトを設け、接続だけしてタスクを保持し続けるクライアントを防いでいます。

### なぜ「接続完了」を独自イベントで通知するのか

WebSocket の `onopen` は TCP と upgrade の完了しか意味しません。このアプリの認証は upgrade 後の最初のメッセージで行うため、**認証に失敗する接続でも `onopen` は発火します**。接続 LED をこれに紐づけると、失敗する接続でも一瞬「接続済み」と表示されていました。

サーバーが購読を開始した直後に `{"type":"connected"}` を送り、フロントはこれを受けて初めて `open` にします。統合テストからも「購読が始まった」ことを直接観測できるようになり、時間待ちに頼らずに済むようになりました。

### なぜ進行中の対局をメモリに置き、DB を正本にしないのか

1手ごとに局面を DB へ書き戻すと、対局中のレスポンスが DB のラウンドトリップに支配されます。チェスの局面は数百バイトで、同時進行数もこの規模のアプリでは限られるため、メモリ保持が現実的です。

ただし**棋譜（`moves`）と対局結果（`games`）は必ず永続化**します。「揮発してよいのは再現可能な派生データだけ」という基準です。局面（FEN）は棋譜から再生できますが、棋譜そのものは失われたら復元できません。

終局時はメモリ上のマップから対局を削除します。ただし参照系（`GET /games/{id}`）はメモリに無ければ DB の `fen` から局面を復元します。削除だけしてこの経路を用意していなかったため、**終了した対局の詳細が 404 になる**不具合がありました（`docs/task-33`）。

### なぜエラーレスポンスを RFC 9457（Problem Details）にしたのか

独自の `{"message": "..."}` でも動作はしますが、標準に乗ると **`Content-Type: application/problem+json` を見るだけでクライアントが「構造化されたエラーだ」と判断できます**。将来 CLI や他サービスが繋がっても、解釈方法を個別に伝える必要がありません。

```json
{
  "type": "/problems/forbidden",
  "title": "Forbidden",
  "status": 403,
  "detail": "あなたの手番ではありません"
}
```

この移行の副産物として、**500 の応答に内部のエラー文（DB のエラーメッセージ）がそのまま含まれていた**問題も塞がりました。`Internal` だけは固定文言に差し替え、原因はログにのみ残しています。

### なぜ Elo の変動を片側だけ計算するのか

白黒それぞれ独立に計算して丸めると、`round()` の結果次第で合計が ±1 ずれ、**系全体のレーティング総和が保存されなくなります**。白の変動値を計算し、黒はその符号を反転させることでゼロサムを構造的に保証しています。

変動値は `games` に保存します。保存しないと履歴画面で「この対局で何点動いたか」を出せず、**後から再計算することもできません**（当時の相手のレーティングが失われるため）。

終局の経路は投了・チェックメイト・切断・ログアウトと複数あるため、レーティングの適用は共通の関数を全経路から呼びます。経路ごとに書くと、片方だけ漏れても気づけません。

### なぜ切断の判定を外部の定期実行に任せるのか

対戦相手が切断したまま戻らない場合、猶予60秒を過ぎたら残ったプレイヤーの勝ちにします。問題は「誰が60秒を計るか」です。

Render の無料枠はリクエストが無いとスピンダウンするため、アプリ内で `tokio::spawn` したタイマーは**プロセスごと消えます**。GitHub Actions から `POST /internal/sweep` を10分間隔で叩く構成にしました。

ただし10分は決着として遅すぎます。そこで判定の契機を3つ用意しています。

| 契機 | 役割 |
|---|---|
| WebSocket 接続時 | 残っている側が画面を開いた瞬間に決着 |
| カウントダウンが0になったとき | 画面を開いたままの側が `claim-abandonment` を呼ぶ |
| 定期実行（10分間隔） | 誰も見ていない対局の後始末 |

判定そのものは常にサーバーが行うため、クライアントが時計を進めても猶予前に勝ちにはなりません。

### なぜ advisory lock をプールではなく専用の接続で取るのか

`&PgPool` を Executor に渡すと、呼び出しごとに**接続を借りて即座に返します**。advisory lock はセッションに紐づくため、ロックを取った接続と解放する接続が別になりえます。

その場合ロックを持ったままの接続がプールに残り、以後 sweep は永久に「実行中」と判定されます。**競合時は成功を返す設計なので、定期実行からは正常に見えたまま掃除が二度と走らない**という状態になります。

`acquire()` で1本を確保して保持し、取得と解放を同じ接続で行います。このバグは単一スレッドのテストでは再現しないため、回帰テストを追加したうえで旧実装に戻し、確実に失敗することを確認しました（`docs/task-36`）。

### なぜパスワードに文字種を強制しないのか

NIST SP 800-63B の方針に沿い、長さ（12文字以上）を主な担保としています。文字種を強制すると `Password1!` のような、**覚えにくいわりに破られやすい**パスワードに誘導してしまいます。長いパスフレーズを許可するほうが合理的です。

長さは**文字数**で数えます。`str::len()` はバイト数を返すため、日本語のパスフレーズが不当に長く評価されていました（「正しい馬の電池」は7文字だが21バイト）。上限（128文字）も設けています。上限が無いと、巨大な入力で Argon2 のハッシュ化がそのまま DoS の入口になります。

ログイン側では検証しません。要件の引き上げを適用すると**既存ユーザーが締め出される**うえ、「短すぎます」と返すことで保存されている値と一致するかに関わらず要件を満たさないことが分かってしまいます。

### なぜ `useGameSocket` にゲームロジックを持たせないのか

このフックは**接続管理と生イベントの中継に専念**し、盤面 state（FEN / 手番 / 結果）の更新は `GamePage` 側で行います。

フック内で FEN 管理まで行う案も検討しましたが、フックが「接続管理」と「ゲーム状態管理」の2責務を持ち、`GameEvent` の種類が増えるたびに内部の分岐を触ることになります。現在の分離なら、`OpponentJoined` や `PlayerDisconnected` を追加したときもフック側は無変更で済みました。

### なぜ接続状態を5値で持つのか

`connecting` / `reconnecting` / `open` / `closed` / `error` の5つを区別しています。当初は初回接続と再接続を区別していませんでしたが、「接続が切れました。再接続しています」と出したいケースで破綻しました。**状態を表す列挙は「実装が区別できるか」ではなく「UI が区別して見せたいか」で設計する**という判断です。

再接続は指数バックオフ（1秒 → 2秒 → 4秒 …最大30秒）で行っています。

### なぜ 401 を CustomEvent でアプリ全体に通知するのか

トークン期限切れの検知を各 API 呼び出し箇所に書くと、新しいエンドポイントを追加するたびに書き漏れが発生します。`client.ts` の1箇所で 401 を検知し `CustomEvent` を発火、`App.tsx` の `SessionExpiredListener` が受け取って自動ログアウトと遷移を行います。**検知を一元化しておけば、API を追加しても自動的に効きます。**

### なぜレースコンディションをアプリのロックではなく SQL で防ぐのか

`SELECT` してから `UPDATE` する二段構えだと、その間に別リクエストが割り込みます。条件付き `UPDATE` の**更新行数**で判定すれば、DB のトランザクションが一貫性を保証します。

```sql
UPDATE games SET black_user_id = $1, status = 'in_progress'
WHERE id = $2 AND black_user_id IS NULL
```

更新行数が 0 なら「既に誰かが参加済み」として 409 を返します。同じ考え方を終局処理にも使っており、WebSocket 接続時の判定と定期実行が同時に走っても、実際に対局を終了させるのは1つだけになります。

### なぜ OpenAPI とルート定義を同じ場所に置くのか

`utoipa-axum` の `OpenApiRouter` でルート登録とドキュメント生成をまとめています。`routes!()` に渡したハンドラがそのまま axum のルートになり、同時に `#[utoipa::path]` の情報から仕様が組み立てられます。

配信される仕様とテストが検証する仕様は同じ組み立てから生成しています。テスト用に組み直すと「テストは緑だが実際に配信される仕様は別物」という状態がありえるためです。統合テストではパス数の一致、認証必須エンドポイントの `security` 宣言、**全 4xx/5xx が `ProblemDetails` を参照していること**を検証しています。

なお Swagger UI 本体は `utoipa-swagger-ui` を使わず、CDN から読み込む 20 行程度の静的 HTML にしています。ビルド時に UI の zip をダウンロードする時間が CI とデプロイのたびにかかるためです。

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
| API ドキュメント | utoipa / utoipa-axum（OpenAPI 3.1、RFC 9457） |
| CI / CD | GitHub Actions |

---

## 実装上の工夫

### PostgreSQL の ENUM 型と Rust の橋渡し

`game_status` / `game_result` を DB の ENUM 型として定義しているため、Rust 側との変換で明示キャストが必要です。`sqlx::query` は動的クエリのためコンパイル時に検証されず、**実行時に初めて型不一致が判明します**。

| 方向 | 書き方 |
|---|---|
| SELECT（デコード） | `SELECT status::text FROM games` |
| バインド（エンコード） | `SET result = $1::game_result` |

バインド側のキャスト漏れは特に厄介でした。`make_move` のチェックメイト時 UPDATE でこれが漏れており、しかもエラーが `tracing::error!` でログ出力されるだけで HTTP レスポンスに伝播しない実装だったため、**API が 200 を返しているのに DB が更新されていない**というサイレント障害になっていました（`docs/task-07`）。

### マイグレーションの適用を起動時に行う

sqlx CLI での手動適用に頼っていたところ、ローカル DB への適用漏れで `column does not exist` が発生しました。**`#[sqlx::test]` は毎回専用 DB に全マイグレーションを当てるため、テストは緑のままローカル環境だけが壊れます。**

起動時に `sqlx::migrate!()` を実行する形にし、ローカル・本番とも適用漏れが構造的に起きないようにしました。本番へのマイグレーション適用が手動だった問題も同時に解消しています。

### コンテナクエリ単位（cqw）と React Portal

駒のサイズをマス幅に追従させるため CSS のコンテナクエリ単位（`cqw`）を使っていますが、**ドラッグ中の駒だけが肥大化する**現象が起きました。

`node_modules/react-chessboard` のソースを読んだところ、ドラッグ中の駒は `@dnd-kit` の `DragOverlay` 経由で `document.body` 直下に portal されていました。React のツリー上は盤面の子でも、**DOM ツリー上はコンテナの外に出ている**ため、cqw の基準を見失っていたわけです。

盤面のマスと、portal される駒本体の両方に `container-type: inline-size` を設定して解決しました。

### ドメインロジックの分離

I/O を持たない純粋関数を `domain/` に切り出しています。引数を渡すだけで結果が決まるため、対局を実際に進めなくても、また DB を用意しなくても検証できます。

```rust
pub fn determine_outcome(position: &Chess) -> (&'static str, &'static str);
pub fn winner_after_resign(resigning: Color) -> &'static str;
pub fn role_of(user_id: Uuid, white: Uuid, black: Option<Uuid>) -> Role;
pub fn outcome_for(result: Option<&str>, my_color: Color) -> Option<Outcome>;
pub fn white_delta(white_rating: i32, black_rating: i32, score: f64) -> i32;
pub fn validate_password(password: &str, username: &str) -> Result<(), PasswordError>;
pub fn decide(white_at: Option<DateTime<Utc>>, black_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<Abandonment>;
```

切断判定は時刻を引数で受け取るため、**60秒待たずに境界の挙動をテストできます**。

### エラーレスポンスの正規化（フロント側）

`fetch` 自体が失敗する場合（サーバー未起動・ネットワーク切断・CORS）、素の `TypeError` が投げられて呼び出し側の `status` 判定が壊れます。これを `ApiError { status: 0 }` に正規化しています。

サーバーは RFC 9457 の `detail` を返しますが、プロキシ越しの異常応答など JSON が返らない場合に備え、主要な HTTP ステータスの日本語フォールバックメッセージを持たせています。

`ErrorBoundary` は `App.tsx` の**最外層**、プロバイダ層よりさらに外側に配置しています。内側に置くと `AuthProvider` / `ToastProvider` 自身の例外を捕まえられません。

### CORS

フロントエンド（Render Static Site）と API（Web Service）を別オリジンで運用するため、`tower-http` の `CorsLayer` を適用しています。許可オリジンは `FRONTEND_ORIGIN` 環境変数から**カンマ区切りで複数指定**でき、ローカル開発用の `localhost` と本番ドメインを同時に許可できます。

---

## Setup

### 必要なもの

- Docker / Docker Compose
- Node.js
- Rust 1.90 以降（ローカルでビルドする場合）
- sqlx-cli（マイグレーションを手動実行する場合。通常は起動時に自動適用されます）

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

`.env` の `JWT_SECRET` と `SWEEP_TOKEN` は必ず変更してください。

```bash
openssl rand -hex 32
```

```bash
docker compose up --build -d
curl http://localhost:3000/health
```

マイグレーションは起動時に自動で適用されます。手動で流す場合は sqlx CLI をホスト側で実行します。

```bash
cd chess
sqlx migrate info
sqlx migrate run
```

> **マイグレーションのファイル名はタイムスタンプ形式で統一してください。** sqlx はファイル名先頭の数値をそのままバージョンとして扱うため、連番形式（`0001_`）を混ぜると既存の `20260805202110_init.sql` より小さい値と解釈され、テーブル作成より先に適用されようとして失敗します（`docs/task-32`）。

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
cd ../frontend
npm install
npm run dev
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

REST は `tower::ServiceExt::oneshot` でルータへ直接リクエストを投げ、HTTP サーバを起動せずにルーティングからハンドラ・DB までを通しで検証しています。WebSocket は `101 Switching Protocols` を伴うため oneshot では扱えず、こちらだけ空きポートで実サーバーを起動します。**同じ `AppState` を共有しているため、oneshot で作った対局が実サーバーの WS ハンドラからも見えます。**

現在 **150 件**のテスト（ユニット 54 / 統合 96）が以下をカバーしています。

| ファイル | 件数 | 内容 |
|---|---|---|
| `auth_test.rs` | 13 | 登録・ログイン・ユーザー列挙攻撃対策・パスワード要件 |
| `game_test.rs` | 12 | 対局参加、指し手の記録、権限・手番・合法性、終了済み対局の取得 |
| `resign_test.rs` | 5 | 投了の結果反映、再投了、投了後の指し手拒否 |
| `checkmate_test.rs` | 5 | Fool's mate / Scholar's mate による終局判定 |
| `ws_test.rs` | 7 | イベント配信・順序・認証・参加者チェック・対局間の隔離 |
| `abandon_test.rs` | 21 | 切断猶予・両者離席・ログアウト即敗北・sweep・ロック解放 |
| `rating_test.rs` | 7 | 経路別の適用、ゼロサム、二重適用の防止 |
| `ranking_test.rs` | 8 | 順位付け、同着、認証任意、自分の順位 |
| `history_test.rs` | 9 | 自分視点の勝敗、絞り込み、ページング |
| `problem_details_test.rs` | 3 | Content-Type・`type`・`status` の検証 |
| `openapi_test.rs` | 6 | 仕様の配信、パス数の一致、`ProblemDetails` の参照 |

これに加え、`domain` 層（手番・終局・勝者・履歴・Elo・パスワード・切断判定）のユニットテストが 54 件あります。I/O を持たない純粋関数なので DB なしで実行でき、一瞬で終わります。

**結果が確定する経路では、API のステータスコードだけでなく `games` テーブルの中身まで assert しています。** 過去に「API は 200 を返すのに DB が更新されていない」というサイレント障害を見逃した経験があるためです（`docs/task-07`）。

### テスト自体が壊れていた例

テストが緑であることは、テストが意図どおり検証していることを意味しません。実際に見つかった例:

- **`assert_no_event` がタイムアウトのみで判定していた**（`docs/task-30`）。サーバーが Close ハンドシェイクを送らず切断する経路では、Close フレームが即座に届くためタイムアウトせず、「イベントが届いた」と誤判定していた
- **セットアップの失敗を assert していなかった**（`docs/task-35`）。パスワード要件の引き上げで事前登録が 400 になり対象ユーザーが存在しなくなったが、未知ユーザーへのログインとして同じ 401 が返るため**緑のまま意味を失っていた**
- **`pg_locks` を DB で絞り込んでいなかった**（`docs/task-36`）。並列実行中の他テストのロックを誤検知していた

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
| Sweep | 10分間隔 / 手動 | `POST /internal/sweep` を叩き、放置された対局を終了させる |

**Render の auto-deploy は無効にしています。** auto-deploy は `main` への push を検知して即座にビルドを始めるため、テストの結果を待ちません。`workflow_run` イベントで CI の完了と結果を受け取り、成功時に限って Deploy Hook を叩く構成にしています。

## Deployment

| 環境変数 | 設定先 | 内容 |
|---|---|---|
| `DATABASE_URL` | バックエンド | Neon の接続文字列 |
| `JWT_SECRET` | バックエンド | `openssl rand -hex 32` の出力 |
| `SWEEP_TOKEN` | バックエンド / GitHub Secrets | 同じ値を両方に設定 |
| `FRONTEND_ORIGIN` | バックエンド | 許可オリジン（カンマ区切り） |
| `APP_URL` | GitHub Secrets | sweep の宛先 |
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

### 品質・機能拡張（完了）

| # | タスク |
|---|---|
| 26 | 統合テスト基盤（lib.rs 切り出し・`#[sqlx::test]`・認証系6件） |
| 27 | 対局 API の統合テスト（join / move / resign / 終局） |
| 28 | OpenAPI 仕様の生成と Swagger UI の配信 |
| 29 | エラーレスポンスの RFC 9457 化（Problem Details・OpenAPI 反映） |
| 30 | WebSocket の統合テスト（配信・認証・参加者チェック・対局間の隔離） |
| 31 | WebSocket 接続の確実性（connected イベント・認証待ちタイムアウト） |
| 32 | 対局履歴 API（`GET /users/me/games`・自分視点の勝敗判定） |
| 33 | 対局履歴の画面（一覧・棋譜再生）と終了済み対局取得の修正 |
| 34 | レーティング（Elo）とランキング |
| 35 | パスワード要件の見直し（長さ中心・拒否リスト・文字数カウント修正） |
| 36 | 対局からの離脱の扱い（切断猶予・ログアウト即敗北・sweep の定期実行） |
| 37 | パスワード入力の改善（表示トグル・要件の案内） |

## Future Work

| 項目 | 内容 |
|---|---|
| Dependabot | Cargo / npm / GitHub Actions の依存更新 |
| MFA（TOTP） | 2段階認証 |
| K 値の可変化 | 対局数の少ないうちは変動を大きくする（暫定レーティング） |
| 再接続時のイベント補完 | 切断中に進んだ手を、再接続後に差分で受け取る |
| レーティング推移のグラフ | `games` の変動値の累積を可視化 |

## 開発記録

全36タスクの設計判断・つまずいた点・再現コマンドを [`chess/docs/`](chess/docs/) に記録しています。特に、型チェックをすり抜けたバグの傾向は横断的な教訓としてまとめました。

- **API 関数の引数順序の取り違え** — `token` と `id` の位置が逆になるバグが4関数すべてで発生。全引数が `string` 型のため `tsc` をすり抜け、ブラウザで実行して初めて発覚した
- **ファイル内容の誤混入・保存漏れ** — 関数定義が消えて呼び出し側だけ残る、別ファイル用のコードが書き込まれる、JSX が誤ったスコープに置かれる。**5回発生**しており、貼り付け後の `git diff` 確認を手順に組み込んだ
- **型システムがカバーしない境界** — Postgres の ENUM、`verbatimModuleSyntax`、CSS のコンテナクエリ基準、コネクションプールとセッションスコープのロック
- **テスト自体のバグ** — 検証したいものを検証しなくなっても、テストは緑のまま通り続ける
- **環境・設定の不一致** — `.env` のポートずれ、PaaS が自動注入する環境変数との衝突、マイグレーションの適用漏れ、ビルド後のプロセス再起動忘れ。いずれもコードとは無関係なエラーとして現れる

いずれも「ビルドが通ること」では検出できず、**実際にブラウザで動かし、DB の中身を確認し、DevTools でネットワークと DOM を見た**ことで発見に至っています。

---

## License

MIT