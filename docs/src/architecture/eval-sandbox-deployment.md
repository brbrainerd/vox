---
title: "Eval sandbox deployment (Coolify)"
description: "Public MCP HTTP eval gateway at eval.voxlang.org — image, compose, DNS, Coolify API sync."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Operators need one place for eval stack topology and provisioning."
audience: ["contributors", "operators"]
related:
  - docs/src/ci/deploy-contract.md
  - vox-eval.compose.yml
---

# Eval sandbox deployment (Coolify)

The **eval sandbox** is a minimal **`vox mcp`** HTTP gateway that exposes **`/v1/eval`** for the static docs playground. It does **not** mount a workspace or provider keys. Production URL: **`https://eval.voxlang.org`**. The documentation site (**`https://voxlang.org`**) is separate (GitHub Pages / mdBook), not this stack.

## Compose and container image

- **SSOT compose:** [`vox-eval.compose.yml`](../../../vox-eval.compose.yml) at the repository root ([`docker/vox-eval.compose.yml`](../../../docker/vox-eval.compose.yml) is a mirror for compose invocations rooted in **`docker/`**).
- **Image:** `ghcr.io/vox-foundation/vox-eval:latest`, built by [`.github/workflows/docker-eval.yml`](../../../.github/workflows/docker-eval.yml) with **`VOX_CLI_FEATURES=mcp-server`** (matches **`command: ["vox", "mcp"]`**).
- **Health:** Docker **`HEALTHCHECK`** and Compose **`healthcheck`** probe **`http://localhost:3921/health`** via **`curl`** (the runtime image installs **`curl`**). This aligns Traefik with an actually reachable backend.
- **`vox doctor --probe`:** Can be used interactively for diagnostics; it is **not** the canonical signal for Docker/Coolify scheduling because it exercises a broader CLI surface than the gateway-only **`mcp-server`** image. Prefer **`GET /health`** for orchestration probes.

## DNS (outside Coolify)

Point **`eval.voxlang.org`** to the same edge IP (or CNAME) where Coolify’s Traefik terminates TLS for your VPS. Coolify cannot create this record; configure it at your DNS provider (for example Cloudflare or your registrar). Confirm with **`dig`** / **`nslookup`** before debugging TLS.

## Coolify configuration

1. Create or select a **Docker Compose** application in Coolify.
2. Set **FQDN / domains** to **`https://eval.voxlang.org`** (or equivalent field for your Coolify version).
3. Inject **`VOX_MCP_HTTP_BEARER_TOKEN`** (and any other env vars from compose) via Coolify **project/environment secrets**.
4. If the GHCR package is private, configure registry credentials in Coolify so **`pull_policy: always`** can fetch **`ghcr.io/vox-foundation/vox-eval`**.
5. Align Traefik / TLS with Coolify’s documented certificate workflow. Raw Compose labels use **`tls.certresolver=letsencrypt`**; your Coolify install may prefer UI-managed certs—avoid duplicate conflicting resolvers.

## API discovery and provisioning (no SSH)

Secrets resolve through **vox-secrets** from canonical env vars ([`deploy-contract.md`](../ci/deploy-contract.md)):

| Purpose | Env vars |
|--------|-----------|
| Coolify base URL | **`COOLIFY_BASE_URL`** |
| Write / deploy | **`COOLIFY_TOKEN`** |
| Read-only **`GET`** (optional) | **`COOLIFY_READ_TOKEN`** (falls back to **`COOLIFY_TOKEN`**) |
| Target app | **`COOLIFY_APP_UUID`** |

**Discover** (list apps and optional version probe):

```bash
vox ci coolify-eval discover
```

**Sync compose from the repo** (updates **`docker_compose_raw`**, optional **`domains`**, then **`GET /api/v1/deploy?uuid=`** unless **`--no-deploy`**):

```bash
vox ci coolify-eval sync-compose \
  --compose vox-eval.compose.yml \
  --domains https://eval.voxlang.org
```

