# task-34 レーティング（Elo）とランキング

## ゴールと完了条件
- 対局結果を Elo レーティングに反映する
- レーティング順の一覧 `/ranking` と、既存画面へのレーティング表示
- 完了条件: 投了・チェックメイトのいずれでも両者のレーティングが動き、総和が保存され、ランキングに反映されること

Future Work 最後の項目。**これで Future Work は空になる。**

## 追加・変更したもの

### バックエンド
| ファイル | 内容 |
|---|---|
| `migrations/..._add_rating.sql` | `users.rating` / `games.white_rating_delta` / `games.black_rating_delta` / `idx_users_rating` |
| `src/domain/elo.rs` | Elo 計算の純粋関数、単体テスト10件 |
| `src/rating.rs` | `apply_rating(pool, game_id)` |
| `src/routes/ranking.rs` | `GET /users/ranking` |
| `src/routes/user.rs` | `GET /users/:id` に `rating` |
| `src/routes/history.rs` | `GameHistoryItem` に `my_rating_delta` |
| `src/routes/game.rs` | `resign_game` と `make_move` の終局分岐から `apply_rating` を呼ぶ |
| `tests/rating_test.rs` | 7件 |
| `tests/ranking_test.rs` | 8件 |
| `tests/openapi_test.rs` | `EXPECTED_PATH_COUNT` 11 → 12 |

### フロントエンド
| ファイル | 内容 |
|---|---|
| `src/api/ranking.ts` | `getRanking(token?, {limit})` |
| `src/pages/RankingPage.tsx` | 順位表 + 自分の順位 |
| `src/types/index.ts` | `RankingEntry` / `RankingResponse` / `rating` / `my_rating_delta` |
| `src/App.tsx` | `/ranking`（ProtectedRoute の外） |
| `src/pages/LobbyPage.tsx` | ヘッダーに「ランキング」 |
| `src/pages/HistoryPage.tsx` | 変動値の列 |
| `src/pages/GamePage.tsx` | 対戦相手を `{username} ({rating})` 形式に |
| `src/styles/global.css` | ランキング表と変動値の色分け |

---

## 第1部: レーティングの計算

### K 値を固定 32 にした
対局数に応じて K を変える方式（暫定レーティング）が一般的だが、`games_played` の管理が必要になる。**まず動くものを入れ、必要なら後で足す。** 変えるときは `elo.rs` の1箇所で済む。

### 黒の変動を計算せず、白の符号を反転する
白黒それぞれ独立に計算して丸めると、`round()` の結果次第で合計が ±1 ずれ、**系全体のレーティング総和が保存されなくなる**。片方だけ計算して反転させればゼロサムが構造的に保証される。

`total_rating_is_preserved` で3局戦わせて合計が変わらないことを確認している。

### 変動値を `games` に保存する
保存しないと履歴画面で「この対局で何点動いたか」を出せない。**後から再計算することもできない**（当時の相手のレーティングが失われるため）。

副次的に、`white_rating_delta IS NULL` が「未適用」を意味するので、二重適用の防止条件としても使える。

### 終局処理から共通の関数を呼ぶ
投了・チェックメイトの2経路それぞれに計算を書くと、片方だけ漏れても気づけない。task-07 で `::game_result` のキャスト漏れが2経路のうち片方だけ残っていたのと同じ構図。

`checkmate_applies_rating` と `resign_moves_ratings_in_opposite_directions` の両方で実際の数値を assert し、**どちらの経路も同じ関数を通っている**ことを確認する。片方だけ実装しても、もう片方のテストが落ちる。

### トランザクションと FOR UPDATE
レーティングは「読んで、計算して、書く」処理なので、同じユーザーの別の対局が同時に終局すると片方の更新が失われる（lost update）。トランザクション内で `FOR UPDATE` を掛けて直列化する。

**ロック順を ID の昇順に固定している。** 同じ2人を含む2局が同時に終局した場合、逆順にロックすると互いに待ち合ってデッドロックになる。

### 未知の `result` で処理を止めない
`score_from_result` は未知の値に `None` を返し、`apply_rating` はログを出して `Ok(())` で抜ける。ここでエラーにすると、**レーティングが計算できないだけで終局処理全体が失敗する**。対局の記録のほうが重要。

---

## 第2部: ランキング

### 認証不要にした
順位は公開情報であり、ログイン前のトップページからも見せられる。**認証を必須にすると、アプリを試す前に登録を求めることになる。**

