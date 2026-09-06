# task-39 axum 0.8 系への移行

## ゴールと完了条件
- axum を 0.7 から 0.8 に上げ、連動する依存も同時に更新する
- 完了条件: 全テストが緑、clippy がクリーン、実機で対局が成立すること

Dependabot の PR（#27 axum / #29 utoipa-axum / #26 tokio-tungstenite）は**個別には通りません**。3つ以上を同時に上げる必要があるため、依存更新ではなく移行タスクとして扱います。

## 同時に上げる必要があるもの

| クレート | 現在 | 移行先 | 理由 |
|---|---|---|---|
| `axum` | 0.7 | 0.8 | 本体 |
| `utoipa-axum` | 0.1 | 0.2 | axum 0.8 対応版 |
| `tower-http` | 0.5 | **0.6** | **Dependabot は提案していないが必要**。axum 0.8 は tower 0.5 系が前提で、tower-http は 0.6 が対応版 |
| `tokio-tungstenite`（dev） | 0.24 | 0.30 | テスト側。単独でも上げられるが、`Message` の型変更が axum 側と同時に来るのでまとめる |

`utoipa` は 5 系のままで問題ありません（utoipa-axum 0.2 は utoipa 5 に対応）。

**`tower-http` が Dependabot の PR に出てこないのが罠です。** 0.5 のままでも semver 上は解決できてしまうため、Dependabot は更新を提案しません。しかしビルドは通らない、という状態になります。

## 想定される破壊的変更

### 1. パスパラメータの記法（影響が最大）

```
0.7:  /games/:id       /files/*path
0.8:  /games/{id}      /files/{*path}
```

**task-36 で `{id}` と書いてしまい 0.7 の `:id` に直してもらった箇所**は、今回まさにこの記法に戻ります。

`Router::route` は不正な記法を**コンパイルエラーではなく実行時 panic** で報告します。ビルドが通っても起動した瞬間に落ちるので、`cargo run` で確認するまで気づけません。

```bash
grep -rn '"/[^"]*:[a-z]' src/ | grep -v "://"
```

### 2. `Message` の型（WebSocket）

```
0.7:  Message::Text(String)      Message::Binary(Vec<u8>)
0.8:  Message::Text(Utf8Bytes)   Message::Binary(Bytes)
```

`ws.rs` の送信箇所と、`tests/common/mod.rs` の受信・送信箇所が該当します。

**task-36 で clippy が「無駄な `.into()`」として削除させた箇所**が、今度は `.into()` が必要になります。`Utf8Bytes` は `From<String>` を実装しているので `.into()` で足ります。

```bash
grep -rn "Message::Text\|Message::Binary" src/ tests/
```

### 3. `Option<T>` 抽出子

0.8 では `Option<Path<T>>` などが `OptionalFromRequestParts` を要求します。このプロジェクトでハンドラ引数に `Option<...>` を使っている箇所があれば影響します。

```bash
grep -rn "Option<Path\|Option<Query\|Option<Json\|Option<TypedHeader" src/
```

### 4. `tokio-tungstenite` 側の `Message`

こちらも 0.26 以降で `Utf8Bytes` になっています。テストヘルパーの `send_auth` / `ws_send_raw` / `next_event` が該当します。

`next_event` の `Message::Text(text)` は `text.as_str()` または `&text` で `&str` として扱えます。

## 実際の結果

**コンパイルエラーは2件だけだった。** 想定より大幅に小さい。

| 種類 | 件数 | 箇所 |
|---|---|---|
| `Message::Text` の型（`String` → `Utf8Bytes`） | 2 | `src/routes/ws.rs:150, 171` |
| パス記法（`:id` → `{id}`） | 3 | `src/lib.rs`（**実行時 panic**） |

`Option<T>` 抽出子の変更、`tower-http` の型不整合は該当なし。テストヘルパー側は既に `.into()` が付いていたため、tokio-tungstenite 0.30 の `Utf8Bytes` にもそのまま通った。

最終的に **146件全件通過**、`fmt` / `clippy -D warnings` もクリーン。

### `.into()` が「冗長」から「必要」に変わった

`Message::Text(json.into())` の `.into()` は、**task-36 で clippy が「無駄な変換」として削除させた箇所**そのもの。0.7 では `Message::Text(String)` だったので確かに冗長だったが、0.8 で `Utf8Bytes` になり必要になった。

当時の削除は正しく、今回の追加も正しい。**依存のバージョンによって「冗長かどうか」が変わる**ため、clippy の指摘は「そのバージョンにおける正しさ」でしかない。

### パス記法は実行時 panic

コンパイルは通り、`cargo run` の起動時に初めて落ちる。今回は `cargo test` でも検出できたが、これは `build_router` を通るテストがあったから。**ルータの組み立てを通らない経路しかなければ、本番で起動失敗して初めて分かる。**

## 一番の学び: パス記法を書く場所が2種類ある

| 書く場所 | 従うルール | 0.7 → 0.8 |
|---|---|---|
| `.route("/games/{id}", ...)` | **axum のバージョン** | `:id` → `{id}` に変わる |
| `#[utoipa::path(path = "/games/{id}")]` | **OpenAPI 仕様** | 常に `{id}`、変わらない |

同じファイルの近い場所に、**バージョンに追従するものとしないものが並んでいる**。0.7 の時代は片方が `:id`、もう片方が `{id}` という状態で、書き間違えやすかった。

**axum 0.8 で記法が OpenAPI 形式に揃ったため、今後この2つは一致する。** ずれていたのは 0.7 までの話。

