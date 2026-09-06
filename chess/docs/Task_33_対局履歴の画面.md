# task-33 対局履歴の画面（一覧・棋譜再生）

## ゴールと完了条件
- 終了した対局を一覧で見返せる `/history`
- 棋譜を1手ずつ再生できる `/games/:id/review`
- 完了条件: 履歴から対局を選び、初期局面から最終局面まで送って戻せること

task-32 で作った `GET /users/me/games` を使う。実装中に既存の `GET /games/:id` のバグを発見したため、その修正も本タスクに含む（後述）。

## 追加・変更したもの

### フロントエンド
| ファイル | 内容 |
|---|---|
| `src/api/history.ts` | `getMyGames(token, {limit, offset})` |
| `src/lib/uciToSan.ts` | UCI 列 → SAN 列の変換 |
| `src/pages/HistoryPage.tsx` | 一覧 + ページング |
| `src/pages/ReviewPage.tsx` | 盤面再生 + 棋譜リスト + キーボード操作 |
| `src/styles/global.css` | 履歴・再生画面のスタイル（`main.tsx` が読み込むのはこちら。`App.css` は未使用） |
| 配線 | `types/index.ts` / `App.tsx` / `LobbyPage.tsx` |

### バックエンド
| ファイル | 内容 |
|---|---|
| `src/models.rs` | `GameDetailRow` に `fen: String` を追加 |
| `src/routes/game.rs` | `position_from_fen` ヘルパーを追加し、`get_game` を DB フォールバック対応に |
| `tests/game_test.rs` | 3件追加 |

## 設計判断の根拠

### 再生に `fen_after` をそのまま使う
`moves` テーブルは各手の直後の局面を `fen_after` として保持している（task-22）。そのため再生は**選ばれた手の `fen_after` を `ChessBoard` に渡すだけ**で済み、chess.js で初手から再現する必要がない。

当時「棋譜として見せるならSANが自然」という理由で SAN 変換をフロントに置いたが、**局面そのものは DB に持たせておいた**。この設計がここで効いた。手数が増えても再生のコストは一定。

### 読み取り専用の盤面に専用コンポーネントを作らない
`ChessBoard` は `isMyTurn` でドラッグの可否を制御しているので、`isMyTurn={false}` を渡せばそのまま閲覧専用になる。`onPieceDrop` / `onPromotionNeeded` は no-op を渡す。表示用のコンポーネントを別に作ると、盤面の見た目を変えたときに2箇所直すことになる。

### `outcome` をサーバーから受け取る
「自分が勝ったか」の判定はフロントでもできるが、`result`（盤面視点）と自分の色から導く処理を画面側に置くと、履歴画面と他の画面で判定が食い違いうる。task-32 で domain 層の純粋関数として実装し**単体テストで守られている**ので、フロントはそれを表示するだけにした。

### ページングを offset 方式にした
新しい順に固定で、対局が途中で挿入されることもないため cursor 方式にする理由が薄い。総件数を返していないので「次へ」は `list.length === PAGE_SIZE` で判定する。

### 矢印キーでの移動
棋譜を見返すときは連続で手を送るので、クリックより速い。`Home` / `End` で最初と最後にも飛べる。

---

## 途中で見つかった既存バグ: 終了済み対局が取得できない

### 症状
再生画面が「対局が見つかりません」で止まった。棋譜（`GET /games/:id/moves`）は取得できているのに、対局情報だけが 404 だった。

```bash
curl -i localhost:3000/games/<終了済みのid>
# HTTP/1.1 404 Not Found
# {"type":"/problems/not-found", ..., "detail":"対局が見つかりません"}
```

### 原因
`get_game` は2段構えになっていた。

```rust
let row = /* DB から参加者・状態を取得 */
    .ok_or_else(|| AppError::NotFound(...))?;   // ここは通る

let position = games.get(&id)
    .ok_or_else(|| AppError::NotFound(...))?;   // ここで落ちていた
```

進行中の対局はメモリ（`AppState.games`）で管理し、終局時に削除する設計のため、**DB に行があるのに局面が無い**状態になる。task-27 の `move_after_resign_returns_404` が裏付けていた挙動が、そのまま参照系にも及んでいた。

**再生画面に限った問題ではない。** 終局後に対局画面をリロードしても同じ 404 になり、サーバー再起動後は進行中の対局まで取得できなくなっていた。