トークンが付いていれば `me` に自分の順位を含める。`extract_user_id` が失敗しても 401 にせず `me: null` にするだけなので、**期限切れのトークンでランキングを開いても一覧は見える**（`invalid_token_does_not_break_the_ranking`）。

### 自分の順位を別エンドポイントにしない
「上位50件」と「自分の順位」を別々の API にすると、画面が2回叩くことになる。1回のレスポンスに両方を含め、上位に自分がいる場合はフロント側で重複表示を避ける。

`own_rank_is_returned_even_when_outside_the_limit` で `limit=1` でも自分の順位が返ることを確認している。**圏外のユーザーにとっては、こちらが主な情報**になる。

### 1局も終えていないユーザーを除外する
登録しただけの 1500 が並ぶと表として意味を成さない。`games_played > 0` で絞る。

判定は `users.games_played` カラムを持つほうが素直だが、カラムを増やすと更新箇所も増える。**まずは `games` を数える形にし、件数が問題になってから考える。** 集計は CTE 1箇所にあるので差し替えは容易。

### `RANK()` を使う
同レーティングは同順位。3位が2人なら次は5位（`DENSE_RANK()` なら4位）。順位表としては `RANK()` が一般的で、`same_rating_shares_the_same_rank` で確認している。

### CTE を定数として共有する
上位一覧と自分の行で**同じ順位定義**を使う必要がある。別々に書くと、片方だけ条件を変えたときに「一覧では3位なのに、自分の順位は4位」という食い違いが起きる。`RANKED_USERS` を `const` にして両方から使う。

---

## 実機で確認できたこと

同じ2人で2局続けた結果:

| | 白の変動 | 白のレーティング |
|---|---|---|
| 1局目 | +16 | 1500 → 1516 |
| 2局目 | +15 | 1516 → 1531 |

**2局目の伸びが小さくなっている。** 勝者のレーティングが上がったぶん期待勝率も上がり、同じ相手に勝っても得られる点数が減るという Elo の性質そのもの。単体テストで守っているのは「格上に勝つほうが伸びる」という不等式だが、実データで期待値の変化を確認できた。

## つまずいた点と教訓

### `AddrInUse` — 今度は古いプロセスが残っていた
task-33 では「ビルドしたのに再起動を忘れた」、今回は「起動済みのプロセスが残っていて新しいものが起動できない」。**同じ「バイナリと実行の分離」から来る、向きの違う2つの症状**。

```bash
lsof -ti:3000 | xargs kill
```

### エンドポイント数の検知が2度目の仕事をした
`EXPECTED_PATH_COUNT` を 11 → 12 に更新。task-32 に続き2度目で、いずれも「手作業で `paths(...)` に足す構造」に対する見張りとして機能した。

## 再現コマンド

```bash
# バックエンド
cd chess
docker compose up -d db
sqlx migrate info
sqlx migrate run
cargo test --test rating_test
cargo test --test ranking_test
cargo test
cargo clippy --all-targets -- -D warnings

lsof -ti:3000 | xargs kill    # 起動済みなら
cargo run

curl -s localhost:3000/users/ranking | jq
psql "$DATABASE_URL" -c "SELECT username, rating FROM users ORDER BY rating DESC"

# フロントエンド
cd ../frontend
npx tsc -b --force
npm run lint
npm run build
npm run dev

# 確認手順
# 1. ログアウト状態で /ranking → 一覧は見えるが「あなたの順位」は出ない
# 2. ログインして /ranking → 自分の行がハイライトされる
# 3. 1局も指していないユーザーで開く → 一覧に自分がいない
# 4. 対局を終えて /history → +16 / -16 が色付きで出る
# 5. 対局画面の「対戦相手」にレーティングが併記される
# 6. 375px 幅で表が崩れないか
```


## 次タスクへの引き継ぎ
- **Future Work は空になった。** 以降は新しく項目を立てる
- 持ち越している小さな課題:
  - K 値の可変化（暫定レーティング）— `users.games_played` を足せば `elo.rs` の変更だけで済む
  - 再接続時のイベント補完（task-22 / task-30 から持ち越し。`connected` に最終 `move_number` を載せる案あり）
  - `MoveHistory.tsx` の SAN 変換を `lib/uciToSan.ts` に寄せる（task-33 から持ち越し）
- ランキングは毎回集計している。ユーザーが増えたら `users.games_played` を持つか、集計をキャッシュする