（このプロジェクトでは task-36 で `{id}` と書いて 0.7 の `:id` に直し、今回また `{id}` に戻すという往復が起きている。）

## uuid のピン留めを解除した

`uuid = { version = "=1.10.0" }` の完全一致ピンは、**今はもう使っていない作業用サンドボックス環境の Rust 1.75 向けの回避策**だった。新しい uuid が引き込む `getrandom 0.4` が edition2024（Rust 1.85 以上）を要求するため、当時のサンドボックスでビルドできなかった。

現在の実行環境はいずれも該当しない。

| 環境 | Rust |
|---|---|
| CI | `dtolnay/rust-toolchain@stable` |
| 本番（Docker） | `rust:1.90` |
| ローカル | 1.96 |

Dependabot の PR #25（1.10.0 → 1.26.0）で CI が緑だったことが決め手になり、`"1"` に緩めた。

**完全一致のピンは「この版でなければ壊れる」という強い主張**なので、理由が消えたら外す。残しておくと、次に読む人（半年後の自分を含む）が同じ調査をすることになる。今回まさにそれが起きた。

**教訓**: ピン留めするときは理由をコメントに残す。今回は残っていたおかげで、解除してよいかを短時間で判断できた。

## 進め方

**先に緑の PR 4本（#33 / #31 / #25 / #24）をマージし、`git pull` してから始めてください。** 対応中に他の更新が混ざると切り分けが難しくなります。

```bash
git switch -c chore/axum-0.8
cd chess

cargo add axum@0.8 --features ws
cargo add utoipa-axum@0.2
cargo add tower-http@0.6 --features cors,trace
cargo add --dev tokio-tungstenite@0.30

# エラーの全体像を掴む
cargo build --all-targets 2>&1 | grep -E "^error" | sort | uniq -c | sort -rn
```

**エラーの種類と件数を先に見てから着手します。** 上から順に潰すと同じ種類の修正を何度も繰り返すことになるので、種類ごとにまとめて直すほうが速い。

### 順序

| # | 対象 | 確認 |
|---|---|---|
| 1 | パス記法（`:id` → `{id}`） | `cargo build` |
| 2 | `Message` の型（`src/routes/ws.rs`） | `cargo build` |
| 3 | `Message` の型（`tests/common/mod.rs`） | `cargo build --all-targets` |
| 4 | 残りのコンパイルエラー | `cargo build --all-targets` |
| 5 | clippy | `cargo clippy --all-targets -- -D warnings` |
| 6 | テスト | `cargo test` |
| 7 | **起動確認** | `cargo run` — パス記法の panic はここで初めて出る |
| 8 | 実機 | 対局・WebSocket・OpenAPI |

**7を飛ばさないこと。** テストは `oneshot` でルータを組み立てるので、パス記法の panic はテストでも出るはずですが、`build_router` を通らない経路があれば見逃します。

## 実機で見るところ

```bash
lsof -ti:3000 | xargs kill
SWEEP_TOKEN=local-dev-sweep-token cargo run
```

| # | 確認 | 理由 |
|---|---|---|
| 1 | 起動時に panic しない | パス記法 |
| 2 | `curl localhost:3000/openapi.json \| jq '.paths \| keys'` | utoipa-axum 0.2 でパスの生成規則が変わっていないか |
| 3 | 対局を1局通す（WebSocket 配信） | `Message` の型変更 |
| 4 | `/docs` が表示される | 静的 HTML なので影響は薄いが念のため |

**2が重要です。** OpenAPI のパスは元々 `{id}` 形式で出力されているはずですが、`utoipa-axum` がルート定義から生成しているため、記法変更の影響を受ける可能性があります。`openapi_test.rs` の `EXPECTED_PATH_COUNT` とパス名の検証が通れば問題ありません。

## Dependabot の PR をどうするか

3本（#27 / #29 / #26）は**閉じます**。このブランチに含まれるため、マージ時に自動で閉じない場合は手動で。

コミットメッセージに以下を書いておくと GitHub が自動で閉じます。

```
Closes #26
Closes #27
Closes #29
```

## 単独で対応できるもの（このタスクとは別）

| PR | 内容 | 備考 |
|---|---|---|
| #32 | typescript 6 → 7 | フロントのみ。`tsc -b` で影響を見る |
| #28 | jsonwebtoken 9 → 11 | `auth.rs` のみ。`encode` / `decode` の署名変更を確認 |

**同時にやらないこと。** 落ちたときの切り分けが難しくなります。

## つまずいたら

エラーが多すぎて手が止まりそうなら、**`tower-http` だけ先に 0.6 に上げてビルドが通るか**を確認してください。これは axum の変更と独立しているので、単独で通ります。通れば tower 系の依存解決は済んだことになり、残りは axum 本体の API 変更に絞れます。

## 次タスクへの引き継ぎ
- **`tower-http` は 0.6.11 が入ったが、0.7.1 が出ている。** 次の Dependabot が提案してくる
- **`tokio-tungstenite` が2バージョン同居している。** axum 0.8 の ws feature が 0.29 を、dev-dependency が 0.30 を引き込む。プロトコル上は問題ないが、揃えるなら dev 側を 0.29 に下げる
- 単独で対応できる残りの major: `typescript` 6 → 7（#32）、`jsonwebtoken` 9 → 11（#28）
- Future Work の残り: MFA（TOTP）、K 値の可変化、再接続時のイベント補完、レーティング推移のグラフ
