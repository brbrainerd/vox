---
title: Shared Compile Cache (sccache + MinIO)
description: One S3-compatible compile cache shared by local agents, worktrees, self-hosted runner containers, and other LAN machines.
category: "CI & Quality"
---

# Shared Compile Cache (sccache + MinIO)

Every build surface used to keep its own cold cache: each git worktree had its
own `target/`, each ephemeral runner container leaned on a single-host Docker
volume, and GitHub-hosted lanes used the 10 GB Actions cache. The same crate
graph was recompiled in parallel by every agent tab. This page is the SSOT for
the shared cache that collapses those silos.

## Local dev agents (recommended default)

This is the fastest path for anyone running builds locally — agent tabs, manual
`cargo build` invocations, or worktree-per-branch workflows.

### 1. Install sccache

```bash
cargo install sccache
```

### 2. Configure user-local cargo settings

Add the following to **`~/.cargo/config.toml`** (your user-level config, NOT
the tracked `.cargo/config.toml` in the repo root):

```toml
[build]
rustc-wrapper = "sccache"
incremental = false
```

**Why `incremental = false`?**
Incremental compilation stores per-crate intermediate state in the local
`target/` directory and **bypasses sccache entirely** — sccache only caches
whole-crate compilations. With incremental on, sccache records zero writes and
you get none of the warm-cache speedup. Mixed settings across machines also fork
the cache key space, so an incremental build on your laptop cannot reuse
artifacts written by a CI runner. Setting `CARGO_INCREMENTAL=0` (or
`incremental = false` in config) is always the right choice when sccache is
the wrapper.

**Why NOT the tracked `.cargo/config.toml`?**
Putting `rustc-wrapper = "sccache"` in the repo-tracked config forces every
contributor to have sccache installed — including people who only want to build
once for a quick PR review. The cross-worktree hit rate is 0% anyway: sccache
cache keys include the absolute path prefix, so artifacts from one worktree
directory are never reused by a different worktree directory (measured; see
[Measured](#measured-vox-bounded-fs--deps-wiped-target-dir-each-pass) below).
The user-local config gives you the speedup without imposing a dependency on
everyone else.

### 3. Verify the setup

```bash
vox ci build-cache-doctor
```

This command checks that sccache is on the `PATH`, that `CARGO_INCREMENTAL` is
not overriding the config-file setting, and that the cache backend (disk or
S3/MinIO) is reachable. Fix any reported warnings before starting a long build.

---

## Architecture

One MinIO container on the CI host serves an S3-compatible bucket
(`vox-sccache`) that **sccache** uses as its storage backend from every
surface:

| Surface | How it connects | Config source |
|---------|----------------|---------------|
| Local shells / agent tabs (Windows host) | S3 via env vars when MinIO is up; falls back to disk cache | `%APPDATA%\Mozilla\sccache\config\config` (`[cache.disk]` default + `SCCACHE_*` env) |
| Self-hosted runner containers | `host.docker.internal:9000` | env injected by `vox ci runner-scale` at spawn (`shared_cache_env`) |
| Other LAN machines | `http://<ci-host>:9000` | same sccache config file, LAN endpoint |
| GitHub-hosted lanes (`gate`, `cross-check`) | GitHub Actions cache service | `SCCACHE_GHA_ENABLED` in the workflow (cannot reach the LAN) |

The bucket allows **anonymous read/write on the LAN** so no credential
plumbing is needed in agent shells or containers. Trade-off: anyone on the LAN
can poison the compile cache; acceptable for a single-operator home network,
not for shared offices — switch to MinIO access keys there (admin credentials
live in `~/.vox/ci-cache.env`, never in the repo).

## Server lifecycle

```bash
# One-time (already provisioned):
docker volume create vox-sccache-s3
docker run -d --name vox-sccache-minio --restart always --memory=1g \
  -p 9000:9000 -p 9001:9001 -v vox-sccache-s3:/data \
  -e MINIO_ROOT_USER=... -e MINIO_ROOT_PASSWORD=... \
  minio/minio server /data --console-address :9001
# Bucket + anonymous policy (mc):
#   mc mb vox/vox-sccache && mc anonymous set public vox/vox-sccache
```

`--restart always` survives Docker/WSL2 engine restarts (this host uses the
WSL2-native Docker Engine, not Docker Desktop — see
[runner-autoscaling.md](runner-autoscaling.md)). The autoscaler probes
`127.0.0.1:9000` before each spawn; if MinIO is down, runner containers fall
back to the per-host disk volume (`SCCACHE_DIR=/cache/sccache`) baked into the
runner image — builds never fail because the cache is away.

## Rules that keep hit rates high

- **`CARGO_INCREMENTAL=0`** wherever sccache is the wrapper. Incremental
  artifacts are uncacheable; mixed settings also fork the cache keys (an
  agent compiling with incremental on cannot reuse what CI wrote).
- Cache keys include compiler version and flags: keep Rust pinned via the
  workspace toolchain so all surfaces agree (`rust-toolchain` SSOT).
- Env vars override the config file — a shell exporting `SCCACHE_*` vars
  diverges from the herd. Prefer the config file.

## Measured (vox-bounded-fs + deps, wiped target dir each pass)

| Pass | Cache state | Wall time | Hit rate |
|------|-------------|-----------|----------|
| cold | empty bucket | 2m36s | 0% |
| warm | populated | **39–47s** | **91.9%** |

A recycled runner container or new CI run (same absolute path) starts at the
warm number. **Local worktrees at different paths do not share cached
artifacts**: sccache cache keys include crate metadata beyond source text, so
`--remap-path-prefix` does not bridge them (measured: 0% cross-worktree hits
even with path normalization). Same-path repeated builds — a worktree rebuilt
after a `cargo clean` — do hit the cache at the warm rate.

## Verifying

```bash
sccache --show-stats   # "Cache location  s3, name: vox-sccache" + hit rate
curl -s http://localhost:9000/minio/health/live   # 200 = server healthy
```
