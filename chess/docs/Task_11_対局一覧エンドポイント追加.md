# task-11 対局一覧エンドポイント(GET /games)追加

## ゴールと完了条件
- ロビー画面のために `GET /games` を追加する
- 完了条件: 未認証で401、`status=waiting` で絞り込み、指定なしで全件が返ること

## 実装
- `models.rs` に `ListGamesQuery`(`status: Option<String>`)と `GameSummary`(id / white_user_id / black_user_id / status / fen / created_at)を追加
- `routes/game.rs` に `list_games` ハンドラを追加。SELECT側は task-06 の教訓どおり `status::text` でキャスト
- `main.rs` で `GET /games` と `POST /games` は同じパスなので `.route("/games", get(list_games).post(create_game))` と1つにまとめて登録

## 設計判断の根拠
- **`status` 未指定なら全件を `created_at DESC` で返す**: ロビーでは `waiting` だけを使うが、汎用的にしておけば将来の対局履歴一覧にも流用できる。フィルタをクライアント側の指定に委ねる形にした
- **JWT必須にする**: 他のエンドポイントと一貫させる。ロビーはログイン後の画面なので認証が前提でよい

## つまずいた点と教訓
- 特筆すべき詰まりなし。task-06 で得た「ENUM型はSELECT側で `status::text` にキャストする」という教訓を先回りで適用でき、同じバグを踏まずに済んだ
- **教訓**: 過去タスクの教訓をドキュメント化しておくと、同種の実装で先回りできる。この docs 運用自体の効果が確認できたケース

## 次タスクへの引き継ぎ
- フロントの `getGames(token, status)` から呼ぶ(task-13)
- レスポンスの `GameSummary` 型はフロントの `types/index.ts` にも同名で定義する

## 再現コマンド
```bash
# 未認証 → 401
curl -i http://localhost:3000/games

# waiting のみ
curl "http://localhost:3000/games?status=waiting" -H "Authorization: Bearer <token>"

# 全件
curl http://localhost:3000/games -H "Authorization: Bearer <token>"
```