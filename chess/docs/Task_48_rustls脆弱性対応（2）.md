# task-48 rustls 脆弱性 RUSTSEC-2026-0346〜0349 対応

## ゴールと完了条件

- 間接依存の `rustls` を RUSTSEC-2026-0346〜0349 の修正版へ更新する
- 変更は `Cargo.lock` の解決結果だけにとどめる（`Cargo.toml` とアプリケーションコードは変更しない）
- 完了条件: `cargo audit`、fmt、Clippy、全テストが成功すること

## 背景

2026-10-14 に `rustls` の advisory が4件公表され、`main` を含むすべてのブランチで CI の `cargo audit` が失敗するようになった。PR の変更内容とは関係なく、Dependabot の PR も同じ理由で赤くなる。

| ID | 内容 | Severity |
|---|---|---|
| RUSTSEC-2026-0346 | クライアントセッションの `Debug` 実装が TLS の秘密情報を出力する | 4.0 (medium) |
| RUSTSEC-2026-0347 | ECH が拒否されたあと、バッファ済みのアプリケーションデータが外側のサーバーへ送られる | 2.3 (low) |
| RUSTSEC-2026-0348 | Brotli による証明書の展開で、認証前に最大 1 GiB を確保しうる | 3.7 (low) |
| RUSTSEC-2026-0349 | 認証されていない ServerHello により、以後の接続が古典的な鍵交換へダウングレードされうる | 3.7 (low) |

修正版はいずれも `0.23.46` 以上。

## 対応: rustls を 0.23.46 へ更新

```bash
cd chess
cargo update -p rustls
```

```text
Updating rustls v0.23.45 -> v0.23.46
```

`rustls` は task-45 と同じく SQLx の `runtime-tokio-rustls` 経由で入る間接依存。既存の依存制約内で解決できる patch update なので、`Cargo.lock` の2行だけが変わる。ほかのクレートは更新していない。

```text
rustls v0.23.46
└── sqlx-core v0.8.6
```

## 検証

一時的な Postgres 16 コンテナを立て、CI の backend ジョブと同じ手順を実行した。

```bash
cd chess

cargo tree -i rustls -e normal
cargo audit
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

結果:

- `cargo tree -i rustls`: `rustls v0.23.46`
- `cargo audit`: 終了コード0、脆弱性0件（更新前は同じ環境で4件を検出）
- `cargo fmt --check`: 成功
- `cargo clippy --all-targets -- -D warnings`: 成功
- `cargo test --all-targets`: 164件成功

CI の `Container smoke test`（`docker compose up` → `/health`）は、手元で別のコンテナがポート 3000 を使っていたためローカルでは実行していない。依存の解決結果だけの変更なので、PR の CI で確認する。

## 変更範囲

| ファイル | 内容 |
|---|---|
| `chess/Cargo.lock` | `rustls 0.23.46` へ更新 |
| `chess/docs/Task_48_rustls脆弱性対応（2）.md` | 判断と検証を記録 |
| `README.md` | task-48 と完了タスク数を反映 |

アプリケーションコード、API 契約、DB schema の変更はない。

## 完了条件

- [x] `rustls` を `0.23.46` へ更新
- [x] RUSTSEC-2026-0346 / 0347 / 0348 / 0349 を解消
- [x] 変更を `Cargo.lock` に限定
- [x] fmt、Clippy、全164テストが成功

## 運用上の教訓

task-45 から1か月足らずで、同じ `rustls` に新しい advisory が出た。`cargo audit` は公表された時点で CI を落とすので、**自分が何も変えていなくても CI が赤くなる**。

赤くなった PR を見たら、まず `main` で同じ失敗が出るかを確認する。出るなら、その PR に修正を混ぜず、`main` 向けの修正 PR を先にマージしてから元の PR を追従させる（task-45・47 と同じ方針）。
