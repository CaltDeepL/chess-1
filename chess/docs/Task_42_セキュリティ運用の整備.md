# task-42 セキュリティ運用の整備（ブランチ保護・依存監査）

## ゴールと完了条件
- CI を通さずに main が変更されない状態にする
- 依存の脆弱性を push / PR のたびに検査する
- 完了条件: main への直接 push が拒否され、CI に監査が組み込まれていること

Dependabot 導入（task-38）で「依存の更新」は自動化されたが、**その周辺の運用に穴があった**。

## 1. ブランチ保護

### 見つかった穴
README には「CI が緑のときだけデプロイが走る」と書いてあり、`workflow_run` でその通りに実装されている。しかし**main への直接 push が可能だった**。実際、作業中に `git push origin main` が通ってしまった。

守られているものと守られていないものを整理すると:

| 守るもの | 仕組み | 状態 |
|---|---|---|
| CI が落ちた状態でデプロイされない | `workflow_run`（task-25） | あった |
| CI を通さないコードが main に入らない | ブランチ保護 | **無かった** |

後者が無いと、レビューもテストも経ずに main が変わり、**CI が赤いまま放置される**。デプロイは止まるので本番は守られるが、main が壊れた状態になる。

### Classic ではなく Ruleset を選んだ

GitHub には旧方式（Classic branch protection）と現行の Ruleset がある。

| | Classic | Ruleset |
|---|---|---|
| 位置づけ | 旧方式 | 現行 |
| 一時的な無効化 | 削除するしかない | **Active / Disabled を切り替えられる** |
| 対象 | パターン1つ | 複数パターン、タグにも対応 |

**一時的に無効化できることが実用上の差。** 緊急時にルールを外したいとき、Classic だと削除して作り直すことになる。

### 設定内容

| 項目 | 値 |
|---|---|
| Enforcement | Active |
| Target | default branch |
| Restrict deletions | 有効 |
| Block force pushes | 有効 |
| Require a pull request | 有効 |
| └ Required approvals | **0** |
| Require status checks | `Backend (Rust)` / `Frontend (React)` |

**Required approvals を 0 にするのが要点。** 1人で開発しているので、1以上にすると自分の PR を自分で承認できず、何もマージできなくなる。

### 効いていることの確認

```
GH013: Repository rule violations found for refs/heads/main.
- Changes must be made through a pull request.
- 2 of 2 required status checks are expected.
```

## 2. 依存の脆弱性監査

### Dependabot との違い

| | 契機 | 対象 |
|---|---|---|
| Dependabot version updates | 新しいバージョンが出たとき | 直接依存 |
| Dependabot security updates | 脆弱性が公開されたとき | 直接・推移的依存 |
| `cargo audit`（CI） | **push / PR のたび** | `Cargo.lock` 全体 |

**3つ目の価値は「今この時点の lock ファイルが安全か」を毎回確認できること。** Dependabot は PR を出すだけなので、放置すれば脆弱なまま。CI に入れれば、残っている限り赤くなり続ける。

### CI への追加

```yaml
# Backend (Rust)
      - name: Security audit
        run: |
          cargo install cargo-audit --locked
          cargo audit
        working-directory: chess

# Frontend (React)
      - name: npm audit
        run: npm audit --audit-level=moderate
```

`--locked` は `cargo-audit` 自身の依存更新で壊れるのを防ぐため。`cargo install` に1〜2分かかるので、遅ければ `rustsec/audit-check` への差し替えを検討する。

**テストの後に置く。** 先に置くと、脆弱性で落ちたときにテスト結果が分からなくなる。

## 3. RUSTSEC-2023-0071（rsa）への対応

### 何が出たか

```
Crate:    rsa 0.9.10
Title:    Marvin Attack: potential key recovery through timing sidechannels
Severity: 5.9 (medium)
Solution: No fixed upgrade is available!
```

**修正版が存在しない**（2023-11 に報告、以降未修正）。「上げれば直る」種類の問題ではない。

### 経路の特定に手間取った

最初 `cargo tree -i rsa` は `jsonwebtoken` 経由と表示した。task-40 で選んだ `rust_crypto` feature が RSA 実装を引き込んでいた。

`aws_lc_rs` に切り替えたところ `cargo tree` からは消えたが、**`cargo audit` は報告し続けた**。両者が見ているものが違うため。

