FROM rust:1.96-bookworm AS builder

WORKDIR /app

# 依存クレートを先にビルドしてキャッシュ
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# アプリケーション本体
COPY . .

RUN touch src/main.rs \
    && cargo build --release


FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/chess-server ./chess-server

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-3000}/health" || exit 1

CMD ["./chess-server"]
