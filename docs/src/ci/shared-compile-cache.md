---
title: Shared Compile Cache (sccache + MinIO)
description: One S3-compatible compile cache shared by local agents, worktrees, self-hosted runner containers, and other LAN machines.
category: ci
---

# Shared Compile Cache (sccache + MinIO)

Every build surface used to keep its own cold cache: each git worktree had its
own `target/`, each ephemeral runner container leaned on a single-host Docker
volume, and GitHub-hosted lanes used the 10 GB Actions cache. The same crate
graph was recompiled in parallel by every agent tab. This page is the SSOT for
the shared cache that collapses those silos.

## Architecture

One MinIO container on the CI host serves an S3-compatible bucket
(`vox-sccache`) that **sccache** uses as its storage backend from every
surface:

| Surface | How it connects | Config source |
|---------|----------------|---------------|
| Local shells / agent tabs (Windows host) | `localhost:9000` | `%APPDATA%\Mozilla\sccache\config\config` (`[cache.s3]`) |
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

`--restart always` survives Docker Desktop restarts. The autoscaler probes
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

A fresh worktree, new agent tab, or recycled runner container starts at the
warm number instead of recompiling the workspace.

## Verifying

```bash
sccache --show-stats   # "Cache location  s3, name: vox-sccache" + hit rate
curl -s http://localhost:9000/minio/health/live   # 200 = server healthy
```
