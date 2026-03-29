FROM rust:1.86-bookworm AS builder

WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY config ./config
COPY hands ./hands

RUN cargo build --release -p abot-cli

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /src/target/release/abot /usr/local/bin/abot
COPY config ./config
COPY hands ./hands

CMD ["abot", "--config", "config/abot.toml"]
