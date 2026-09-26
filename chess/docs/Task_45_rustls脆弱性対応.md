# task-45 rustls 脆弱性 RUSTSEC-2026-0285 対応

## ゴールと完了条件

- 間接依存の `rustls` を RUSTSEC-2026-0285 の修正版へ更新する
- `cargo audit` を失敗させていた、利用していない SQLx の MySQL / RSA 依存経路を無効化する
- 既存の PostgreSQL、migration、テスト機能を維持する
- 完了条件: `cargo audit`、fmt、Clippy、全テストが成功すること

## 背景

`cargo audit` で `rustls` の RUSTSEC-2026-0285 が検出された。TLS 1.3 の handshake message を誤った encryption level で受理し得る問題で、修正版は `0.23.45` 以上。

```text
Crate:    rustls
ID:       RUSTSEC-2026-0285
Severity: 5.3 (medium)
Solution: Upgrade to >=0.23.45
```

作業開始時のブランチでは `rustls 0.23.43`、最新 `main` では `0.23.44` が解決されていた。どちらも影響範囲に含まれるため、最終的に `0.23.45` へ固定更新した。

## 対応1: rustls を 0.23.45 へ更新

```bash
cd chess
cargo update -p rustls --precise 0.23.45
```

`rustls` は直接依存ではなく、SQLx の `runtime-tokio-rustls` 経由で入る。`Cargo.toml` に直接バージョンを追加せず、既存の依存制約内で `Cargo.lock` の解決結果だけを更新した。

```text
rustls v0.23.45
└── sqlx-core v0.8.6
```

## 対応2: SQLx の不要な既定機能を無効化

`rustls` 更新後の `cargo audit` では RUSTSEC-2026-0285 は消えたが、次に `rsa 0.9.10` の RUSTSEC-2023-0071 が失敗原因として残った。

```text
Crate:    rsa
Version:  0.9.10
ID:       RUSTSEC-2023-0071
Severity: 5.9 (medium)
Solution: No fixed upgrade is available
```

このアプリケーションは PostgreSQL だけを使う。一方、SQLx の既定 feature に含まれる `any` によって、利用していない MySQL 関連パッケージが監査対象になっていた。脆弱性を allowlist へ追加するのではなく、必要な feature を明示する方針に変更した。

```diff
-sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "macros"] }
+sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "macros", "migrate"] }
```

`cargo tree -i rsa` は `nothing to print` となり、RSA は有効な依存グラフから外れた。`Cargo.lock` にパッケージ情報が残っていても、実際に有効な feature graph には含まれないため `cargo audit` の脆弱性件数は0になる。

## つまずいた点: migrate の明示が必要

最初は `default-features = false` だけを追加し、既存の feature 一覧をそのまま使った。この状態では SQLx の既定 feature に含まれていた `migrate` まで無効になり、Clippy で次のエラーになった。

```text
error: `migrate` feature required
error[E0433]: could not find `migrate` in `sqlx`
```

影響したのは以下の2経路。

- 起動時の `sqlx::migrate!()`
- 統合テストの `#[sqlx::test(migrations = "./migrations")]`

そこで `migrate` を明示 feature に追加した。不要な `any` / MySQL 経路だけを外し、PostgreSQL と migration の機能は維持している。

## 検証

```bash
cd chess

cargo tree -i rustls --locked --offline
cargo tree -i rsa --locked --offline
cargo audit --no-fetch
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

結果:

- `cargo tree -i rustls`: `rustls v0.23.45`
- `cargo tree -i rsa`: 有効な依存なし
- `cargo audit`: 終了コード0、脆弱性0件
- `paste 1.0.15` の RUSTSEC-2024-0436 は既存の unmaintained warning として1件
- `cargo fmt --all -- --check`: 成功
- `cargo clippy --all-targets -- -D warnings`: 成功
- `cargo test --all-targets`: 164件成功

## 変更範囲

| ファイル | 内容 |
|---|---|
| `chess/Cargo.toml` | SQLx の既定機能を無効化し、必要な feature と `migrate` を明示 |
| `chess/Cargo.lock` | `rustls 0.23.45` へ更新 |
| `chess/docs/Task_45_rustls脆弱性対応.md` | 判断、検証、つまずいた点を記録 |
| `README.md` | task-45 と完了タスク数を反映 |

アプリケーションコード、API 契約、DB schema の変更はない。

## 完了条件

- [x] `rustls` を `0.23.45` へ更新
- [x] RUSTSEC-2026-0285 を解消
- [x] 未使用の RSA 依存経路を allowlist ではなく feature 設定で無効化
- [x] SQLx の PostgreSQL / macros / migration 機能を維持
- [x] fmt、Clippy、全164テストが成功

## 残課題

`paste 1.0.15` の RUSTSEC-2024-0436 は脆弱性ではなく unmaintained advisory で、現在の監査では allowed warning。直接依存ではないため、親依存の更新で除去可能になった時点で対応する。

CI 復旧のために advisory の ignore を増やしたり、無関係な依存更新を同じ差分へ混ぜたりしない。