### 修正
`games` テーブルは `fen` を保持しているので、メモリに無ければ**DBの FEN から局面を復元する**。

```rust
let position = {
    let games = state.games.read().await;
    match games.get(&id) {
        Some(p) => p.clone(),
        None => position_from_fen(&row.fen)?,
    }
};
```

**なぜ「終局時にメモリへ残す」ではないのか**: 残す案もあるが、メモリ使用量が対局数に比例して増え続け、削除の契機を別途決める必要が出る。DB に `fen` がある以上、必要になったときに復元するほうが状態の持ち方として単純。

**なぜ FEN の解析失敗を 500 にするのか**: DB に保存された FEN が壊れているのはサーバー側の異常であり、クライアントの入力に起因しない。400 を返すと「リクエストが悪い」という誤った説明になる。

### 追加したテスト

| テスト | 内容 |
|---|---|
| `finished_game_is_still_retrievable` | 投了後も 200 で最終局面が返る |
| `checkmated_game_reports_game_over` | 復元した局面でも `is_game_over` / `is_check` が効く |
| `unknown_game_returns_404` | 存在しない対局は従来どおり 404 |

## つまずいた点と教訓

### バイナリを再起動していなかった
`cargo build` が成功したあとも 404 が続いた。動いていたのは修正前のプロセスだった。**Rust はビルドと実行が分かれているため、ビルド成功はデプロイ完了を意味しない。** ブラウザで確認する前に、サーバーを再起動したかを確認する。

### 参照系のテストは「終わった後」も試す
`game_test.rs` の既存9件は、いずれも**進行中の対局**に対する `GET /games/:id` しか通っていなかった。終局後に取得するケースが1件あれば、画面を作る前に見つかっていた。状態が遷移するリソースでは、**各状態でひととおり参照できるか**を確認する。

### 症状が出た場所と壊れている場所は別
再生画面を作って初めて表面化したが、バグは新しく書いたコードではなく既存の API にあった。新機能が動かないとき新しい側を疑うのは自然だが、**既存の API を新しい条件（終了済み）で叩いたのが初めてでは？** という観点も要る。

### 同じエラー文言を複数の分岐で使い回さない
「対局が存在しない」と「対局は存在するが局面が無い」に同じ `"対局が見つかりません"` を使っていたため、切り分けが遅れた。RFC 9457 化（task-28）で `type` を持つようにはなったが、どちらも `/problems/not-found` なので区別できなかった。

### 貼り付け時の断片混入（4度目）
`App.tsx` で `/games/:id` ルートの末尾が重複し、`LobbyPage.tsx` ではヘッダーの断片がコンポーネント関数の**外**（ファイル末尾）に貼られて `user` / `logout` が未定義になった。state.rs 全置換 / JSX の関数内混入 / `common/mod.rs` の関数置換に続くパターン。**貼り付け後は必ず `git diff` を見る。**

## 残った重複
`MoveHistory.tsx`（対局中の棋譜表示）も UCI → SAN の変換を内部に持っている。今回 `lib/uciToSan.ts` を切り出したので、**`MoveHistory.tsx` をこの関数を使う形に寄せると重複が消える**。既存の対局画面に手を入れることになるため今回は分けた。

## 再現コマンド

```bash
# バックエンド
cd chess
docker compose up -d db
cargo test --test game_test
cargo test
cargo clippy --all-targets -- -D warnings
# 修正を反映するには再起動が必要
cargo run

# フロントエンド
cd ../frontend
npx tsc --noEmit
npm run lint
npm run dev

# 確認手順
# 1. 対局を1局終わらせる（投了でよい）
# 2. ロビーの「対局履歴」から /history へ
# 3. 勝敗・相手・手数・日時が出ているか
# 4. 対局をクリック → 再生画面
# 5. ▶ で最後まで送り、◀ で初期局面まで戻せるか
# 6. 矢印キーでも動くか
# 7. 375px 幅で崩れないか
# 8. 終局後の対局画面をリロードしても 404 にならないか（今回の修正分）
```

## 次タスクへの引き継ぎ
- Future Work は「レーティング」1件のみ。`users` へのカラム追加・終局時の更新処理・表示の3箇所に手が入る
- `MoveHistory.tsx` の SAN 変換を `lib/uciToSan.ts` に寄せる（上記）
- 履歴一覧に総件数を返していないため、正確なページ数表示はできない。必要になれば `X-Total-Count` ヘッダか、レスポンスを `{items, total}` に変える