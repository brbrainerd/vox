# Multi-stage build for minimal production image (~50MB)
# Usage:
#   docker build -t vox .
#   docker run -e GEMINI_API_KEY=... -p 3000:3000 vox

FROM rust:1.80-slim AS builder
WORKDIR /app

# Cache dependency layer
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p vox-cli \
    && strip /app/target/release/vox

# Runtime image — no Rust toolchain, just the binary + TLS certs
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/vox /usr/local/bin/vox

# VoxDB data volume mount point
VOLUME /root/.vox
EXPOSE 3000

# Health check via vox doctor (non-interactive)
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD vox doctor 2>&1 | grep -q "All checks passed" || exit 1

CMD ["vox", "mcp"]
