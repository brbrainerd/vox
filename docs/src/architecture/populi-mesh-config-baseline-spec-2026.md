---
title: "Populi Mesh — Config Baseline Spec (S1, 2026-05-01)"
description: "Slice S1 child spec for workstream W7 partial. Designs the Vox.toml [mesh] schema, sensible defaults, an env-var precedence policy, and the populi-quickstart how-to. Sets up the surface that S2/S3 operator-UX work extends."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Defines the configuration surface for the Populi mesh and the env-var deprecation contract."
---

# Populi Mesh — Config Baseline (S1 child spec)

**Parent.** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md), Slice S1, Workstream W7 partial.

**Goal.** Make `vox populi serve` usable without setting any `VOX_MESH_*` env var, by introducing a `[mesh]` section in the workspace `Vox.toml`, sensible defaults, and a quickstart how-to. Establish the precedence rule that env vars override config and document it once.

**Non-goals.**
- `vox populi pair` / `vox populi inventory` — S2 / S3.
- Removing env vars — that's a deprecation cycle, scoped to S3 (`populi-mesh-operator-ux-completion-spec`). S1 only adds equivalents and marks env vars as "supported but use config".
- Per-peer config (peer-list, per-peer pubkey) — needs pairing, S2.

---

## Part 1 — Current state

**36 distinct `VOX_MESH_*` and `VOX_ORCHESTRATOR_MESH_*` env vars** are read across the codebase (counted via grep on the populi/cli/config crates). Notable ones:

| Env var | Purpose |
|---------|---------|
| `VOX_MESH_ENABLED` | Master switch. |
| `VOX_MESH_TOKEN` | Bearer auth token. |
| `VOX_MESH_ADMIN_TOKEN` | Admin scope token. |
| `VOX_MESH_SUBMITTER_TOKEN` | Submitter scope token. |
| `VOX_MESH_CONTROL_ADDR` | Bind/connect address. |
| `VOX_MESH_NODE_ID`, `VOX_MESH_SCOPE_ID`, `VOX_MESH_RANK` | Identity. |
| `VOX_MESH_LABELS` | Free-form node labels for capability hints. |
| `VOX_MESH_ADVERTISE_GPU`, `VOX_MESH_DEVICE_CLASS` | Operator-asserted hardware class (Layer C per ADR-018). |
| `VOX_MESH_HTTP_HEARTBEAT_SECS`, `VOX_MESH_MAX_STALE_MS`, `VOX_MESH_SERVER_STALE_PRUNE_MS` | Timing. |
| `VOX_MESH_HTTP_MAX_BODY_BYTES`, `VOX_MESH_HTTP_RATE_LIMIT*` | Limits. |
| `VOX_MESH_REGISTRY_PATH`, `VOX_MESH_*_STORE_PATH` | Persistence locations. |
| `VOX_MESH_DONATION_POLICY_JSON`, `VOX_MESH_EXEC_POLICY` | Policy. |
| `VOX_MESH_REPLAY_PERSIST`, `VOX_MESH_REPLAY_STATE_PATH` | Replay state. |
| `VOX_MESH_JWT_HMAC_SECRET` | JWT signing. |
| `VOX_MESH_BOOTSTRAP_EXPIRES_UNIX_MS` | Bootstrap window. |
| `VOX_MESH_CODEX_TELEMETRY` | Misc telemetry knob. |

The workspace `Vox.toml` at the repo root already uses `[section]` syntax (e.g., `[review.coderabbit]`). The user config `~/.vox/config.toml` is a flat key-value store keyed by env-var names ([toml_config.rs:13](../../../crates/vox-config/src/toml_config.rs:13)).

**What's wrong.**
1. A new contributor on a fresh box has to read `populi.md` cover-to-cover to know which env vars to set.
2. Some defaults are unreasonable (no fallback bind address, no quickstart store path).
3. Precedence between env vars, user config, and any future workspace config is undocumented; conflicts are silent.
4. There's no `[mesh]` block — every operator setting is either an env var or a hand-edited vox-secrets secret.
5. `vox populi serve` failure modes (port in use, missing token) are bare Rust errors, not actionable hints.

---

## Part 2 — Design

### 2.1 Configuration sources and precedence

```
                           merge order (later overrides earlier)
                           ────────────────────────────────────►
default values  →  workspace Vox.toml [mesh]  →  user ~/.vox/config.toml  →  env vars  →  CLI flags
```

