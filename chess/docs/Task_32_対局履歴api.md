# task-32 対局履歴 API（バックエンド）

## ゴールと完了条件
- 自分が参加した終了済みの対局を一覧できる API を追加する
- 完了条件: `GET /users/me/games` が動き、統合テスト9件が通ること

画面は task-33 で作る。API と画面を分けたのは、検証の粒度を保つため。

## 追加するもの

| ファイル | 内容 |
|---|---|
| `migrations/20260906075200_add_games_history_indexes.sql` | 部分インデックス3本 |
| `src/domain/history.rs` | `outcome_for()` と単体テスト7件 |
| `src/routes/history.rs` | `list_my_games` ハンドラ |
| `tests/history_test.rs` | 統合テスト9件 |
| 配線 | `domain/mod.rs` / `routes/mod.rs` / `lib.rs` / `openapi.rs` |

## 設計判断の根拠

### なぜ `GET /games` に足さず新設するのか
ロビーの一覧は「他人の対局を探す」、履歴は「自分の対局を見返す」で**問い自体が違う**。1つのハンドラに `?status=&mine=` を足すと分岐が増え、OpenAPI の記述も曖昧になる。分けたほうがテストも書きやすく、既存のロビーに一切手を入れずに済む。

### なぜマイグレーションが結果カラムの追加ではなくインデックスだけなのか
`finished_at` を新設しなくても、終局時に `updated_at` が更新されるため代用できる。カラムを増やすと更新箇所も増えるので、**既存の値で足りるなら足さない**。

一方インデックスは必要で、`WHERE (white_user_id = $1 OR black_user_id = $1)` は単一のインデックスでは効かないため、白番用・黒番用を別々に張る。`status = 'finished'` の部分インデックスにしているのは、対象が終了済みだけだから。インデックス自体が小さくなる。

### なぜ勝敗判定を domain 層に出すのか
`games.result` は "white_win" / "black_win" という**盤面視点**の値だが、画面に出したいのは「自分が勝ったか」。この変換は I/O を持たない純粋関数なので、`domain/history.rs` に置けば DB 不要で単体テストできる（task-29 の `domain` 切り出しと同じ方針）。

「同じ対局でも見る人によって win / loss が反転する」ことは、統合テスト側でも `outcome_is_relative_to_the_viewer` として確認している。

### なぜ未知の `result` で panic しないのか
`outcome_for` は想定外の値に対して `None` を返す。ここで panic すると、DB に未知の値が1件入っただけで**一覧全体が 500 になる**。1件の表示が欠けるほうが影響が小さい。

### なぜ limit に上限を設けるのか
上限が無いと1リクエストで全件を引かれる。既定20・上限100とし、範囲外は 400。

## つまずいた点と教訓

### マイグレーションのバージョン番号の形式を揃える
新規マイグレーションを `0001_...` として置いたところ、`openapi_test.rs` が軒並み `relation "games" does not exist` で失敗した。

このプロジェクトの既存マイグレーションは `20260805202110_init.sql` というタイムスタンプ形式で、sqlx は**ファイル名先頭の数値をそのままバージョンとして扱う**。`0001` は整数 1 と解釈され、`20260805202110` より小さいため、`games` テーブルの作成より**先に**インデックス追加が適用されようとしていた。

`20260906075200_add_games_history_indexes.sql` にリネームして解決。

**教訓**: 連番形式とタイムスタンプ形式を混在させない。既存ファイル名を1つ確認してから新規ファイルの名前を決める。「連番に合わせる」という指示だけでは、どちらの形式かは決まらない。

### 症状が出た場所と原因の場所がずれる
失敗したのは `openapi_test.rs` で、今回追加した `history_test.rs` ではなかった。マイグレーション自体が全テストの前提なので、**壊れると無関係なテストから落ちる**。エラーメッセージ（`relation "games" does not exist`）がテーブル作成の話をしているときは、テストの内容ではなくマイグレーションの適用順を疑う。

### `EXPECTED_PATH_COUNT` が設計どおり機能した
`openapi_test.rs` の `EXPECTED_PATH_COUNT` を 10 → 11 に更新した。これは「エンドポイントを足したのに OpenAPI に載せ忘れる」ことを検知するための仕掛けで、**初めてエンドポイントを追加した回に実際に仕事をした**。手作業で `paths(...)` に足す構造である以上、この種の見張りは有効。

## 実装に合わせて調整が必要な箇所

コードは未コンパイル。

| # | 箇所 | 想定 | 確認方法 |
|---|---|---|---|
| 1 | `end_reason` の値 | 投了は `"resignation"` | task-30 で確認済み |
| 2 | `updated_at` の更新 | 終局時に更新される | 更新していなければ `resign` / 終局処理の UPDATE に `updated_at = now()` を足す |
| 3 | `get_auth` ヘルパー | 未定義なら `wiring-snippets.rs` の実装を追加 | `grep -n "fn get_auth" tests/common/mod.rs` |
| 6 | `openapi_test.rs` の `EXPECTED_PATH_COUNT` | エンドポイント追加分だけ増やす | 10 → 11 |
| 4 | `chrono` の feature | `serde` が有効か | 無効なら `DateTime<Utc>` が Serialize できない |
| 5 | ルーター組み立ての場所 | `openapi_router()` 内（task-29 で切り出し済み） | 配信仕様に載るのはこちら |

**特に #2 が重要。** `updated_at` が終局時に更新されていないと、並び順が「対局開始順」になり `history_is_ordered_newest_first` が落ちる。落ちたらカラム追加ではなく UPDATE 文の修正で対応する。

## 再現コマンド

```bash
cd chess
docker compose up -d db
sqlx migrate info    # 適用順が想定どおりか確認
sqlx migrate run
cargo test --test history_test
cargo test                    # 全74件（unittests 24 + 統合 50）
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# 実物の確認
TOKEN=$(curl -s -X POST localhost:3000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"password123"}' | jq -r .token)
curl -s localhost:3000/users/me/games -H "Authorization: Bearer $TOKEN" | jq
```


## 次タスクへの引き継ぎ（task-33: 画面）
- `/history` 一覧と `/games/:id/review` 棋譜再生の2画面
- **再生は `GET /games/:id/moves` の `fen_after` をそのまま盤面に渡すだけで済む**（task-22 で DB に持たせた判断がここで効く）。chess.js で初手から再現する必要はない
- `src/types/index.ts` の `GameEvent` に `connected` が入っていない（task-31 で追加したイベント）。履歴の型を足すついでに揃えておくとよい