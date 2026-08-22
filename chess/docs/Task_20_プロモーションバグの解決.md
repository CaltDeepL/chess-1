# task-20 プロモーション(歩の昇格)バグの調査と解決

## ゴールと完了条件
- 歩が最終ランクに到達したとき、駒種を選択して正しいUCI(例 `e7e8q`)を送信できる
- 完了条件: クイーン昇格・アンダープロモーション・キャンセル・キャンセル後の再ドラッグの4パターンが正しく動作すること

## 発生していた症状
`POST /games/:id/move` に**不正なUCI文字列**(例: `c2d1b`)が送信されていた。

## 原因
プロモーション駒種の選択オーバーレイ表示中も**ドラッグ自体が制限されておらず**、`pendingPromotion`(選択待ちのsource/target)が残ったまま別の駒をドラッグしてPOSTが成立してしまう。その後に選択ボタンを押すと、**無関係な2操作のsource/targetが混ざった `makeMove` 呼び出し**になっていた。

## 解決までの経緯(重要)
1. **1回目の対策**: `pendingPromotion` 中は `isMyTurn` を `false` にしてドラッグを止める + `handlePieceDrop` / `handlePromotionNeeded` に二重ガードを追加 → 再現手順で解消・`tsc -b` 通過を確認
2. **しかし再発**: 同じ症状が再び発生
3. **最終的な修正**: 責務の置き場所を変えた
   - `ChessBoard.tsx` の `onPieceDrop` は、昇格時は**サーバーへ送らず `onPromotionNeeded` を呼ぶだけ**にして `false` を返す
   - `GamePage.tsx` の `handlePieceDrop` は UCI組み立て時に `pendingPromotion` 中の未指定呼び出しをガードする

## 設計判断の根拠
- **1回目の対策は「ガードを足す」アプローチで、根本の構造は変えていなかった**。だから抜け道が残り再発した
- 最終的な修正は「**そもそも昇格時はサーバーに送る経路に入らない**」という構造にした。ガードの網羅性に依存せず、経路自体を分ける
- **教訓**: 再発したバグは、対症療法(ガード追加)ではなく**責務の配置**を見直す。「防ぐ」より「起こりえない構造にする」

## 検証結果
| ケース | 結果 |
|---|---|
| クイーン昇格(捕獲あり) | `POST /move` 200 OK、送られたUCIどおり a8 にクイーンが配置されたFENが返却 |
| アンダープロモーション(ナイト) | 200 OK、正しくナイトが配置 |
| 昇格ダイアログの「キャンセル」 | **ネットワークリクエストが一切発生しない**ことを確認 |
| キャンセル後に再ドラッグ | `pendingPromotion` の残留なし、正常に再度ダイアログが出て昇格できる |

- 検証中コンソールに `Square width not found` が1回出たが、`GameOverOverlay.tsx` へのHMR更新と重なって発生した react-chessboard 内部の描画レースで、再現性なし(この例外は task-22 で恒久対処)

## 次タスクへの引き継ぎ
- `ChessBoardProps` に `onPromotionNeeded` が追加された。`GamePage` 側で渡し忘れると型エラーになる(本番デプロイ時のビルドで実際に検出された、task-24 参照)

## 再現コマンド
```bash
# curlで昇格直前の局面を作り、ブラウザでドラッグ&ドロップ→駒種選択
# DevToolsのネットワークタブで送信されたUCI文字列を確認する
curl -X POST http://localhost:3000/games/<id>/move \
  -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"uci":"c7c8q"}'
```