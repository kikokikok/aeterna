ARG RUST_VERSION=1.83

FROM rust:${RUST_VERSION}-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG BUILD_DATE
ARG VCS_REF

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --package aeterna

FROM debian:bookworm-slim AS runtime

ARG BUILD_DATE
ARG VCS_REF

LABEL org.opencontainers.image.title="Aeterna"
LABEL org.opencontainers.image.description="Universal Memory & Knowledge Framework for Enterprise AI Agent Systems"
LABEL org.opencontainers.image.source="https://github.com/kikokikok/aeterna"
LABEL org.opencontainers.image.licenses="Apache-2.0"
LABEL org.opencontainers.image.created="${BUILD_DATE}"
LABEL org.opencontainers.image.revision="${VCS_REF}"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 -s /bin/bash aeterna

WORKDIR /app

COPY --from=builder /app/target/release/aeterna /usr/local/bin/aeterna

RUN chown -R aeterna:aeterna /app

USER aeterna

ENV RUST_LOG=info
ENV AETERNA_CONFIG_PATH=/app/config

EXPOSE 8080
EXPOSE 9090

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/aeterna", "status"] || exit 1

ENTRYPOINT ["/usr/local/bin/aeterna"]
CMD ["serve"]
