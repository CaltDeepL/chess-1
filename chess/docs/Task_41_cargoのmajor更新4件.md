# task-41 Cargo の major 更新4件（argon2 / tower-http / rand / shakmaty）

## ゴールと完了条件
- axum 0.8 移行（task-39）の後に提案された Cargo の major 更新に対応する
- 完了条件: 全テストが緑、**既存ユーザーがログインできること**、**合法手判定・終局判定の意味が変わっていないこと**

| PR | 判断 |
|---|---|
| #36 tower-http 0.6.11 → 0.7.1 | そのままマージ（CI 緑） |
| #38 argon2 0.5.3 → 0.6.0 | 対応（コード変更あり） |
| #35 rand 0.8.8 → 0.10.2 | **依存ごと削除** |
| #37 shakmaty 0.27.3 → 0.30.1 | 対応（影響範囲が最大のため最後に） |

---

# 第1部: argon2 0.6

## 変更点

`SaltString` が廃止され、`hash_password` が salt を内部で自動生成するようになった。`PasswordHash` は `password_hash::phc::PasswordHash` に移動。

```diff
- use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
+ use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};

- let salt = SaltString::generate(&mut rand::thread_rng());
- let password_hash = Argon2::default().hash_password(payload.password.as_bytes(), &salt)?
+ let password_hash = Argon2::default().hash_password(payload.password.as_bytes())?
```

**salt の生成を呼び出し側に書かせない設計への変更**で、使い方を誤る余地が減っている。呼び出し側が salt を使い回す、乱数源を間違えるといった事故が構造的に起きなくなる。

## `rand` は上げるのではなく消えた

`rand::` への直接参照は `SaltString::generate` の2箇所だけだった。argon2 が salt を内部生成するようになったため、**このプロジェクトが `rand` を直接使う理由がなくなった**。

```bash
cargo remove rand
```

PR #35 は「対応した」のではなく「不要になった」。依存が1つ減ったのは純粋な改善で、`rand` 0.9 での `thread_rng()` → `rng()` の改名にも今後付き合わなくてよくなった。

**依存を上げるとき、そもそも要るのかを一度考える。**

## 最重要の確認: 既存ハッシュの互換性

argon2 のハッシュ形式（PHC string format）が変わっていると、**既存ユーザー全員がログインできなくなる**。デプロイして初めて分かる種類の障害で、影響が最大。

### 実機での検証方法

ローカル DB の既存ユーザー（2026-08-06 作成、argon2 0.5.3 でハッシュ化）で確認した。

| 試行 | 結果 | 意味 |
|---|---|---|
| 誤ったパスワード | **401** | ハッシュのパースは成功し、比較だけが失敗した |
| 正しいパスワード | **200 + JWT 発行** | 完全に検証できている |

**401 と 500 を区別したのが要点。** 単に「ログインできた」だけでは、失敗時にどの段階まで到達しているか分からない。形式が読めなければ `PasswordHash::new` で 500 になるので、401 が返ること自体が「パースは通っている」証拠になる。

### テストで固定する

`src/auth.rs` の単体テストに、**0.5.3 が実際に出力した PHC 文字列を固定で埋め込んだ**。

```rust
#[test]
fn old_argon2_hash_is_still_verifiable() {
    const STORED_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$...";
    const ITS_PASSWORD: &str = "menu test secret password";

    let parsed = PasswordHash::new(STORED_HASH).expect("旧バージョンのPHC文字列がパースできない");
    assert!(
        Argon2::default().verify_password(ITS_PASSWORD.as_bytes(), &parsed).is_ok(),
        "旧バージョンで作られたハッシュを検証できない"
    );
}
```

**実行時に生成すると意味がない。** 生成側も検証側も同じ実装になるため、形式が変わっても必ず通ってしまう。既存の `existing_short_password_can_still_log_in` がまさにそれで、**バージョン間の互換性は検証していなかった**。

パラメータ（m / t / p）は文字列の中に含まれているので、ライブラリの既定値が変わっても保存済みハッシュはその値で検証される——という点もこのテストが確認している。

