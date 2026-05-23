---
title: "Docker image baselines (D05)"
description: "How to record cold-start and health-check timing for Vox OCI images"
category: "Language Reference"
last_updated: "2026-03-25"

schema_type: "TechArticle"
---

# Docker image baselines

**Purpose (D05):** track regressions in image size, layer cache reuse, and **`vox doctor --probe`** latency inside containers.

## Recommended probes

1. **Build** (from repo root):  
   `docker build -t vox:probe .`  
   `docker build -t vox:populi -f infra/containers/Dockerfile.populi .`
2. **Cold start:**  
   `docker run --rm vox:probe vox doctor --probe` — exit code **0** when the toolchain inside the image passes default doctor checks.
3. **Healthcheck simulation:**  
   `docker run --rm vox:probe sh -c 'time vox doctor --probe'`

Record wall times and image sizes (`docker image ls`) when changing `Dockerfile`, Rust toolchain pins, or Debian base images. CI jobs validate Compose and image smoke only; trend capture is **operator-local** unless promoted to a benchmark workflow later.

## Related

- [Deployment compose SSOT](deployment-compose.md)
- [Cross-platform runbook](../archive/research-2026-q1/vox-cross-platform-runbook.md)