Verify **`COOLIFY_APP_UUID`** matches the eval compose application (**`vox ci coolify-eval discover`** prints **`uuid`** and **`fqdn`**). **`deploy-hetzner.yml`** Gate 3 assumes this UUID backs **`https://eval.voxlang.org/health`** unless **`COOLIFY_PUBLIC_EVAL_HEALTH_URL`** overrides it. Gate 1 is only **`cargo build -p vox-cli --locked`** on **`ubuntu-latest`**; merge-quality gates live in **[`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml)**.

### GitHub Actions

Manual **[`.github/workflows/coolify-eval-sync.yml`](../../../.github/workflows/coolify-eval-sync.yml)** (`workflow_dispatch`) runs **`discover`** and optionally **`sync-compose`** using repository secrets. Exact Coolify **`PATCH`** shapes may differ by major version; treat failures as a signal to confirm API docs for your install.

**`deploy-hetzner.yml` Gate 3:** The public **`curl`** probe must still run when Gate 1 is skipped (**`workflow_dispatch`** + **`skip_tests: true`**). The health-check job should **`needs: [smoke-ci, deploy-coolify]`** with **`if: ${{ always() && needs.deploy-coolify.result == 'success' }}`** so GitHub Actions does not skip the HTTPS verifier just because smoke CI did not run.

## Recovery loop (everything except Cloudflare)

Use this **repeat-until-green** procedure when **`curl -fsS https://eval.voxlang.org/health`** fails (TLS errors, **503** **`no available server`**, etc.). Symptom → cause mapping: **[deploy-contract Gate 3 cheatsheet](../ci/deploy-contract.md)**.

### Step 1 — Identity (`COOLIFY_APP_UUID` vs eval app)

**Prerequisites:** **`COOLIFY_BASE_URL`** and a read-capable bearer (**`COOLIFY_READ_TOKEN`** or **`COOLIFY_TOKEN`**) in the environment, or resolve the same values via vox-secrets locally.

```bash
vox ci coolify-eval discover
```

Confirm exactly one application is the eval stack: **`docker_compose_raw`** is present (compose app), **`fqdn`** / name matches **`eval.voxlang.org`**. Align GitHub repository secret **`COOLIFY_APP_UUID`** with that row’s **`uuid`** so **`deploy-hetzner.yml`** Gate 3 targets the correct resource.

### Step 2 — Compose sync and secrets (Coolify + GHCR)

Push SSOT compose from this repo (writes Coolify application state; requires write/deploy token):

```bash
vox ci coolify-eval sync-compose \
  --compose vox-eval.compose.yml \
  --domains https://eval.voxlang.org
```

Or run **[`.github/workflows/coolify-eval-sync.yml`](../../../.github/workflows/coolify-eval-sync.yml)** with **`sync_compose: true`**.

In Coolify, set **`VOX_MCP_HTTP_BEARER_TOKEN`** and any other env vars referenced by [`vox-eval.compose.yml`](../../../vox-eval.compose.yml). If **`ghcr.io/vox-foundation/vox-eval`** is private, add registry credentials in Coolify. Wait until the deployment finishes and the service is **running** and **healthy** (Compose healthcheck uses **`curl`** → **`http://localhost:3921/health`**).

### Step 3 — TLS and Traefik router

Choose **one** TLS strategy: Coolify UI **FQDN / Generate TLS certificates** for **`eval.voxlang.org`**, **or** Traefik **`tls.certresolver=letsencrypt`** in labels — avoid duplicate conflicting resolvers (see §Coolify configuration above). Confirm the Traefik **`Host`** rule matches **`eval.voxlang.org`** (same as [`vox-eval.compose.yml`](../../../vox-eval.compose.yml)). For HTTP-01 ACME, port **80** must reach Traefik on the origin.

### Step 4 — Backend (**503**)

If TLS verifies but you still see **503** / **`no available server`**, inspect Coolify/Docker logs for the eval container. Inside the container, **`GET /health`** on port **3921** must return **200**. Typical fixes: wrong **`COOLIFY_APP_UUID`**, missing **`VOX_MCP_HTTP_*`**, failed image pull, or **`Host`** mismatch.

### Step 5 — CI alignment (optional)

After the public URL is green, push **`main`** or run **Deploy Hetzner (Coolify)** so **[`.github/workflows/deploy-hetzner.yml`](../../../.github/workflows/deploy-hetzner.yml)** Gate 3 passes.

### Cloudflare (operator-only; not automated here)

Do this at your DNS provider when **`eval.voxlang.org`** uses Cloudflare:

1. **DNS:** **`eval`** → **`A`** / **`AAAA`** (or **`CNAME`**) to the **Coolify/VPS** public target Cloudflare documents for your setup. Confirm with **`dig eval.voxlang.org`**.
2. **Proxy:** **DNS only (grey)** simplifies origin TLS/Let’s Encrypt while debugging. **Proxied (orange)** visitors see Cloudflare’s certificate; set **SSL/TLS** to **Full** or **Full (strict)** when the origin presents a valid cert. Avoid **Flexible** if you rely on **`VOX_MCP_HTTP_REQUIRE_FORWARDED_HTTPS=1`** (see compose).
3. **ACME:** For HTTP-01, ensure **`http://eval.voxlang.org/.well-known/acme-challenge/`** is not blocked by rules. Prefer **DNS-01** in Coolify if HTTP-01 is impractical; that may require a Cloudflare **API token** (**DNS:Edit**) configured **in Coolify**, not in this repository.
4. **Wrong origin:** Ensure **`eval`** is not a **CNAME** to GitHub Pages or another unrelated host.

## Verification

**Loop exit:** from a machine with a normal CA store (no **`-k`**):

```bash
curl -fsS https://eval.voxlang.org/health
```

Expect **HTTP 200**. Repeat until success after each Coolify/DNS change.

**Periodic check (every ~3s, PowerShell):**

```powershell
while ($true) {
  curl.exe -fsS "https://eval.voxlang.org/health" 2>$null
  if ($LASTEXITCODE -eq 0) { break }
  Start-Sleep -Seconds 3
}
```

**Periodic check (bash):**

```bash
until curl -fsS https://eval.voxlang.org/health; do sleep 3; done
```
