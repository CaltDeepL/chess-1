# task-22 棋譜サイドバー(UCI → SAN表示)

## ゴールと完了条件
- 対局画面の横に棋譜(指し手履歴)を表示する
- 標準代数記法(SAN)で表示する
- 完了条件: 白黒ペアで整形表示され、WebSocket経由の新しい指し手がリロードなしで追記されること

## 実装
### バックエンド
- `GET /games/:id/moves`: `MoveRow`(move_number / uci / fen_after)の配列を返す

### フロント
- `MoveHistory.tsx` を `.game-layout`(flex)で盤面と横並びに配置
- 初回はRESTで取得、以降はWebSocketの `move` イベントで追記
- `chess.js` でUCI列を初手から再生し **SAN(`e4`, `Nf3`, `O-O`, `Nxe5` 等)に変換**して表示

## 設計判断の根拠
- **表示はSAN、通信はUCI**: UCI(`e2e4`)は機械的に扱いやすいがチェスプレイヤーには読みにくい。棋譜として見せるならSANが自然。変換はフロントで `chess.js` に任せる(task-19 で導入済み)
- **`.game-page` の `max-width` を 560px → 960px に拡張**: task-21 のロビーと同じパターン。サイドバーを追加するなら器を広げる必要がある

## つまずいた点と教訓
| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | バックエンドがコンパイルできない | `get_moves` の雛形が `MoveRow` 未import・**存在しない `AppError` 型**を使用していた | 他ハンドラと同じ `(StatusCode, String)` エラー形式に統一 |
| 2 | `query_as` で型エラー | `MoveRow` に `sqlx::FromRow` の derive が無かった | derive を追加 |
| 3 | サイドバーが表示されない | **`moves` state と `useEffect` がコンポーネント外(モジュールスコープ)に、`<MoveHistory>` のJSXが `handlePieceDrop` 関数の中に紛れ込んでいた** | 正しい位置に配置し直し |
| 4 | `Square width not found` 例外 | react-chessboard内部のアニメーション処理がDOM幅の測定に失敗するタイミング問題 | `showAnimations: false` で発生経路ごと遮断 |

- **教訓(#3)**: ファイル内容の誤混入がついに**関数スコープ単位**で起きた。「コンポーネントは存在するが画面に出ない」ときは、そのJSXが本当にreturn文の中にあるかを確認する
- **教訓(#4)**: ライブラリ内部のタイミング問題は、原因を追い切るより**その機能自体を無効化して発生経路を断つ**ほうが安上がりなことがある。アニメーションはこのアプリの本質ではないので割り切った

## 補足: move_number のズレ
APIが返す `move_number` は shakmaty の仕様で白番の手の直後から2に進み、本来のPGN手数とはズレる。ただし `MoveHistory.tsx` は**配列のインデックスで白黒をペアリング**しており、この値自体を表示に使っていないため実害なしと判断した。

## 次タスクへの引き継ぎ
- `moves` state は「初回REST取得+WSで追記」の二重管理。WSの再接続時にイベントを取りこぼすと齟齬が出る可能性がある(現状は許容)

## 再現コマンド
```bash
curl http://localhost:3000/games/<id>/moves -H "Authorization: Bearer <token>"
npx tsc -b --force
npm run dev
# 対局を進めてサイドバーに 1. e4 e5 / 2. Nf3 Nc6 のように出るか確認
```