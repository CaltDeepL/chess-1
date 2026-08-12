FROM rust:1.90 AS builder

WORKDIR /app

# 依存クレートだけを先にビルドしてDockerレイヤーにキャッシュする。
# ソースコードを変更しただけの再ビルドでは、Cargo.toml/Cargo.lockが
# 変わらない限りこの層は再利用され、Renderの無料枠でもビルド時間が安定する。
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

COPY . .
# 依存クレートのビルド結果を無効化させず、アプリ本体のバイナリだけ再ビルドさせる
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/chess-server /app/chess-server

# ドキュメント目的のデフォルト値。実際のリッスンポートはPORT環境変数
# (Render等が動的に注入する)をmain.rs側で読んで決定するため、
# ここでのEXPOSEの値自体はコンテナの挙動に影響しない。
EXPOSE 3000
CMD ["./chess-server"]
