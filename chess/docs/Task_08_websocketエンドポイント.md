# task-08 WebSocketエンドポイント

## ゴールと完了条件
- `GET /ws/games/:id` で接続し、対局の更新をリアルタイムに受信できる
- 完了条件: 白の指し手→黒の指し手→白の投了のイベントが購読側に順次届き、異常系(第三者トークン・不正JSON・無効トークン)が拒否されること

## 設計
- クライアントは接続後、最初に `{"token": "..."}` を送信して認証(task-01の方針どおりクエリパラメータ方式は不採用)
- サーバーはトークンを検証し、その対局の参加者かどうかDBで確認
- 対局ごとに `tokio::sync::broadcast` チャンネルを `AppState.game_channels` として保持
- `make_move`(指し手ごとに `Move`、終局時は `GameOver` も)と `resign_game`(投了時に `GameOver`)から配信

### イベント種別
| イベント | 内容 |
|---|---|
| `Move` | FEN、UCI、チェック状態、終局判定 |
| `GameOver` | 結果(white_win/black_win/draw)、終了理由 |

## 設計判断の根拠
- **`broadcast` チャンネルを対局ごとに持つ**: 全体で1本にすると、無関係な対局のイベントまで各クライアントに届き、受信側でのフィルタが必要になる。対局単位ならサーバー側で自然に絞れる
- **`AuthMessage { token: String }` に rename 属性を付けない**: フィールド名がそのままJSONキー(`token`)になる。フロント側と型定義を突き合わせる際、変換規則が挟まらないぶん間違いにくい

## つまずいた点と教訓
| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | `AppState` 定義と `GameEvent` 型が消失 | **WSハンドラのコードを誤って `state.rs` に書き込んでしまった** | 設計に沿って正しいファイルへ再配置 |
| 2 | `resign_game` 内に存在しない変数を参照する壊れたブロードキャストの断片が混入 | 編集時の取り違え | 正しい位置に書き直し |
| 3 | `Cargo.toml` の `axum` version がプレースホルダ `"..."` のまま | 提示されたコード例をそのままコピーした | `"0.7"` に修正(`ws` featureは指定済みだった) |

- **教訓**: ファイル内容の誤混入がついにファイル単位で発生した(`state.rs` ↔ `ws.rs`)。task-05・task-06 の「関数が消える」から規模が拡大している。**編集後は必ず `cargo build` を通し、意図したファイルに書けているかを確認する**
- **教訓**: コード例に含まれるプレースホルダ(`"..."`)は、そのままだとTOMLとしては構文的に正しいため見落としやすい

## 次タスクへの引き継ぎ
- WebSocketの動作確認にはPython + `websockets` の簡易クライアント(`ws_listen.py`)を用意した。フロント実装前の検証手段として有効
- フロント側の `useGameSocket` は task-14 で実装する

## 再現コマンド
```bash
python3 -m venv wsenv && wsenv/bin/pip install websockets
GAME_ID=<game_id> TOKEN=<token> wsenv/bin/python ws_listen.py

# 別ターミナルから指し手を送ってイベント到達を確認
curl -X POST http://localhost:3000/games/<id>/move \
  -H "Authorization: Bearer $TOKEN_W" -H "Content-Type: application/json" -d '{"uci":"e2e4"}'
```