| | 見ているもの |
|---|---|
| `cargo tree` | 現在のターゲットで**実際にビルドされる**依存 |
| `cargo audit` | **`Cargo.lock` に記載されている**依存すべて |

`cargo update` でも `cargo generate-lockfile` でも消えなかった。原因は `jsonwebtoken` の `default` feature が `use_pem` を有効にしていたこと。

```diff
- jsonwebtoken = { version = "11", features = ["aws_lc_rs"] }
+ jsonwebtoken = { version = "11", default-features = false, features = ["aws_lc_rs"] }
```

**`cargo add --features` は既存の features に追加するだけで、`default` は無効にならない。** これで依存が9個減った（294 → 285）が、`rsa` はまだ残った。

最終的に `Cargo.lock` を直接読んで親を特定した。

```bash
grep -n 'rsa' Cargo.lock       # 定義と参照1件のみ
sed -n '1840,1875p' Cargo.lock # 親を確認 → sqlx-mysql
```

**`sqlx-mysql` 経由だった。** `sqlx` の features に `postgres` しか指定していなくても、`Cargo.lock` には mysql / sqlite 用のエントリが載る。これは sqlx 側の構造なので、こちらでは消せない。

### ignore する判断

`cargo tree -i rsa --target all` が「nothing to print」を返す——**どのターゲットでもビルドされない**。ビルドされないものは実行もされないため、RSA の復号処理にタイミング攻撃を仕掛ける経路が存在しない。

`chess/.cargo/audit.toml` に理由と見直しの契機を書いて無視する。

**「無視する」ことより「なぜ無視してよいのか」を書き残すことが本体。** 理由の無い ignore は、理由の消えたバージョンピンと同じ末路をたどる（task-39 の `uuid = "=1.10.0"`）。

### `paste`（unmaintained）は書かない

脆弱性ではなく「メンテナンス終了」の通知で、`cargo audit` の既定では失敗扱いにならない。

`--deny warnings` にすれば失敗させられるが、**自分では対処できない依存（sqlx などが使っているもの）で CI が赤くなり続ける**。そうなると「赤いのが普通」になり、本物の脆弱性を見逃す。既定のままとした。

## 4. 副産物: 本番デプロイが落ちる状態だった

`docker compose build` を試したところ、ビルドが失敗した。

```
error: rustc 1.90.0 is not supported by the following package:
  shakmaty@0.30.1 requires rustc 1.95
```

**task-41 の shakmaty 0.30 移行で、本番のビルドが壊れていた。** Dockerfile が `rust:1.90` 固定だったため。

| 環境 | Rust | 結果 |
|---|---|---|
| ローカル | 1.96 | 通る |
| CI | `dtolnay/rust-toolchain@stable` | 通る |
| **Docker（本番）** | **1.90 固定** | **落ちる** |

**ローカルと CI が両方通るので、`docker compose build` を実行するまで気づけなかった。** 依存の MSRV（最低必要 Rust バージョン）が上がると、固定バージョンの環境だけが取り残される。

`rust:1.96` に上げて解決。

**引き継ぎ**: CI が `stable`、Docker が固定という乖離は今後も同じ問題を起こす。`rust-toolchain.toml` を置いて両方から参照すれば、CI で検知できるようになる。

## 5. `.dockerignore` が未追跡だった

最初の `docker compose build` で**14GB のコンテキスト転送に141秒**かかった。`target/` が含まれていた。

`.dockerignore` 自体は存在していたが git に追跡されておらず、**CI やデプロイでは効いていなかった**可能性がある。追跡した後は 549kB / 0秒。

## 結果

| 項目 | 状態 |
|---|---|
| main への直接 push | 拒否される |
| `cargo audit` | exit 0（vulnerability 0件、warning 1件） |
| `npm audit` | 0件 |
| CI | 緑（3分37秒） |
| Docker ビルド | 通る |

## 次タスクへの引き継ぎ
- **`rust-toolchain.toml` で CI と Docker の Rust バージョンを揃える**（上記）
- `cargo install cargo-audit` の1〜2分が CI に乗っている。遅ければ `rustsec/audit-check` に差し替える
- Dependabot security updates（リポジトリ設定側のトグル）が有効か確認する
- 候補: CodeQL（公開リポジトリなら無料）、dependency-review-action（PR で新規追加される依存を検査）
- Future Work: MFA（TOTP）、K 値の可変化、再接続時のイベント補完、レーティング推移のグラフ