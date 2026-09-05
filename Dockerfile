# Multi-stage build for minimal production image (~50MB)
# Cross-platform lanes, feature matrix, and env toggles: docs/src/architecture/vox-cross-platform-runbook.md

FROM rust:1.98.1-slim-bookworm AS builder
# Install system dependencies (required by openssl-sys and other C-bound crates)
RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*
# Install libdbus and GTK dependencies (required by wry/keyring on Linux)
RUN apt-get update && apt-get install -y libdbus-1-dev libglib2.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app

ARG VOX_CLI_FEATURES=

# Copy the entire workspace (filtered by .dockerignore)
# This is safer than individual COPY commands for a large monorepo with many embedded resources.
COPY . .

# Build only the CLI entrypoint. 
# --locked ensures we use the exact versions from Cargo.lock.
# -j 1 limits memory usage (critical for build stability on smaller runners/VPS).
RUN if [ -z "$VOX_CLI_FEATURES" ]; then \
      cargo build --profile dist -j 1 --locked -p vox-cli; \
    else \
      cargo build --profile dist -j 1 --locked -p vox-cli --features "$VOX_CLI_FEATURES"; \
    fi

# Runtime image — no Rust toolchain, just the binary + TLS certs
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/dist/vox /usr/local/bin/vox
# Tiny script for mesh compose worker
COPY examples/golden/mesh/noop.vox /opt/vox/mesh-noop.vox
COPY infra/containers/entrypoints/vox-entrypoint.vox /usr/local/bin/vox-entrypoint.vox

# VoxDB data volume mount point
VOLUME /root/.vox
EXPOSE 3000
EXPOSE 9847

# Health check via vox doctor --probe
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD vox doctor --probe

ENTRYPOINT ["vox", "run", "--interp", "/usr/local/bin/vox-entrypoint.vox"]
CMD ["vox", "mcp"]
