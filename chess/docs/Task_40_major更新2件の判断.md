# task-40 major 更新2件の判断（jsonwebtoken 11 / TypeScript 7）

## ゴールと完了条件
- `jsonwebtoken` を 9 から 11 に上げ、JWT の検証を単体テストで守る
- `typescript` 6 → 7 を上げるか判断する
- 完了条件: 全テストが緑。上げない判断をした場合は理由が記録に残っていること

task-39 で「単独で対応できるもの」として保留していた Dependabot の PR 2本（#28 / #32）。**片方は上げ、片方は上げなかった。**

| PR | 判断 | 理由 |
|---|---|---|
| #28 jsonwebtoken 9 → 11 | **上げる** | 影響は `Cargo.toml` の1行。テストで守れる |
| #32 typescript 6 → 7 | **見送る** | `typescript-eslint` が未対応。lint が起動しなくなる |

---

# 第1部: jsonwebtoken 11 への更新

## 変更内容

| ファイル | 内容 |
|---|---|
| `chess/Cargo.toml` | `jsonwebtoken = { version = "11", features = ["rust_crypto"] }` |
| `chess/src/auth.rs` | 単体テスト8件を追加 |

コードの変更は `Cargo.toml` の1行のみ。API の署名は変わっていない。

## v11 の変更点: 暗号バックエンドの明示が必須になった

v11 から `rust_crypto` か `aws_lc_rs` のどちらかを feature で有効にしないと、**署名・検証時に `CryptoProvider` が見つからず panic する**。

**コンパイルは通る。** feature の指定漏れは型エラーにならないため、実行して初めて分かる。

### `rust_crypto` を選んだ理由
このプロジェクトは HS256（`from_secret` による対称鍵）しか使っていない。RSA / EC に対応する `aws_lc_rs` は C++ ライブラリのビルドを伴い、**Docker のビルド時間が伸びる**。純粋 Rust 実装で足りる。

将来 RS256（公開鍵署名）に切り替えるなら `aws_lc_rs` を検討する。

## テストの通り方が原因を教えてくれた

feature 指定前の状態では、**146件のうち `sweep_endpoint_requires_the_token` だけが通った**。

この1件は sweep トークン（共有シークレット）のチェックで先に失敗して終わるため、**JWT の検証に到達しない**。他のテストはすべて `register_user` などでトークンを発行するので panic した。

「1件だけ通った」という事実が、失敗が JWT 経路に限定されていることを示していた。**どのテストが落ちたかだけでなく、何が通ったかにも情報がある。**

### 追加したテストでも同じ形が再現した
feature を外して `cargo test --lib auth` を実行すると、8件のうち **7件が panic し、`extract_user_id_rejects_a_missing_header` だけが通った**。

これもヘッダーが無い時点で `Err` を返すため、JWT の検証に到達しない。**統合テストで起きたことが、単体テストの層でそのまま再現している。**

## 追加した単体テスト

`exp` を検証しているテストが**1件も無かった**。`grep -rn "exp\b" tests/ src/auth.rs` で出たのは `issue_token` 内の1行のみ。

| テスト | 何を守るか |
|---|---|
| `valid_token_round_trips` | **暗号バックエンドが有効になっているか**（今回の件がそのまま再発検知される） |
| `expired_token_is_rejected` | `Validation::default()` の既定値が緩くなっていないか |
| `token_within_expiry_is_accepted` | 上の対。「常に拒否する」実装でも通ってしまうのを防ぐ |
| `token_signed_with_another_secret_is_rejected` | 署名の検証が効いているか |
| `tampered_token_is_rejected` | ペイロード改ざんを弾くか |
| `extract_user_id_reads_the_bearer_header` | ヘッダーの解析 |
| `extract_user_id_rejects_a_malformed_header` | `Bearer ` プレフィックスなし |
| `extract_user_id_rejects_a_missing_header` | ヘッダー欠落 |

### なぜ `Validation::default()` をテストで守るのか
`verify_token` は既定値に依存しており、**有効期限をどこまで厳しく見るかはライブラリ側が決めている**。バージョンを上げて既定値が変われば検証の厳しさが変わるが、コンパイルエラーにはならない。

緩くなれば期限切れトークンが通り、厳しくなれば正常なトークンが弾かれる。どちらも本番で初めて分かる種類の変化なので、テストで固定しておく。

### 「拒否する」テストには対になる「受け入れる」テストを置く
`expired_token_is_rejected` だけだと、`verify_token` が**常に `Err` を返す**実装でも通ってしまう。`token_within_expiry_is_accepted` と対で置くことで、拒否の条件が正しいことまで確認できる。

これは task-35 で `login_with_wrong_password_returns_401` が「意図した経路を通らないまま緑だった」件と同じ発想。**assert が通ることと、意図した状態を検証していることは別。**

## 再現コマンド

