FROM rust:1.87-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY mk_core ./mk_core
COPY storage ./storage
COPY errors ./errors
COPY utils ./utils
COPY config ./config
COPY opal-fetcher ./opal-fetcher

RUN cargo build --release -p opal-fetcher --bin opal-fetcher

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/opal-fetcher /usr/local/bin/

ENV OPAL_FETCHER_HOST=0.0.0.0
ENV OPAL_FETCHER_PORT=8080
ENV RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["opal-fetcher"]
