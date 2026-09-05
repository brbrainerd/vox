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
#   - pnpm 11       → ci.yml `pnpm/action-setup@v6` version 11
#   - Runner 2.337.0 → matches the version GitHub forces via self-update on cold
#     start; keep this current or every ephemeral runner re-downloads on boot,
#     defeating "ready to take a job immediately" (see `Downloading N runner` in
#     the container's startup log to check for drift). A stale pin here doesn't
#     just cost a slow cold start: the runner starts the job on the old version,
#     downloads the update mid-job, then exits to apply it — and since this
#     container has no restart loop, the exit kills the job (2026-07-27 incident:
#     3 nightly workflows failed for days from exactly this).

ARG RUST_VERSION=1.96.0
ARG RUNNER_VERSION=2.337.0

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
#
# The GTK/WebKit/dbus/alsa -dev packages below aren't used by most crates
# directly — they satisfy `tauri = { features = ["test"] }` in
# crates/workspace-hack/Cargo.toml, which every workspace member (including
# CLI-only crates like vox-compiler) depends on for shared feature
# unification. That pulls gdk-sys/gtk-sys/webkit2gtk-sys/etc. into ANY
# workspace crate's build closure, even ones with zero GUI code — e.g.
# `cargo mutants -p vox-compiler` needs libgtk-3-dev purely as a side effect
# (2026-07-27 incident; see also the follow-up to fix workspace-hack itself
# so non-GUI crates stop paying this cost). libgtk-3-dev alone provides
# gdk-3.0/gdk-x11-3.0/gtk+-3.0/atk/gdk-pixbuf-2.0/pango/glib-2.0/gio-2.0/
# gobject-2.0's .pc files — no separate libgdk/libglib/libpango/libatk
# packages needed.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev libsqlite3-dev \
    curl git ca-certificates jq ripgrep bash sudo libicu74 \
    libglib2.0-dev \
    libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev \
    libappindicator3-dev libdbus-1-dev libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust toolchain (pinned).
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain "$RUST_VERSION" \
    --component rustfmt,clippy,llvm-tools-preview \
    && rustup show

# Node 24 + pnpm 11 (matches ci.yml / docs-quality.yml SSOT).
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
    && apt-get install -y nodejs \
    && npm install -g pnpm@11 \
    && rm -rf /var/lib/apt/lists/*

# Pinned CI tools + sccache (shared compiler cache for warm ephemeral starts).
RUN cargo install cargo-nextest cargo-llvm-cov cargo-hakari sccache --locked

# Now that sccache exists, route CI job compiles through it (shared /cache volume).
ENV RUSTC_WRAPPER=sccache

# GitHub Actions runner agent.
#
# The arch is derived from TARGETARCH rather than hardcoded to x64. Docker sets
# TARGETARCH automatically (amd64 / arm64); GitHub names the same two artifacts
# x64 / arm64, hence the one-line mapping. Without this the image only builds
# for x64, which means an Apple-silicon or Ampere host can only run it under
# emulation — and emulated compilation of a 136-crate workspace is slow enough
# to defeat the purpose of hosting a local runner at all.
ARG TARGETARCH
RUN mkdir -p /actions-runner && cd /actions-runner \
    && case "${TARGETARCH}" in \
         amd64) RUNNER_ARCH=x64 ;; \
         arm64) RUNNER_ARCH=arm64 ;; \
         *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
       esac \
    && curl -fsSL -o runner.tar.gz \
       "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz" \
    && tar xzf runner.tar.gz && rm runner.tar.gz \
    && ./bin/installdependencies.sh

# The container runs as root; the actions runner refuses to `run.sh` as root
# unless this is set (the original fleet set it via env).
ENV RUNNER_ALLOW_RUNASROOT=1

COPY infra/ci-runner/entrypoint.sh /entrypoint.sh
RUN sed -i 's/\r$//' /entrypoint.sh && chmod +x /entrypoint.sh

WORKDIR /workspace
ENTRYPOINT ["/entrypoint.sh"]
