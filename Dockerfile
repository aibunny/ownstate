# syntax=docker/dockerfile:1
# ONNX Runtime's downloaded static archive needs GCC 14 C++ ABI symbols.
FROM rust:1.96.0-trixie AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY apps ./apps
COPY migrations ./migrations

# false builds the download-free mechanics image; fastembed remains the default.
ARG OWNSTATE_FASTEMBED=true
ARG CARGO_BUILD_JOBS=2
ENV CARGO_INCREMENTAL=0
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    case "$OWNSTATE_FASTEMBED" in \
      true) cargo build --locked --release -p ownstate-api -p ownstate-worker -p ownstate-mcp ;; \
      false) cargo build --locked --release --no-default-features -p ownstate-api -p ownstate-worker -p ownstate-mcp ;; \
      *) echo 'OWNSTATE_FASTEMBED must be true or false' >&2; exit 1 ;; \
    esac \
    && mkdir -p /out \
    && cp target/release/ownstate-api target/release/ownstate-worker target/release/ownstate-mcp /out/

FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3t64 libstdc++6 libgomp1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ownstate \
    && useradd --uid 10001 --gid ownstate --no-create-home --home-dir /app ownstate \
    && mkdir -p /app/.fastembed_cache \
    && chown -R ownstate:ownstate /app
WORKDIR /app
ENV OWNSTATE_EMBEDDING_CACHE_DIR=/app/.fastembed_cache
USER 10001:10001

FROM runtime AS worker
COPY --from=builder /out/ownstate-worker /usr/local/bin/ownstate-worker
# The worker currently handles SIGINT, whereas the API also handles SIGTERM.
STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/local/bin/ownstate-worker"]

FROM runtime AS mcp
COPY --from=builder /out/ownstate-mcp /usr/local/bin/ownstate-mcp
ENTRYPOINT ["/usr/local/bin/ownstate-mcp"]

FROM runtime AS api
COPY --from=builder /out/ownstate-api /usr/local/bin/ownstate-api
ENV OWNSTATE_HTTP_ADDR=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --start-period=300s --retries=60 \
    CMD curl --fail --silent http://127.0.0.1:8080/ready || exit 1
ENTRYPOINT ["/usr/local/bin/ownstate-api"]
