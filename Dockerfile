# syntax=docker/dockerfile:1

# ── Arguments ───────────────────────────────────────────────
ARG RUST_VERSION=1.83
ARG PACKAGE=aeterna

# ── Base: tooling layer (cached across builds) ──────────────
FROM rust:${RUST_VERSION}-bookworm AS base

RUN rustup default nightly

# Install system dependencies needed to compile native crates (openssl-sys, libgit2-sys, etc.)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-chef and sccache via pre-built binaries (seconds, not minutes)
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
      https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && cargo binstall --no-confirm cargo-chef sccache

ENV CARGO_INCREMENTAL=0
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache

WORKDIR /app

# ── Planner: extract dependency recipe ──────────────────────
FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Builder: compile deps, then source ──────────────────────
FROM base AS builder

ARG PACKAGE=aeterna

# 1) Cook dependencies only — this layer is cached until Cargo.toml/Cargo.lock change
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# 2) Build the application (only your source code recompiles)
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release --package ${PACKAGE} \
    && cp ./target/release/${PACKAGE} /usr/local/bin/app

# ── Runtime: minimal production image ───────────────────────
FROM debian:bookworm-slim AS runtime

ARG BUILD_DATE
ARG VCS_REF

LABEL org.opencontainers.image.title="Aeterna"
LABEL org.opencontainers.image.description="Universal Memory & Knowledge Framework for Enterprise AI Agent Systems"
LABEL org.opencontainers.image.source="https://github.com/kikokikok/aeterna"
LABEL org.opencontainers.image.licenses="Apache-2.0"
LABEL org.opencontainers.image.created="${BUILD_DATE}"
LABEL org.opencontainers.image.revision="${VCS_REF}"

RUN useradd -m -u 1000 -s /bin/bash aeterna

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/local/bin/app /usr/local/bin/app

RUN chown -R aeterna:aeterna /app

USER aeterna

ENV RUST_LOG=info
ENV AETERNA_CONFIG_PATH=/app/config

EXPOSE 8080
EXPOSE 9090

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/app", "status"]

ENTRYPOINT ["/usr/local/bin/app"]
