# Telemetry Coolify Deployment — Handoff (2026-06-20)

Status of `telemetry.voxlang.org` rollout, what's automated, and the **2 manual
steps** that unblock the rest. Once both are done, I (the agent) can finish the
deployment end-to-end via the Coolify API + `gh`.

## ✅ Done (automated / merged)

- Server folded into `server/telemetry/` (excluded workspace); boot-migration +
  bearer gate; `docker/vox-telemetry.compose.yml`; CI/CD workflows; compute-placement
  policy. **All on `main`** (PR #391, commits `6d5258252f..8034a8b093`).
- **GHCR image built & pushed:** `ghcr.io/vox-foundation/vox-telemetry-server:latest`
  (workflow run 27886784471, success).
- Live-audited the Coolify instance: target is the **existing "Vox Ecosystem"**
  project (`no88080okk0c0gk4ss0cgw0o`, env `production`), already hosting eval —
  **separate** from "FableForge Production". No new project needed.

## ⛔ The blocker

Creating the Coolify app and setting its secrets needs a **Write-scoped Coolify API
token**. The only token available (FableForge `.env.local` `COOLIFY_TOKEN`) is
**Deploy+Read** — `POST /api/v1/applications/public` returns `403 "Missing required
permissions: write"`. Vault (`secret/fableforge/infra`) holds Coolify **login**
creds but no API token. There is no password-grant API to mint one — it must be
created in the UI.

## 🔧 Manual step 1 — provide a Write-scoped Coolify token (pick ONE)

**Option A (preferred — let the agent finish everything):**
1. Coolify UI → `http://178.156.212.19:8000` → **Keys & Tokens → API Tokens**.
2. Create a token with **read + write + deploy** permissions. Name it `vox-telemetry-deploy`.
3. Give it to the agent (or `vault kv put secret/fableforge/infra COOLIFY_API_TOKEN=<token>`).

→ The agent then runs **Manual-2's DNS aside excepted** everything below automatically.

**Option B (do the app yourself in the UI):**
1. Coolify → project **Vox Ecosystem** → **+ New Resource → Docker Compose** (public repo).
2. Repository `https://github.com/vox-foundation/vox`, branch `main`,
   **Compose file path** `/docker/vox-telemetry.compose.yml`, server `localhost`.
3. **Environment variables** (mark both as build+runtime secrets):
   - `CLICKHOUSE_PASSWORD` = a strong random value
   - `VOX_TELEMETRY_INGEST_TOKEN` = a strong random value (also goes to GitHub secrets, below)
4. Deploy once. Copy the **application UUID** from the URL and give it to the agent.

## 🔧 Manual step 2 — Cloudflare DNS (always required; no CF API token available locally)

In Cloudflare DNS for `voxlang.org`:
- Add `A` record: **`telemetry` → `178.156.212.19`**, **Proxy status = DNS only (grey cloud)**.
  (Grey cloud is required so Traefik's Let's Encrypt HTTP-01 challenge reaches the VPS
  and TLS terminates at Traefik — same as the eval host.)

## 🤖 What the agent runs once Manual-1 (token) + Manual-2 (DNS) are done

With a Write token `$TOK` against `BASE=http://178.156.212.19:8000`:

1. **Create the app** in Vox Ecosystem (if Option A):
   ```
   POST $BASE/api/v1/applications/public
   { project_uuid: no88080okk0c0gk4ss0cgw0o, server_uuid: so0ssokc0wcoskgw8wo4808w,
     environment_name: production, git_repository: https://github.com/vox-foundation/vox,
     git_branch: main, build_pack: dockercompose,
     docker_compose_location: /docker/vox-telemetry.compose.yml,
     ports_exposes: "4318", name: vox-telemetry, instant_deploy: false }
   ```
2. **Set app secrets** (`CLICKHOUSE_PASSWORD`, `VOX_TELEMETRY_INGEST_TOKEN`) via
   `POST $BASE/api/v1/applications/{uuid}/envs`.
3. **GitHub repo secrets** on `vox-foundation/vox`:
   ```
   gh secret set COOLIFY_TELEMETRY_APP_UUID --body <uuid>
   gh secret set VOX_TELEMETRY_INGEST_TOKEN --body <same value as step 2>
   ```
4. **Deploy** (Deploy scope suffices): `GET $BASE/api/v1/deploy?uuid=<uuid>`, or run the
   `Deploy Telemetry (Coolify)` workflow via `gh workflow run`.
5. **Verify Gate-3**: `curl -sS https://telemetry.voxlang.org/healthz` → `ok` (verified TLS),
   then POST one bearer-authed `vox.command` event and confirm a row in `events_raw`.

## Notes

- **Decoupling**: the telemetry app lives in **Vox Ecosystem** (not FableForge's project);
  ClickHouse is internal-only (no ports, no Traefik) with a Vox-owned named volume.
- **Client side** (`vox-telemetry-otlp` endpoint default + bearer, Phase 5) is on branch
  `claude/telemetry-track-f` — merge that to have vox instances actually upload.
- The `VOX_TELEMETRY_INGEST_TOKEN` is a **write-only anti-abuse key**, not a privacy
  boundary; privacy is client redaction + server allowlist.