**偽陽性でないことを確認した。** パスワードを意図的に誤った値に変えると失敗し、戻すと通ることを見た。固定文字列を埋め込むテストは、`PasswordHash::new` さえ通れば「何かしら検証している」ように見えるため、この確認をしないと意味が薄い。

（advisory lock の回帰テスト（task-36）、JWT の feature 指定漏れ（task-40）に続き3度目。**新しいテストは、直したはずの問題で赤くなることを確かめて初めて意味を持つ。**）

## つまずいた点: git の操作で混乱した

依存更新そのものより、**複数の作業が並行したときのブランチ操作**で時間を取られた。

### 何が起きたか

| # | 症状 | 原因 |
|---|---|---|
| 1 | `npm install` が ERESOLVE で失敗 | ブランチを消しても `package.json` の `typescript@^7.0.2` がワークツリーに残っていた |
| 2 | `git switch` が `untracked working tree files would be overwritten` で拒否 | 同じ md がブランチ側ではコミット済み、手元では未追跡 |
| 3 | `git pull` が同じ理由で拒否 | 同上 |
| 4 | 2つのコミットのメッセージが入れ違い | ヒアドキュメントがシェルに解釈され、`-F` ではなく手打ちで渡していた |
| 5 | `git add chess/src/auth.rs` が `pathspec did not match` | 既に `chess/` の中にいた |
| 6 | CI が rustfmt で落ちた | 別ブランチのはずが `auth.rs` を含んでいた |

### 教訓1: コミットしていない変更はブランチを移動しても付いてくる

```bash
git switch main            # M package.json と出ていた
git branch -D chore/typescript-7
npm install                # 失敗
```

ブランチを消しても、**そのブランチで変更したファイルは手元に残る**。`git switch` の出力に `M <file>` が並ぶのはその警告だが、見落としやすい。

**ブランチを切り替える前にコミットするか `git stash` する。** 切り替えた後は `git status` で何が付いてきたかを確認する。

### 教訓2: ファイル単位で戻す

```bash
git checkout -- package.json package-lock.json
```

`git checkout .` や `git restore .` を使っていたら、**同時に変更していた `chess/src/auth.rs`（jsonwebtoken の単体テスト8件）も消えていた**。複数の作業が混ざっているときは、範囲を明示して戻す。

### 教訓3: `--amend` の前に中身を見る

コミットメッセージが2本とも入れ違っていた。`git show --stat HEAD` で内容を確認していれば、コミットした直後に気づけた。

```bash
git log --oneline -1
git show --stat HEAD       # 内容とメッセージが合っているか
```

`4 files changed, 558 insertions` なら `chess/` 配下、`1 file changed, 6 insertions` なら `dependabot.yml`。**行数と件数を見るだけで、どちらのコミットか判別できる。**

### 教訓4: 長いコミットメッセージは `-F` かエディタで

ヒアドキュメントの途中でシェルに解釈され、`>....` のプロンプトが出たままメッセージ本文が貼られる事故が起きた。`gti` のタイプミスも混ざった。

**複数行のメッセージは、ファイルに書いて `-F` で渡すか、`git commit` でエディタを開く。** コマンドラインに直接埋め込まない。

### 教訓5: 混ざったブランチは作り直すほうが速い

CI が `auth.rs` の rustfmt で落ちた時点で、`dependabot.yml` だけのはずのブランチに余計なファイルが入っていることが分かった。**取り除くより、main から作り直すほうが確実で速い。**

```bash
git switch main && git pull
git switch -c chore/ignore-typescript-7-v2
git add .github/dependabot.yml     # 対象を明示
git commit -m "..."
```

`git add .` ではなく**対象を明示する**ことで、混入そのものを防げる。

### まとめ

いずれも「複数の作業が同時に走っていた」ことに起因する。task-40（jsonwebtoken）、TypeScript の検証、README の更新、Dependabot の設定が並行しており、**どのブランチに何が乗っているかを見失った**。

一度に1つの作業に絞るのが理想だが、依存更新は他の PR が次々来るので完全には避けられない。せめて **`git status` と `git show --stat` を挟む**習慣を持つ。

