# Reproducible self-hosted CI runner image — `vox-ci-runner-local` / GHCR.
#
# This is the SSOT for the image the `vox ci runner-scale` autoscaler spawns and
# that backs the `[self-hosted, linux, x64, docker, browser]` merge gate. It bakes
# in the GitHub Actions runner agent + the vox toolchain so ephemeral runners are
# ready to take a job immediately, and configures sccache against a shared cache
# volume so cold starts stay warm.
#
# Build (run from repo root):
#   docker build -t vox-ci-runner-local:latest -f infra/ci-runner/Dockerfile .
#
# Version SSOTs (keep aligned with the rest of the repo):
#   - Rust 1.96.0   → root Dockerfile (`rust:1.96.0-slim-bookworm`) / rust-toolchain.toml / contracts SSOT
#   - Node 24       → docs-quality.yml `node-version: 24`
#   - pnpm 9        → ci.yml `pnpm/action-setup@v6` version 9
#   - Runner 2.334.0 → matches the deployed fleet (`Current runner version`)

ARG RUST_VERSION=1.96.0
ARG RUNNER_VERSION=2.334.0

FROM ubuntu:24.04

ARG RUST_VERSION
ARG RUNNER_VERSION

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=always \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    # Shared sccache dir lives on the mounted cache volume (see runner_scale.rs).
    # NOTE: RUSTC_WRAPPER=sccache is set LATER (after sccache is installed) — setting
    # it here would make the toolchain/tool builds below try to invoke a not-yet-
    # existent sccache as the rustc wrapper.
    SCCACHE_DIR=/cache/sccache

# System packages matching the self-hosted fleet baseline + runner deps.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev libsqlite3-dev \
    curl git ca-certificates jq ripgrep bash sudo libicu74 \
    libglib2.0-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust toolchain (pinned).
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain "$RUST_VERSION" \
    --component rustfmt,clippy,llvm-tools-preview \
    && rustup show

# Node 24 + pnpm 9 (matches ci.yml / docs-quality.yml SSOT).
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
    && apt-get install -y nodejs \
    && npm install -g pnpm@9 \
    && rm -rf /var/lib/apt/lists/*

# Pinned CI tools + sccache (shared compiler cache for warm ephemeral starts).
RUN cargo install cargo-nextest cargo-llvm-cov cargo-hakari sccache --locked

# Now that sccache exists, route CI job compiles through it (shared /cache volume).
ENV RUSTC_WRAPPER=sccache

# GitHub Actions runner agent.
RUN mkdir -p /actions-runner && cd /actions-runner \
    && curl -fsSL -o runner.tar.gz \
       "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz" \
    && tar xzf runner.tar.gz && rm runner.tar.gz \
    && ./bin/installdependencies.sh

# The container runs as root; the actions runner refuses to `run.sh` as root
# unless this is set (the original fleet set it via env).
ENV RUNNER_ALLOW_RUNASROOT=1

COPY infra/ci-runner/entrypoint.sh /entrypoint.sh
RUN sed -i 's/\r$//' /entrypoint.sh && chmod +x /entrypoint.sh

WORKDIR /workspace
ENTRYPOINT ["/entrypoint.sh"]