```bash
cd chess
docker compose up -d db
cargo test --lib auth      # 追加した8件のみ
cargo test                 # 全156件（ユニット 62 / 統合 94）
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

### feature 指定漏れの再現（実施済み）

```toml
jsonwebtoken = "11"        # features を外す
```

```
test result: FAILED. 1 passed; 7 failed
```

8件のうち7件が `CryptoProvider` の panic で落ち、`extract_user_id_rejects_a_missing_header` のみ通った。**追加したテストが今回の問題を確実に検知することの実証**。

feature を戻すと全件緑。task-36 で advisory lock の回帰テストを旧実装に戻して確認したのと同じ手順で、**新しいテストは、直したはずの問題で赤くなることを確かめて初めて意味を持つ**。


```bash
git commit -F docs/commits/task-40-chore-upgrade-jsonwebtoken-11.txt
```

---

# 第2部: TypeScript 7 を見送った判断

## 何が起きたか

```bash
npm install typescript@7
npm run lint
```

```
typescript-eslint does not support TS 7.0.
See also https://github.com/typescript-eslint/typescript-eslint/issues/10940
for tracking typescript-eslint's support for TS >=7.1
```

**`typescript-eslint` が TS 7.0 に未対応で、lint がそもそも起動しない。** 対応は 7.1 以降を待つ必要がある。

`npm install` の時点で予兆は出ていた。

```
npm warn peer typescript@">=4.8.4 <6.1.0" from typescript-eslint@8.69.0
npm warn ERESOLVE overriding peer dependency
```

peer dependency の範囲が `<6.1.0` で、7 は明確に範囲外。**警告のまま押し通せてしまう**ので、install が成功したことは何の保証にもならない。

`npx tsc -b` と `npm run build` は通った。型チェックとビルドだけを見ると問題なく見える。

## 見送った理由

| | 得るもの | 失うもの |
|---|---|---|
| TS 7 | コンパイル速度（Go 実装） | **型情報を使った lint 全体** |

このプロジェクトの規模（62モジュール、ビルド 130ms）では、コンパイル速度の改善は体感できない。一方 lint が止まれば CI の検査が1つ丸ごと落ちる。

**「上げられるから上げる」ではなく「上げて何が良くなるか」で判断する。** メジャーバージョンが出たこと自体は、上げる理由にならない。

## Dependabot に無視させる

放っておくと毎週同じ PR が来て、そのたびに同じ調査をすることになる。

```yaml
# .github/dependabot.yml の npm セクション
    ignore:
      # typescript-eslint が TS 7.0 に未対応（7.1 以降で対応予定）。
      # 上げると型情報を使った lint が起動しなくなる。
      # https://github.com/typescript-eslint/typescript-eslint/issues/10940
      - dependency-name: "typescript"
        versions: ["7.x"]
```

**`versions: ["7.x"]` としているので、8 が出れば再び提案される。** ただし 7.1 で typescript-eslint が対応した場合もこの設定では無視されるため、上記の issue を追う必要がある。

**無視する設定には必ず理由と、いつ見直すかを書く。** 理由の無い `ignore` は、`uuid = "=1.10.0"` のピン留めと同じで、後から解除してよいか分からなくなる（task-39）。

## つまずいた点

### ブランチを消しても `package.json` の変更は残る

```bash
git switch main            # M package.json と出ていた
git branch -D chore/typescript-7
npm install                # ERESOLVE で失敗
```

`package.json` に `typescript@^7.0.2` が書き込まれたまま main のワークツリーに残っており、そこから解決しようとして失敗した。**コミットしていない変更はブランチを移動しても付いてくる。**

```bash
git checkout -- package.json package-lock.json
```

ファイル単位で戻すことで、同時に変更していた `chess/` 側（task-40 の jsonwebtoken）に触れずに済んだ。**`git checkout .` や `git restore .` を使っていたら、jsonwebtoken の作業も消えていた。**

**教訓**: ブランチを切り替える前にコミットするか `git stash` する。切り替え後は `git status` で何が付いてきたかを確認する。

### `tsc -b` の実行場所

```
error TS6053: File '.../frontend/tsconfig.json' not found.
```

`tsconfig.json` はリポジトリ直下にある。`frontend/` はソースディレクトリでしかない（task-38 で `package.json` の場所を確認したときと同じ）。README の手順もリポジトリ直下で実行する形に統一する。

## 次タスクへの引き継ぎ
- **TypeScript 7 は typescript-eslint が対応したら再検討する。** issue #10940 を追う
- Dependabot 由来の対応はこれで完了。残りは Future Work のみ
- Future Work: MFA（TOTP）、K 値の可変化、再接続時のイベント補完、レーティング推移のグラフ
- **MFA を実装するなら、第1部の単体テストが土台になる。** TOTP の検証も同じ `auth.rs` に入るため、時刻に依存する検証をテストする形が既にできている