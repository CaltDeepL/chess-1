# task-30 WebSocket の統合テスト

## ゴールと完了条件
- WebSocket のイベント配信を自動テストで守る（従来は Python の簡易クライアントで手動確認）
- 完了条件: `cargo test` と CI の両方が緑で、配信・認証・参加者チェック・対局間の隔離がカバーされること

## 依存の追加

```toml
[dev-dependencies]
tokio-tungstenite = "0.26"
futures-util = "0.3"
```

バージョンは axum が内部で使っているものに合わせる。

```bash
cargo tree -i tungstenite
```

## 構成: REST は oneshot のまま、WS だけ実サーバー

```
test_state(pool) で AppState を1つ作る
        │
        ├─→ spawn_server(&state) で TcpListener 起動 ──→ WS の接続・受信
        │
        └─→ post_json / create_game / make_move (oneshot) ──→ REST 操作
```

| ファイル | 内容 |
|---|---|
| `tests/common/mod.rs` | `spawn_server` / `open_ws` / `send_auth` / `connect_ws` / `next_event` / `assert_no_event` / `ws_send_raw` を**追加** |
| `tests/ws_test.rs` | 7件 |

結果: 全58件（unittests 17 + 統合 41）が緑。`cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` も通過。

## 設計判断の根拠

### なぜ oneshot が使えないのか
既存の統合テストは `tower::ServiceExt::oneshot` で Router に直接リクエストを渡している。これは **HTTP 1往復を模擬する**仕組みであり、`101 Switching Protocols` を経て双方向通信に移る WebSocket は扱えない。実際に TCP ポートを開く必要がある。

### なぜ REST 側まで実サーバーに寄せないのか
`AppState` のフィールドは `Arc` でラップされているため、`state.clone()` から組み立てた Router 同士は `games` / `game_channels` を共有する（task-27）。したがって **oneshot で作った対局が、実サーバーの WS ハンドラからも見える**。REST 側を書き換える必要がなく、既存ヘルパーをそのまま再利用できるため差分が最小になる。

### なぜポート0で bind するのか
固定ポートにすると、テストの並列実行やローカルで起動中の `cargo run` と衝突する。ポート0を渡せば OS が空きポートを割り当てる。

### なぜ受信にタイムアウトを入れるのか
イベントが配信されない不具合が起きたとき、タイムアウトが無ければ `ws.next().await` で**永久に待ち続ける**。ローカルなら Ctrl-C で済むが、CI ではジョブ枠を占有する。「落ちる」ことと「固まる」ことは別問題として扱う。

### `other_games_do_not_leak_events` を入れた理由
配信チャネルが対局ごとに分かれていることは、コードを読めば分かるが**壊れても他のテストは全部通る**。対局Bを購読して対局Aで指し、届かないことを確認する形で明示的に守る。あわせて対局B自身の指し手が届くことも確認し、「そもそも購読が機能していないから届かなかった」という偽陽性を防いでいる。

## 実装を読んで分かったこと

### 認証はクエリではなく最初のメッセージ
`ws_handler` は upgrade を無条件に受け入れ、`handle_socket` の冒頭で最初の Text メッセージ `{"token":"..."}` を待つ。したがって:

| | HTTP的な認証 | このアプリ |
|---|---|---|
| upgrade | 401 で拒否 | **常に成功する** |
| 認証失敗 | 接続できない | `{"error":"..."}` を送って切断 |

テストも「接続を試みて Err になる」ではなく、「接続はできるがエラーメッセージが返り、以降イベントが届かない」形で書く必要がある。

**この設計自体は妥当**で、ブラウザの `WebSocket` API は接続時に任意のヘッダを設定できないため、トークンを渡すならクエリ文字列かメッセージのどちらかになる。クエリはアクセスログやリファラに残りうるので、メッセージで渡すほうが安全。

### 購読開始までの隙間
`subscribe()` は認証・参加者チェック・DB照会を**すべて終えてから**呼ばれる。認証メッセージの送信完了時点ではまだ購読していないため、そこで即座に REST を叩くとイベントを取りこぼす。

broadcast は購読前のイベントを配送しないので、これは仕様どおりの挙動。テスト側では `connect_ws` に 200ms の待機を入れて吸収している。

## つまずいた点と教訓

| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | 認証失敗系の2件が「届かないはずのイベントが配信された」で失敗 | `assert_no_event` がタイムアウトのみで判定していた。認証・参加者チェック失敗時、サーバーは Close ハンドシェイクを送らずソケットを drop するため、tungstenite 側には `Close` フレームや `Connection reset without closing handshake` が**即座に届く**。タイムアウトしないので「イベントが届いた」と誤判定していた | ストリーム終了・Close フレーム・エラーのいずれも「イベントではない」として扱うよう修正 |
| 2 | `other_games_do_not_leak_events` が409で失敗 | `setup_game` のユーザー名が `"white"` / `"black"` 固定で、同一テスト内で2回呼ぶと2回目の登録が重複 | UUIDサフィックスで毎回ユニーク化 |
| 3 | `resign_is_broadcast` が失敗 | 期待値を `"resign"` と書いていたが、実装は一貫して `"resignation"` を返す（`resign_test.rs` とも整合） | 期待値を修正 |
| 4 | `checkmate_is_broadcast` が失敗 | 4件目の move イベントに `result` / `end_reason` が乗る想定だったが、実装は `GameOver` を**別イベントとして追加送信**する | `next_event` をもう1回呼んで別イベントとして検証 |

### 教訓1: 「タイムアウトした = 何も起きていない」ではない（#1）
`assert_no_event` は `tokio::time::timeout(...).is_err()` で「イベントが無いこと」を表現していた。しかし待っていたのは**イベントではなくストリームの次のアイテム**であり、Close もエラーもアイテムとして届く。

述語の意味が `is_err()` という書き方に隠れていたため、**テストヘルパー自身のバグが、対象コードのバグのように見えた**。「何かが起きないこと」を検証するヘルパーは、起こりうる「何か」を列挙して明示的に分類する必要がある。

### 教訓2: 既存テストが答えを持っていた（#3）
`end_reason` の値は `resign_test.rs` が既に DB の中身で assert していた。WS 配信側の期待値だけを推測で書いたためにずれた。**同じ値を扱う既存テストがあるなら、そこを先に読む。**

### 教訓3: イベントの粒度は推測しない（#4）
「終局を伴う指し手」を1イベントで表すか2イベントで表すかは設計の分かれ目で、コードを見ないと決まらない。`GameEvent` のシリアライズ構造を確認すれば済んだ。

## その他の注意点

### 購読 → 発生 の順序（最重要）
```rust
// 正しい
let mut ws = connect_ws(addr, &game_id, &white).await;   // 1. 購読
make_move(&state, &game_id, &white, "e2e4").await;       // 2. 発生
next_event(&mut ws).await;                               // 3. 受信

// 誤り: 指してから繋ぐとイベントは既に流れ終わっている → タイムアウトで失敗
```
実装が正しくてもテストだけが落ちるため、原因の切り分けに時間を取られやすい。task-27 の 404 と同じ種類の失敗。

### 待機によるタイミング依存
`connect_ws` の 200ms は**環境が遅いと不足しうる**。CI で稀に落ちるようなら、サーバー側に「購読完了」を伝える仕組みを入れるのが本筋（下記）。

### イベントの構造（確認済み）
| 契機 | イベント | 主なフィールド |
|---|---|---|
| 指し手 | `Move` | `uci` |
| 投了 | `GameOver` | `result`, `end_reason: "resignation"` |
| チェックメイト | `Move` → `GameOver` の**2件** | 同上（`end_reason: "checkmate"`） |

## 次タスクへの引き継ぎ

### 購読完了の通知（推奨）
`subscribe()` の直後に `{"type":"connected"}` を1件送る実装にすると:

- テストの `sleep` を「`connected` を受け取るまで待つ」に置き換えられ、タイミング依存が消える
- フロントの接続LEDが「TCPが繋がった」ではなく「**購読が始まった**」を表せるようになる。現状は upgrade 成功時点で緑になるため、認証失敗でも一瞬緑になるはず

### その他
- 再接続時のイベント取りこぼし（task-22 で「現状は許容」とした点）は未カバー
- `handle_socket` の最初の `recv()` にタイムアウトが無い（コード中のコメントにも記載あり）。接続だけして何も送らないクライアントがタスクを保持し続ける
- 残る Future Work は「対局履歴の閲覧」「レーティング」の2件

## 再現コマンド

```bash
cd chess
docker compose up -d db
cargo test --test ws_test
cargo test                              # 全58件（unittests 17 + 統合 41）
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