---

# 第2部: shakmaty 0.30

major が3つ跨ぐうえ、合法手判定・終局判定の中核なので最後に回した。

## 修正は3箇所だけだった

コンパイルエラーは2種類、実質3箇所。**いずれも参照の受け渡しの変更**で、判定ロジックには一切触れていない。

### `Position::play` が値渡しになった

```diff
- match position.clone().play(&mv) {
+ match position.clone().play(mv) {
```

`src/routes/game.rs:378` と `src/domain/outcome.rs:39` の2箇所。

### `Fen::from_position` が参照を取るようになった

```diff
  pub fn position_to_fen(position: &Chess) -> String {
-     Fen::from_position(position.clone(), EnPassantMode::Legal).to_string()
+     Fen::from_position(position, EnPassantMode::Legal).to_string()
  }
```

**これは改善。** 0.27 では値を要求されたため `clone()` が必須だったが、0.30 では参照で済む。`position_to_fen` は指し手のたびに呼ばれるので、局面のコピーが1回減る。

コンパイラは `&position.clone()` を提案してきたが、**`clone()` してから参照を取るのは無駄**。引数が既に `&Chess` を受けていたので、そのまま渡せばよい。**コンパイラの提案は「通る形」であって「正しい形」とは限らない。**

## 「コンパイルが通れば OK」ではなかった

shakmaty は他の依存と性質が違う。`axum` や `argon2` と違い、**このアプリの正しさそのものを担っている**。API が変わらなくても、内部の判定が変わっていれば「詰みなのに詰みと判定されない」といった形で現れうる。しかもコンパイルは通る。

確認は既存のテストに委ねた。

| テスト | 何を守るか | 結果 |
|---|---|---|
| `domain::outcome` 6件 | 詰み・ステイルメイト・駒不足の判定 | 緑 |
| `checkmate_test.rs` 5件 | Fool's mate / Scholar's mate の実際の進行 | 緑 |
| `illegal_move_is_rejected` | 合法手の判定 | 緑 |
| `malformed_uci_is_rejected` | 不正な UCI の拒否 | 緑 |
| `checkmate_is_broadcast` | 終局イベントの配信 | 緑 |

**この領域を手厚くテストしておいた投資が、そのまま回収された。** ルール判定をライブラリに任せる方針を採る以上、ライブラリの挙動を自前のテストで固定しておくことに意味がある。

逆に、コンパイルを通すために `expect` や型変換を挟んだ箇所は無い。**そういう箇所があれば、意味が変わっていないか個別に見る必要があった。**

---

## Dependabot 由来の対応が一巡した

task-38 で導入してから、提案された更新への対応が一通り終わった。

| PR | 対応 | タスク |
|---|---|---|
| minor/patch グループ（npm 9件 / uuid / actions 2件） | そのままマージ | 38 |
| axum 0.8 + utoipa-axum + tower-http + tokio-tungstenite | 4つ同時に上げる | 39 |
| jsonwebtoken 11 | feature 指定を追加 | 40 |
| typescript 7 | **見送り**（typescript-eslint が未対応） | 40 |
| tower-http 0.7 | そのままマージ | 41 |
| argon2 0.6 | 対応。既存ハッシュの互換性を確認 | 41 |
| rand 0.10 | **依存ごと削除**（不要になった） | 41 |
| shakmaty 0.30 | 対応 | 41 |

**8件の提案に対し、4通りの結末があった**（そのままマージ / まとめて対応 / 見送り / 削除）。「Dependabot が出した PR をマージする」だけの作業ではないことが、一巡して分かった。

## 結果

**160件**（ユニット 63 / 統合 97）が緑。`fmt` / `clippy -D warnings` もクリーン。

## 次タスクへの引き継ぎ
- Dependabot 由来の対応は完了。以降は通常の運用（週1でグループ化された PR をレビュー）
- **shakmaty の feature に `variant` がある。** 将来 Chess960 などを扱うなら有効化する
- Future Work: MFA（TOTP）、K 値の可変化、再接続時のイベント補完、レーティング推移のグラフ