- **Workspace `Vox.toml` `[mesh]`** — project-scoped, shared by everyone working on the repo (gitable).
- **User `~/.vox/config.toml`** — per-user; existing flat KV store. Per-user mesh settings (bearer token, node_id) live here.
- **Env vars** — runtime overrides; remain fully supported.
- **CLI flags** — per-invocation override.

A new `MeshConfig::resolve()` function performs the merge and returns a fully-resolved struct. Every place currently doing direct `std::env::var("VOX_MESH_…")` is rewritten to use this struct.

### 2.2 The `[mesh]` schema

```toml
[mesh]
enabled = true                        # default false on a fresh checkout

# Identity (defaults: derived from hostname + Ed25519 keypair persisted in vox-secrets)
node_id = "auto"                      # "auto" | explicit string
scope_id = "auto"                     # "auto" | explicit string
rank = 0

# Labels — used for capability matching by the orchestrator.
# Defaults are EMPTY; operator-asserted only.
labels = []                           # e.g. ["gpu:nvidia", "tier:dev"]

# Network
[mesh.control]
addr = "127.0.0.1:0"                  # 0 = pick a free port; explicit for cross-host
heartbeat_secs = 15
max_stale_ms = 60000
server_stale_prune_ms = 300000

[mesh.control.http]
max_body_bytes = 1048576              # 1 MiB
rate_limit_per_sec = 100
rate_limit_burst = 200

# Storage (defaults to `~/.vox/mesh/`)
[mesh.store]
backend = "sqlite"                    # only "sqlite" supported in S1; future: "memory" for tests
path = "auto"                         # "auto" or explicit directory

# Probes — see populi-mesh-probe-correctness-spec
[mesh.probe]
order = []                            # empty = platform default; explicit = override
cache_ttl_secs = 300

# Donation / execution policy
[mesh.policy]
exec = "permissive"                   # "permissive" | "strict"
donation_kinds = ["llm", "training"]  # task kinds this node will accept

# Observability
[mesh.observability]
emit_traces = true
trace_sample = "always"               # "always" | "off" — S1 only supports these two

# Bootstrap (HTTP join URL — single string for S1; list comes with S2 multi-peer)
[mesh.bootstrap]
http_join = ""                        # e.g. "https://other-box:9000"
expires_unix_ms = 0                   # 0 = no expiry
```

Schema is versioned implicitly (additive only). Unknown keys produce a warning but do not refuse to start.

### 2.3 The `[mesh]` user-config keys

In `~/.vox/config.toml` (existing flat-KV format, key-prefix convention):

```toml
"mesh.token" = "…"                    # bearer token (was VOX_MESH_TOKEN)
"mesh.admin_token" = "…"              # was VOX_MESH_ADMIN_TOKEN
"mesh.submitter_token" = "…"          # was VOX_MESH_SUBMITTER_TOKEN
"mesh.jwt_hmac_secret" = "…"          # was VOX_MESH_JWT_HMAC_SECRET
```

Why split: identity-and-secrets belong to the user; project topology belongs to the workspace. This matches the existing `~/.vox/config.toml` purpose (per-user env-var-shaped values).

### 2.4 Defaults that change behavior

`vox populi serve` with **no env vars and no `[mesh]` block**:

- Refuses to start with: `"Mesh is disabled. Set [mesh].enabled = true in Vox.toml, or pass --enable, to start."`
  - **Rationale.** The mesh shouldn't auto-start on every contributor's machine just because they cloned the repo. Explicit opt-in.

`vox populi serve --enable` (or `[mesh].enabled = true`):
- Picks a free port if `addr` is `127.0.0.1:0`.
- Generates a bearer token on first run, persists to `~/.vox/config.toml` `"mesh.token"`, and prints it once: `"Generated mesh token: …  (saved to ~/.vox/config.toml)"`. Subsequent runs reuse it.
- Stores everything under `~/.vox/mesh/`.

This is the "10 minutes to first task" path the north-star promises.

### 2.5 New CLI verbs

- `vox populi config show` — prints the resolved `[mesh]` config with the source of each value (`default | workspace | user | env | flag`).
- `vox populi config check` — validates the schema, reports unknown keys, refuses to start if values conflict (e.g., `enabled = false` but `--enable` is passed → the latter wins, but check warns).
- `vox config check` (project-wide existing verb, if it exists; otherwise scope to `vox populi config check`) — runs mesh validation as a sub-check.

### 2.6 Error message rewrites

Every error path that currently produces a bare Rust error gets a wrapper. Examples:

