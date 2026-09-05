# task-27 対局APIの統合テスト

## ゴールと完了条件
- join / move / resign / 終局判定を統合テストで守る
- **APIのステータスコードだけでなく、DBの中身まで検証する**
- 完了条件: 認証6件と合わせて計25件が `cargo test` とCIの両方で通ること

## テスト構成

| ファイル | 件数 | 内容 |
|---|---|---|
| `tests/auth_test.rs` | 6 | 登録の成否・重複409・パスワード長・ログインの成否・ユーザー列挙攻撃対策 |
| `tests/game_test.rs` | 9 | join(正常・自作400・満席409)、move(記録・第三者403・手番違反403・不正手400・不正UCI400・連続手の順序) |
| `tests/resign_test.rs` | 5 | 投了の結果反映・白黒それぞれの勝者判定・再投了409・投了後move404・第三者403 |
| `tests/checkmate_test.rs` | 5 | Fool's mate / Scholar's mate の結果反映・終局後のmove拒否・終局後resign409・棋譜の記録 |

## 設計判断の根拠

### なぜ「APIが200を返すこと」だけでは不十分なのか
task-07 で、**投了とチェックメイトの両方で `::game_result` キャストが漏れており、APIは200を返すのにDBが更新されていない**というサイレント障害が起きていた。エラーが `tracing::error!` でログ出力されるだけでHTTPレスポンスに伝播しない実装だったため、手動のcurl確認では気づけなかった。

そこで、結果が確定する経路のテストでは必ず `games` テーブルを直接読んで assert している。

```rust
let (db_status, result, end_reason) = fetch_outcome(&state, &game_id).await;
assert_eq!(db_status, "finished");
assert_eq!(result.as_deref(), Some("black_win"));
assert_eq!(end_reason.as_deref(), Some("checkmate"));
```

これで、同じキャスト漏れが再発すれば即座に赤くなる。**当時見逃した2つの経路(投了・チェックメイト)が両方とも守られた**状態になった。

### なぜ Fool's mate と Scholar's mate を使うのか
実際に詰みまで指す必要があるため、最短の詰み筋を選んだ。

| 詰み筋 | 手順 | 結果 |
|---|---|---|
| Fool's mate | `f2f3` `e7e5` `g2g4` `d8h4` | 黒の勝ち(2手) |
| Scholar's mate | `e2e4` `e7e5` `f1c4` `b8c6` `d1h5` `g8f6` `h5f7` | 白の勝ち(4手) |

白勝ち・黒勝ちの両方を通すことで、**勝者判定が逆になっていないか**も検証できる。

### なぜテストヘルパーを `AppState` 単位にするのか
後述のバグの結果だが、設計としても正しい。このアプリは進行中の対局をメモリ(`AppState.games`)で管理しているため、**1つの対局に対する複数リクエストは同じ `AppState` を共有していなければ成立しない**。実際のサーバーも1つの `AppState` を全リクエストで共有しているので、テストもその構造に合わせるのが自然。

## つまずいた点と教訓

### moveのテスト4件がすべて404で失敗
| 症状 | 原因 |
|---|---|
| `move_is_recorded_in_db` などが 404 | `tests/common/mod.rs` の `app(pool)` が**リクエストごとに新しい `AppState` を作っていた**。`create_game` で登録した対局が、次のリクエストで組み立てられる別の Router からは見えない |

```
create_game → Router A のメモリに対局を登録
join        → Router B(メモリは空)。DBは共有なのでDB更新だけは成功する
make_move   → Router C(メモリは空)→ 対局が見つからず404
```

`test_state(pool)` で **1つの `AppState` を作り、リクエストごとに `state.clone()` から Router を組み立てる**形に変更して解決。`AppState` のフィールドは `Arc` でラップされているため、clone しても `games` / `game_channels` は共有される。

**なお `join` のテストが通っていたのは、join がDBのみを更新する実装だったため。** メモリを参照する `move` だけが落ちるという症状が、原因の切り分けを助けた。

### 教訓
- **これは実装のバグではなくテストの組み立て方の問題だった。** 「進行中の対局はメモリ、確定情報はDB」というこのアプリ固有のアーキテクチャに、テストの構造が合っていなかった
- 逆に言えば、**テストを書いたことでその設計特性が明示的に浮かび上がった**。どこまでがDBで完結し、どこからメモリ状態に依存するのかが、テストの通り方から読み取れた
- 失敗したテストと通ったテストの差(move は落ちるが join は通る)は、原因を絞り込む有力な手がかりになる

### ヘルパーの重複解消
`auth_test.rs` だけが `common` を使わず独自の `app()` / `post_json()` を持っていたため、`common` に寄せて統一した。`register_user` ヘルパーは内部で `assert_eq!(status, OK)` するので、ステータス自体を検証したいテスト(重複409など)では `post_json` を直接使う。

## 次タスクへの引き継ぎ
- 現在カバーできていない領域: WebSocket のイベント配信、`GET /games` の絞り込み、`GET /games/:id/moves`、プロモーション
- `domain` 層を切り出せば、手番判定・終局判定・勝者決定を**DB不要のユニットテスト**にでき、実行時間も短くなる

## 再現コマンド
```bash
cd chess
docker compose up -d db
cargo test                       # 全25件
cargo test --test checkmate_test # 個別実行
cargo test --all-targets         # CIと同じオプション
```