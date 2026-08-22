# task-06 対局API(作成・参加・指し手・状態取得)

## ゴールと完了条件
- 対局の作成・参加・指し手送信・状態取得がすべてJWT認証必須で動作する
- 参加者チェック・手番チェックが機能する
- 完了条件: 第三者アクセス403、手番違反403、正常系200(盤面更新+movesテーブル記録)がcurlで確認できること

## 各エンドポイントの仕様
### POST /games(対局作成)
- 認証ユーザーを `white_user_id` として `games` へINSERT、メモリ上のHashMapにも登録

### POST /games/:id/join(対局参加)
- 対局作成者(white)自身の参加は 400
- 既に `black_user_id` が埋まっている対局への参加は 409
- **`UPDATE ... WHERE black_user_id IS NULL` で更新行数をチェック** — 同時参加のレースコンディション対策
- 成功時 `status` を `waiting` → `in_progress` に更新

### POST /games/:id/move(指し手送信)
- 参加者(white/black)以外は 403、手番違反は 403(黒未参加の場合は 409)
- 指し手ごとに `moves` へ `move_number` / `uci` / `fen_after` を記録
- 終局(詰み・ステイルメイト・駒不足)検知時に `games.status` を `finished`、`result` と `end_reason` を更新

### GET /games/:id(状態取得)
- 盤面(FEN)・チェック状態・終局判定を返す

## 設計判断の根拠
- **レースコンディションはアプリのロックでなくSQLの条件で防ぐ**: `SELECT` してから `UPDATE` する二段構えだと間に別リクエストが割り込む。`WHERE` 条件付き `UPDATE` の更新行数で判定すれば、DBのトランザクションが一貫性を保証してくれる
- **手番違反を403にする**: 「認証は通っているが、この操作をする権限が今はない」という意味で401(未認証)ではなく403(権限なし)が適切

## つまずいた点と教訓
| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | `join_game` で500エラー | `GameRow.status`(PostgresのカスタムENUM型 `game_status`)を Rust の `String` で受けようとして型ミスマッチ | SELECT側で `status::text` とキャスト |
| 2 | `unexpected closing delimiter` | `make_move` 関数の閉じ括弧の後に余分な `}` が1つ残存 | 余分な括弧を削除 |
| 3 | `E0425: cannot find function 'position_to_fen'` | task-02 で作った関数の定義が消失(呼び出し3箇所は残存) | ファイル末尾に復元 |

- **教訓(ENUM型)**: `sqlx::query` / `query_as` は動的クエリのためコンパイル時に型チェックされず、**実行時に初めて型不一致が判明する**。SELECT側(デコード)は `status::text`、バインド側は `$1::game_result` と、**両方向で明示キャストが必要**
- **教訓(定義消失)**: task-05 に続き2件目。差分編集を繰り返す環境では、関数がまるごと消える事故が現実に起きる

## 次タスクへの引き継ぎ
- ENUM型のキャストは **バインド側でも必要**。task-07(投了)で同じ問題を踏むので先に意識しておくこと
- `GET /games/:id` のレスポンスには `white_user_id` / `black_user_id` / `result` が含まれていない。フロントで手番の色判定ができず、task-16 で `GameDetailResponse` を新設することになる

## 再現コマンド
```bash
TOKEN_W=<whiteのtoken>; TOKEN_B=<blackのtoken>
GAME=$(curl -s -X POST http://localhost:3000/games -H "Authorization: Bearer $TOKEN_W")
curl -X POST http://localhost:3000/games/<id>/join -H "Authorization: Bearer $TOKEN_B"
curl -X POST http://localhost:3000/games/<id>/move \
  -H "Authorization: Bearer $TOKEN_W" -H "Content-Type: application/json" -d '{"uci":"e2e4"}'
docker compose exec db psql -U chess -d chess_db \
  -c "SELECT move_number, uci FROM moves ORDER BY move_number;"
```