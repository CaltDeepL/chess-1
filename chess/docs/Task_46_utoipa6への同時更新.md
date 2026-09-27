# task-46 utoipa 6 / utoipa-axum 0.3 への同時更新

## ゴールと完了条件

- `utoipa` と `utoipa-axum` を互換性のある major version の組み合わせへ更新する
- Dependabot PR #63 / #64 で発生した Rust のコンパイルエラーを解消する
- OpenAPI の生成内容と配信経路を既存テストで維持する
- 完了条件: fmt、Clippy、全テスト、`cargo audit` が成功すること

## 背景

Dependabot は次の major update を別々の PR として作成した。

| PR | 単独更新 | 結果 |
|---|---|---|
| #63 | `utoipa 5.5.0` → `6.0.0` | Backend と Container smoke test が失敗 |
| #64 | `utoipa-axum 0.2.0` → `0.3.0` | Backend と Container smoke test が失敗 |

`utoipa-axum 0.2` は `utoipa 5`、`utoipa-axum 0.3` は `utoipa 6` と組み合わせる必要がある。片方だけを更新すると依存グラフに `utoipa 5` と `utoipa 6` が同居し、同名でも異なる型として扱われる。

代表的なエラーは `OpenApiRouter::with_openapi` に渡す `OpenApi` の型不一致で、`RefOr<Schema>`、`Paths`、`routes!` にも波及した。CI の33件は独立した不具合ではなく、この1つの version mismatch が原因だった。

## 対応

両方の直接依存を同じ変更で更新した。

```diff
-utoipa = { version = "5", features = ["axum_extras", "uuid", "chrono"] }
-utoipa-axum = "0.2"
+utoipa = { version = "6", features = ["axum_extras", "uuid", "chrono"] }
+utoipa-axum = "0.3"
```

更新後の依存経路は1系統だけになる。

```text
utoipa v6.0.0
├── chess-server
└── utoipa-axum v0.3.0
    └── chess-server
```

アプリケーションコード、API 契約、DB schema の変更はない。既存の OpenAPI 統合テスト6件が通るため、仕様生成と `/openapi.json`、Swagger UI、Bearer auth 定義も維持されている。

`utoipa-axum 0.3` は内部マクロ依存を unmaintained の `paste 1.0.15` から `pastey 0.2.3` へ変更する。この更新によって、task-45 で残課題としていた RUSTSEC-2024-0436 の warning も依存グラフから消えた。

## 検証

```bash
cd chess

cargo tree -i utoipa@6.0.0
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo audit --no-fetch
```

結果:

- `utoipa 6.0.0` は直接依存と `utoipa-axum 0.3.0` の共通の1バージョンのみ
- format check、全 target の check、Clippy が成功
- 全164テストが成功（OpenAPI 統合テスト6件を含む）
- `cargo audit` は終了コード0
- `paste 1.0.15` / RUSTSEC-2024-0436 は除去

## 変更範囲

| ファイル | 内容 |
|---|---|
| `chess/Cargo.toml` | `utoipa 6` と `utoipa-axum 0.3` を同時指定 |
| `chess/Cargo.lock` | 対応する依存解決結果へ更新 |
| `chess/docs/Task_46_utoipa6への同時更新.md` | 原因、判断、検証結果を記録 |
| `README.md` | task-46 と完了タスク数を反映 |

## 完了条件

- [x] `utoipa` と `utoipa-axum` を互換 version へ同時更新
- [x] `utoipa` の二重依存と33件のコンパイルエラーを解消
- [x] OpenAPI の生成・配信・認証定義を維持
- [x] unmaintained の `paste` 依存を除去
- [x] fmt、check、Clippy、全164テスト、security audit が成功

## 運用上の教訓

密結合したクレートの major update は、Dependabot の個別 PR をそのままマージせず、互換 version の組み合わせを1本の PR で検証する。個別 PR #63 / #64 は統合 PR のマージ後に閉じる。
