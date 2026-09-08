# task-43 本番が5回連続で壊れていた件（builder/runtime の ABI 不一致）

## 何が起きたか

Render のデプロイ履歴を見ると、**#43 から #48 まで5回連続で赤**だった。

| PR | 内容 | 状態 |
|---|---|---|
| #42 | argon2 0.6 / rand 削除 | 緑 |
| **#43** | **shakmaty 0.30 に移行** | **赤** |
| #45 / #46 | cargo audit の追加 | 赤 |
| #47 / #48 | rust-toolchain.toml | 赤 |
| #49 | **builder/runtime の ABI 不一致修正** | 緑 |

**その間ずっと、テストは160件すべて緑、CI も緑だった。**

## 2段階の障害

### 第1段階: MSRV（task-42 で発見）

shakmaty 0.30.1 が rustc 1.95 以上を要求するが、Dockerfile は `rust:1.90` 固定。**ビルドが落ちる**。

```
error: rustc 1.90.0 is not supported by shakmaty@0.30.1
```

`rust:1.96` に上げて解決……したように見えた。

### 第2段階: GLIBC（今回）

`rust:1.96` にしたところ、**ビルドは通るが起動時に落ちる**ようになった。

```
./chess-server: /lib/x86_64-linux-gnu/libc.so.6:
version `GLIBC_2.38' not found
==> Exited with status 1
```

| | builder | 生成されるバイナリ |
|---|---|---|
| 変更前 | `rust:1.90`（bookworm ベース） | GLIBC 2.36 要求 |
| 変更後 | `rust:1.96`（**trixie ベース**） | GLIBC 2.38 要求 |
| runtime | `debian:bookworm-slim` | GLIBC 2.36 まで |

**`rust:<version>` のようにベース OS を明示しないタグは、その時点の最新安定版 Debian を使う。** 1.90 と 1.96 の間に Debian が bookworm から trixie に世代交代しており、ベースイメージが静かに入れ替わっていた。

GLIBC は**前方互換のみ**（古い環境でビルドしたバイナリは新しい環境で動くが、逆は動かない）。ビルド環境のほうが新しいと、実行環境で必要なシンボルが見つからない。

## 修正

builder のベース OS を明示的に固定し、runtime と同じ Debian 世代に揃える。

```diff
- FROM rust:1.96 AS builder
+ FROM rust:1.96-bookworm AS builder
```

あわせて、使っていない `libssl-dev` を runtime から削除した。`sqlx` は `runtime-tokio-rustls` を使っており OpenSSL に依存しない。

```diff
- apt-get install -y libssl-dev ca-certificates
+ apt-get install -y --no-install-recommends ca-certificates
```

## なぜ気づけなかったか

### `docker compose build` の成功で判断していた

task-42 で `rust:1.96` に上げたとき、**ビルドが通ったことをもって「解決した」と判断した**。しかし起動はしていない。

```
ビルドが通る ≠ バイナリが動く
```

このプロジェクトで繰り返し出ているパターンの、最も深いところで起きた版。

| 段階 | 通るもの | 通らないもの |
|---|---|---|
| コンパイル | 型が合う | 実行時の型不一致（ENUM キャスト） |
| テスト | ロジック | 環境依存（マイグレーション適用漏れ） |
| **Docker ビルド** | **リンク** | **実行時の共有ライブラリ解決** |

### デプロイの失敗を見ていなかった

Render のデプロイ履歴は見に行かないと分からない。**CI が緑ならデプロイも成功していると暗黙に仮定していた。** `workflow_run` で「CI 緑のときだけデプロイする」仕組みは作ったが、**デプロイ自体の成否は誰も見ていなかった**。

## 再発防止

### 起動確認をビルド確認に含める

```bash
docker compose up -d
sleep 5
curl -f http://localhost:3000/health || docker compose logs app
```

**`/health` が返ることまで確認する。** `docker compose build` の成功だけでは不十分。

```
{"status":"ok"}
```

### Dockerfile のベースイメージにバージョンを明示する

```
rust:1.96          ← ベース OS が暗黙。世代交代で静かに変わる
rust:1.96-bookworm ← 明示。runtime と揃えられる
```

**`FROM` にバージョンを書いていても、それだけでは固定されていない。** 言語のバージョンと OS のバージョンは別。

### 検討: デプロイの失敗を通知する

Render のデプロイ失敗を GitHub や Slack に通知する仕組みがあれば、5回も気づかずにいることはなかった。Render の Webhook か、Deploy Hook のレスポンスを見る形が考えられる（引き継ぎ）。

## rust-toolchain.toml との関係

**`rust-toolchain.toml` は今回の問題を解決しない。** あれが担保するのは Rust コンパイラのバージョンであって、ベース OS の世代ではない。

ただし**組み合わせると噛み合う**。

| 決めるもの | 場所 |
|---|---|
| Rust のバージョン | `rust-toolchain.toml`（1箇所） |
| ベース OS の世代 | `Dockerfile` の `-bookworm` サフィックス |

`FROM rust:1.96-bookworm` の `1.96` 部分は速度のためだけで、`rust-toolchain.toml` が正しさを担保する。**`-bookworm` の部分こそが、Dockerfile で明示しなければならない情報。**

Rust を上げるときは `rust-toolchain.toml` だけを変え、`FROM` は `-bookworm` を保ったまま数字を合わせる（合わせなくても動くが遅くなる）。

## 確認

```bash
# ローカル
docker compose up -d
curl -f http://localhost:3000/health
# {"status":"ok"}

# 本番（Render のログ）
# INFO chess_server: migrations applied
# INFO chess_server: listening on 0.0.0.0:10000
# ==> Your service is live 🎉
```

## 次タスクへの引き継ぎ
- **デプロイの失敗を検知する仕組みが無い。** Render の Webhook 等で通知できないか検討する
- `HEALTHCHECK` が Dockerfile に無い。追加すればコンテナ単体でも異常を検知できる
- Future Work: MFA（TOTP）、K 値の可変化、再接続時のイベント補完、レーティング推移のグラフ