- Port in use: `"Port 9000 is in use. Another vox populi process? Run 'vox populi config show' for the current bind address, or change [mesh.control].addr."`
- Missing token at request time: `"No mesh token set. Run 'vox populi token generate' or set [mesh.token] in ~/.vox/config.toml."`
- Schema mismatch on store: `"Mesh store at <path> is at schema v1; this binary expects v2. Run 'vox populi store migrate' to upgrade."`

### 2.7 The `populi-quickstart.md` how-to

Lives at [`docs/src/how-to/populi-quickstart.md`](../how-to/populi-quickstart.md) (new file).

Structure:
1. What you'll have at the end (3 sentences).
2. Step 1: enable the mesh (`vox populi config init` writes the `[mesh]` block with defaults).
3. Step 2: start the server (`vox populi serve`).
4. Step 3: check status (`vox populi status`).
5. Step 4: submit a task in-process to confirm the loop works.
6. "What's next" — pointers to S2 / S3 features.
7. Troubleshooting: 5 most likely first-run errors and what to do.

Length target: ~150 lines, mostly fenced commands + 1–2 sentences each.

### 2.8 Env-var deprecation policy

Every env var that has a `[mesh]` equivalent is documented as **"supported"** in S1, **"deprecated, prefer [mesh].…"** in S2, and **"removed"** no earlier than S3+1. This is added to `populi.md` as a single table.

Removal is *not* part of this spec.

---

## Part 3 — Test plan

### 3.1 Unit tests

`vox-config/src/mesh.rs::tests`:
- `precedence_default_only` — no env, no config → defaults applied.
- `precedence_workspace_overrides_default` — `[mesh]` value beats default.
- `precedence_user_overrides_workspace` — user config wins for keys it sets.
- `precedence_env_overrides_user` — env var wins.
- `precedence_flag_overrides_env` — CLI flag wins.
- `unknown_key_warns_does_not_fail` — extra `[mesh.foo]` block warns and continues.
- `addr_auto_picks_free_port` — `127.0.0.1:0` resolves to a free port at bind time.
- `bearer_token_generated_and_persisted_on_first_run` — fresh user config; first call generates and saves.

### 3.2 Integration tests

`crates/vox-cli/tests/populi_serve_quickstart.rs` (new):
- `fresh_install_serves_with_only_enable_flag` — clean dir, `vox populi serve --enable`, verify it accepts a request from `vox populi status`.
- `config_show_shows_source_of_each_value` — set one workspace value, one user value, one env var; verify `vox populi config show` reports the right source for each.
- `config_check_rejects_unknown_at_known_section` — invalid value type produces actionable error.

### 3.3 Doc test

The quickstart how-to has every command verified against the actual CLI surface (manual once, automated as a follow-on backlog item).

---

## Part 4 — Acceptance criteria

1. `vox populi serve --enable` works on a fresh box with no `VOX_MESH_*` env vars set.
2. `[mesh]` block in `Vox.toml` covers every existing `VOX_MESH_*` variable with a documented mapping.
3. `vox populi config show` prints the resolved config with sources.
4. Every `VOX_MESH_*` env var still works (S1 deprecates none).
5. `docs/src/how-to/populi-quickstart.md` exists and walks a contributor from clone to first task.
6. Backlog items closed: `MESH-105` (partial — `serve --enable` covers `join`), `MESH-110`–`MESH-113`, `MESH-141`–`MESH-148`.

---

## Part 5 — Out-of-scope items

- **`vox populi pair` / `peers` / `inventory` verbs** — S2 / S3.
- **Per-peer config** — needs pairing, S2.
- **Removing or repurposing env vars** — S3+1 timeframe at earliest.
- **TLS cert paths** — `[mesh.tls]` block is reserved but not implemented in S1; backlog `MESH-149`.
- **Per-task budget config** — different concern.

---

## Part 6 — Rough cost

- `MeshConfig` struct + resolver: ~250 LOC.
- `vox populi config` subcommand: ~150 LOC.
- Token generation + persistence: ~100 LOC.
- Error message rewriting: ~100 LOC scattered across handlers.
- Tests: ~300 LOC.
- Doc: `populi-quickstart.md` (~150 lines), `populi.md` env-var mapping table (~80 lines).

Total: ~1000 LOC + ~230 doc lines. No new dependencies (existing `toml`, `serde`).

---

## Revision history

- **2026-05-01.** Initial S1 child spec.
