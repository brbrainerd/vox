---
title: "Environment Variables vs. Clavis: Strategy and Cross-System Settings 2026"
description: "Research synthesis on when environment variables are necessary, when they are harmful, what alternatives exist, and how Vox Clavis can evolve to support cross-system user settings sync for logged-in users."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-16"
training_eligible: false
training_rationale: "Core platform strategy for configuration, secrets, and user settings across systems, orchestrators, and nodes."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Environment Variables vs. Clavis: Strategy and Cross-System Settings 2026

> **See also:**
> - [Clavis secrets, env vars, and API key strategy research 2026](clavis-secrets-env-research-2026.md)
> - [Clavis as a one-stop secrets manager: research findings 2026](clavis-one-stop-secrets-research-2026.md)
> - [Clavis V2: Full Implementation Plan 2026](clavis-implementation-plan-2026.md)
> - [Clavis SSOT reference](../reference/clavis-ssot.md)

---

## 1. The core frustration: why environment variables break things

Environment variables are the de-facto standard for passing configuration into processes. You can't avoid them entirely. But they have a well-understood set of failure modes that compound badly across a distributed system like Vox:

### 1.1 The specific problems with env vars

**Operational brittleness across machines**
- Env vars set in one shell session or `.env` file on one machine are invisible on others.
- Deploying to a new system requires manually reconstructing the entire variable set — error-prone and undocumented.
- A program that works locally silently fails on a fresh install or CI runner because a required var wasn't documented.

**No type safety or schema**
- Env vars are always strings. A mistyped `VOX_ORCHESTRATOR_MAX_AGENTS=ten` produces crashes or silent defaults at runtime, not a startup error.
- `vox_config::env_parse` helps, but every call site must still handle parsing errors independently.

**No lifecycle controls**
- An env var set in 2023 may still be in use today with no audit trail, no expiry, and no way to know which services depend on it.
- Rotation means updating the variable in every environment it was set — with no orchestration.

**Leakage surface**
- Any process can read its parent's env vars.
- Docker `ENV` instructions bake secrets into image layers (read from `/proc/<pid>/environ`).
- Crash dumps and debug logs routinely capture full environment snapshots.
- GitGuardian's 2025 data: **65% of secrets found in 15M public Docker images came from ENV instructions**.

**Cross-system inconsistency**
- A user with 3 machines, a CI runner, and 2 mesh nodes needs to set the same 40+ vars in 6 places.
- They go out of sync. Debugging this is always painful and often silent.

**Config-vs-secrets confusion**
- `VOX_ORCHESTRATOR_MAX_AGENTS=4` is config. `OPENROUTER_API_KEY=sk-...` is a secret. Both arrive through the same channel with identical semantics — zero governance distinction.

archived_date: 2026-04-18
---

## 2. When environment variables ARE necessary and irreplaceable

Despite their problems, env vars cannot be eliminated. Understanding *exactly* where they are mandatory prevents over-engineering alternatives.

### 2.1 Bootstrap and process isolation (irreducible)

Env vars are the **only standardized mechanism for process-boundary configuration injection** on POSIX and Windows. This makes them irreducible for:

**1. Selecting which backend to use**
`VOX_CLAVIS_BACKEND=infisical` — the process has to know *where to get its config* before it can get any config. This is a chicken-and-egg constraint. You cannot resolve this with the vault you haven't connected to yet.

**2. Docker and container orchestration**
- `docker run -e VAR=val` is the canonical way to inject runtime configuration into an immutable image.
- Kubernetes `ConfigMaps` and `Secrets` are ultimately injected as env vars or mounted files — env vars remain the most portable primitive.
- This is **not going away**. The Docker/OCI ecosystem has standardized on it.

**3. CI runners**
- GitHub Actions, GitLab CI, and our own self-hosted runners inject secrets as env vars. There is no other way to transfer credentials into an ephemeral container that doesn't have a persistent keyring or network-accessible vault.
- `VOX_CLAVIS_PROFILE=ci`, `VOX_CLAVIS_BACKEND=env_only`, and the CI secrets themselves must arrive this way.

**4. Third-party tool compatibility**
- Tools like `OPENAI_API_KEY`, `GITHUB_TOKEN`, `HUGGING_FACE_HUB_TOKEN` are read as env vars by the upstream SDKs directly. Clavis already handles these via alias tables.
- We cannot change how HuggingFace or OpenAI's CLI tools resolve their credentials.

**5. Shell-level tooling and scripts**
- `export VOX_DATA_DIR=/mnt/vox-data` in a shell session remains the simplest, most portable way to configure a CLI tool without a config file.

### 2.2 The right minimal set

Env vars should be the **bottom of the resolution stack** — the escape hatch, the CI primitive, and the bootstrap signal only. They should not be the primary UX for human operators.

**Keep for env vars:**
- Bootstrap selection: which profile, which backend, which account
- CI/ephemeral environments: inject-once-per-run credentials
- Third-party compatibility aliases: `OPENAI_API_KEY`, `HF_TOKEN`, etc.
- Shell-level developer convenience for non-secrets (paths, modes, flags)

**Move everything else to Clavis:**
- Actual secret values (keys, tokens, passwords)
- Operator tuning parameters that persist across sessions
- User preferences and non-secret configuration that should follow the user

---

## 3. The taxonomy: what we actually have

Vox has three semantically different populations of environment variables all treated identically today:

### Category A: True secrets (go through `resolve_secret`)
API keys, tokens, database credentials — anything that would cause harm if leaked. These already have `SecretId` entries and should increasingly move to the keyring/vault rather than env vars.

### Category B: Operator tuning parameters (non-secret config)
`VOX_ORCHESTRATOR_MAX_AGENTS`, `VOX_MESH_A2A_MAX_MESSAGES`, `VOX_BUILD_TIMINGS_BUDGET_WARN` — these are integers, booleans, and strings that control behavior without being security-sensitive. They are currently in `OPERATOR_TUNING_ENVS` but treated like config, not secrets.

**Problem:** They have no place to live in Clavis's current semantic model. They aren't secrets; they aren't bootstrap signals. They're just... configuration. Right now they live in `.env` files or shell sessions and fail to sync across machines.

### Category C: Bootstrap/profile selectors (env-only by design)
`VOX_CLAVIS_PROFILE`, `VOX_CLAVIS_BACKEND`, `VOX_CLAVIS_CUTOVER_PHASE` — these must come from env vars because they configure the resolution mechanism itself.

This taxonomy is implicit in the codebase today. Making it explicit is Wave 0 of the Clavis V2 plan.

archived_date: 2026-04-18
---

## 4. Better alternatives: what exists and what fits

### 4.1 For secrets: Clavis is already the right answer

The Clavis architecture is sound. Resolution precedence (`env → backend → keyring → auth_json → populi_env`) is correct. The SecretId registry is comprehensive at 400+ entries.

Gaps identified in `clavis-one-stop-secrets-research-2026.md` remain the priority targets:
- Audit logging (Wave 2)
- Secret versioning / rotation tracking (Waves 4, 8)
- Profile-scoped value overrides (Wave 5)
- A2A credential delegation (Wave 7)

### 4.2 For non-secret config: structured TOML/JSON files via `vox_config`

The `vox_config::env_parse` helper is the right primitive but currently lacks a layered file-based config source. Industry standard (Figment, config-rs) is a layered model:

```
defaults (compiled in)
  ← ~/.vox/config.toml (user-global)
  ← .vox/config.toml (project-local)
  ← $VOX_CLAVIS_PROFILE-specific overrides
  ← environment variables (highest precedence, CI/override)
```

This would let a user set `VOX_ORCHESTRATOR_MAX_AGENTS=8` once in `~/.vox/config.toml` and never think about it again. It would also enable cross-system sync (§5).

**Key insight:** Non-secret operator config doesn't need encryption or keyring storage. It just needs a stable home and a sync mechanism.

### 4.3 For cross-system consistency: two paths

**Path 1 (local + sync):** `~/.vox/config.toml` as the home for non-secret config, with an optional sync to VoxDB/Turso so the file is replicated across machines when the user is logged in.

**Path 2 (remote-first):** VoxDB as the canonical location, resolved on each session start. Cached locally for offline use.

Both paths require user authentication (the "logged in to Vox" precondition).

### 4.4 For Docker and deployment: env vars with a migration path

Keep env vars as the Docker/CI injection mechanism but ensure Clavis treats them as **low-precedence compatibility sources** in strict profiles — not the source of truth.

Operators deploying Vox in Docker can set `VOX_CLAVIS_BACKEND=vault` and `VOX_CLAVIS_VAULT_URL=...` as env vars to bootstrap Clavis, which then takes over for all other resolution.

---

## 5. Cross-system settings: the vision for logged-in Vox users

This is the most strategically interesting dimension of the user's question. The goal: a user logging into Vox on a new machine should have their settings "just work" — without re-entering 40 env vars.

### 5.1 The two distinct problems

**Problem A: Secret sync across machines**
User has `OPENROUTER_API_KEY=sk-...` stored on their dev machine. They want it available on their home server and in the mesh. This is a secret sync problem.

**Problem B: Configuration sync across machines**
User has set `VOX_ORCHESTRATOR_MAX_AGENTS=8`, `VOX_MODEL=claude-sonnet-4-5`, `VOX_DATA_DIR=/mnt/vox` on their dev machine. They want these preferences on their other machines. This is a config sync problem.

These need different solutions because their security requirements differ.

### 5.2 Secret sync: Hybrid (Keyring + VoxDB ciphertext) architecture

The existing research (§5.3 of `clavis-one-stop-secrets-research-2026.md`) describes this precisely:

```
Local keyring holds:          vox-clavis-vault/master KEK
VoxDB/Turso stores:           AES-256-GCM ciphertext (encrypted by DEK wrapped by KEK)
                              Wrapped DEKs (encrypted by KEK, never stored plaintext)

Sync model:
  - User logs in → KEK in local keyring
  - Pull encrypted secret corpus from VoxDB  
  - Decrypt DEKs locally → decrypt secrets into memory
  - Secret values never stored in VoxDB in plaintext
  - Cloud has ciphertext only; user has sovereignty
```

**Security invariants:**
- The cloud (VoxDB/Turso) never sees the KEK. It only ever stores wrapped DEKs and double-encrypted ciphertext.
- A new machine bootstraps by: `vox login` → enters master passphrase or platform-specific auth → derives KEK → decrypts corpus from VoxDB.
- Secrets marked `device_local_only: true` in `SecretMetadata` are excluded from sync.

**For `persistable_account_secret: true` secrets** (as defined in `ids.rs`): these are eligible for cross-device sync. Today only LLM API keys and integration keys have this flag set to true. This is correct — mesh tokens and JWT HMAC secrets should stay device/deployment-local.

### 5.3 Config sync: user profile store in VoxDB

Non-secret configuration is simpler:

```toml
# ~/.vox/config.toml  (local, optional)
[orchestrator]
max_agents = 8
model = "claude-sonnet-4-5"

[populi]
temperature = 0.7
```

With sync enabled, this file is pushed to a VoxDB `user_config` table row keyed by `(account_id, key)`. On a new machine, `vox login` pulls this table and materializes `~/.vox/config.toml` locally.

**Resolution order for config values:**
```
1. env var (highest; CI/override)
2. ~/.vox/config.toml (local user prefs; loaded from VoxDB if synced)
3. .vox/config.toml  (project-level, repo-committed)
4. compiled defaults
```

This matches the Figment/config-rs layered pattern and is a clean extension of what `vox_config::env_parse` already does.

### 5.4 Propagation to orchestrators and mesh nodes

When Vox runs distributed (mesh workers, orchestrator nodes):

**Secrets:**
Each node should be bootstrapped with a scoped role token (`VOX_MESH_WORKER_TOKEN`) that grants it access to the VoxDB vault at its privilege level only. It resolves secrets independently through Clavis — the orchestrator does not inject raw secrets into worker task descriptors. This is the A2A delegation model (Wave 7).

**Non-secret config:**
Mesh workers should receive their config from the orchestrator via structured task descriptors or from a shared VoxDB config table, not from environment variables at each node. Operator tuning vars (`VOX_MESH_A2A_MAX_MESSAGES`, etc.) should be readable from VoxDB config rows, with env var as a local override.

**Performance model:**
- Config is cached in-process after first resolve. No per-secret round-trip during normal operation.
- VoxDB Turso uses embedded libSQL for local read latency under 1ms. Remote sync happens at session start and on `vox config sync`.
- Bloom-filter membership test for the runtime scrubber (§4.5 of one-stop research) ensures secret redaction doesn't become a hot path.

archived_date: 2026-04-18
---

## 6. Recommended architecture: three-tier configuration

Synthesizing the research, Vox should converge on three distinct tiers:

```
┌─────────────────────────────────────────────────────────────┐
│  Tier 1: Bootstrap                                          │
│  Mechanism: Environment variables (always supported)        │
│  Contents:  Profile selectors, backend choices,             │
│             CI-injected credentials, Docker compatibility   │
│  Sync:      None (per-deployment, ephemeral)               │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  Tier 2: Secrets (vox-clavis / vox clavis)                  │
│  Mechanism: Keyring + VoxDB vault (KEK/DEK envelope)        │
│  Contents:  API keys, tokens, OAuth credentials,            │
│             mesh transport tokens, DB credentials           │
│  Sync:      Opt-in account sync (ciphertext only to VoxDB)  │
│  Filter:    persistable_account_secret flag                 │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  Tier 3: Operator config (vox_config / vox config)          │
│  Mechanism: ~/.vox/config.toml ← VoxDB user_config table   │
│  Contents:  Model prefs, tuning params, feature flags,      │
│             non-secret orchestrator settings                │
│  Sync:      Automatic on login (plaintext, no encryption)   │
│  Precedence: env > ~/.vox/config.toml > .vox/config.toml   │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. What should drive implementation decisions

### Already well-designed (maintain as-is)
- `SecretId` registry and `SecretSpec` metadata structure
- `resolve_secret` resolution precedence chain
- `persistable_account_secret` flag on SecretMetadata
- `device_local_only` flag for secrets that must not sync
- `secret-env-guard` and `clavis-parity` CI gates
- `BackendMode::Auto` fallback logic

### Gaps requiring new work

**Short-term (high value, low risk):**

1. **Explicit config vs. secret split** — Audit `OPERATOR_TUNING_ENVS` and label each entry as `ConfigClass::OperatorTuning` vs `SecretClass::*`. Stop the conflation of tuning params with secrets.

2. **`~/.vox/config.toml` as the canonical home for non-secret config** — Let `vox_config::env_parse` fall back to a TOML file before the compiled default. This alone prevents most "broken across machines" frustrations for non-secret settings.

3. **`vox config set/get/list` CLI surface** — Mirrors `vox clavis set/get`. Allows users to manage non-secret preferences without touching env vars.

**Medium-term (higher complexity):**

4. **VoxDB `user_config` table for sync** — When `vox login` succeeds, pull user config from VoxDB and materialize to `~/.vox/config.toml`. Push on `vox config sync`. Zero encryption needed (not secrets).

5. **VoxDB secret ciphertext sync (account opt-in)** — Pull encrypted Clavis vault from VoxDB on login. Requires KEK/DEK implementation described in §5.2 and the cloudless threat model. This is the full cross-device secret sync story.

6. **Audit logging (Wave 2 of Clavis V2)** — Without this, cross-device secret usage is invisible.

**Longer-term:**

7. **A2A credential delegation (Wave 7)** — Critical for mesh and orchestrator security. Prevents raw secrets from propagating between nodes.

8. **Runtime secret scrubber** — `redact_secrets_from_value` for MCP tool outputs.

archived_date: 2026-04-18
---

## 8. Docker and deployment: concrete guidance

For users asking "should I still use environment variables when deploying?"

**Yes, for:**
- `VOX_CLAVIS_BACKEND=infisical` — bootstrap signal
- `VOX_CLAVIS_PROFILE=prod` — environment identification
- `INFISICAL_TOKEN=...` or `VAULT_TOKEN=...` — the one bootstrap credential that unlocks the vault
- `VOX_ACCOUNT_ID=...` — identity for vault auth
- Third-party tool credentials that Clavis reads via compatibility aliases

**No, for:**
- The 40+ actual secret values. Once Clavis is bootstrapped, it resolves them from the vault. Only one env var (the bootstrap credential) needs to be in Docker.
- Operator tuning parameters that should be in `~/.vox/config.toml` or VoxDB config.

**The target Docker flow:**
```dockerfile
# In the image: nothing sensitive
ENV VOX_CLAVIS_BACKEND=vault
ENV VOX_CLAVIS_PROFILE=prod

# At runtime: only the bootstrap credential
docker run -e VAULT_TOKEN=hvs.short-lived-token -e VAULT_ADDR=https://vault.internal ...
```

Everything else Clavis resolves from the vault at startup. The image has no secrets baked in.

---

## 9. Summary: answers to the original questions

**"When is it proper, necessary, and essential to use env vars?"**

Env vars are proper when:
1. Selecting which backend/profile to use (bootstrapping)
2. Injecting credentials into Docker/CI ephemeral environments  
3. Providing quick developer overrides within a shell session
4. Third-party compatibility (tools that read their own specific var names)

Env vars are improper when:
- Used as the permanent home for secrets (use keyring/vault)
- Used for non-ephemeral operator tuning (use `~/.vox/config.toml`)
- Used to pass secrets between processes (use A2A delegation refs)

**"Can/should we use Vox Clavis to manage env vars even further?"**

Yes. The correct direction is making Clavis the **control plane** and env vars the **escape hatch**. The `CutoverPhase::Enforce` mode — already defined in the codebase — is the eventual target for production: Clavis-managed secrets only, env vars rejected at startup in strict profiles.

The Clavis V2 roadmap (`clavis-implementation-plan-2026.md`) lays out the path. The highest-impact next moves are Wave 1 (metadata enrichment), Wave 2 (audit logging), and Wave 5 (profile-scoped overrides).

**"Can logged-in Vox users share settings across systems, orchestrators, and nodes?"**

Yes, with a two-layer model:
- **Non-secret config:** Sync `~/.vox/config.toml` via VoxDB `user_config` table on login. Plaintext. Fast. Already architecturally feasible with the Turso backend.
- **Secrets:** Sync AES-256-GCM ciphertext to VoxDB. KEK held only in local OS keyring (never leaves device). Decrypt locally. This is the Hybrid (Keyring + VoxDB ciphertext) tier. Opt-in. Only `persistable_account_secret: true` secrets eligible.

**"Performing across systems, orchestrators, and nodes?"**

- Config resolution is cache-on-first-read (in-process, <1ms for tuning params).
- Vault decryption is session-start only; secrets stay in memory as `secrecy::SecretString`.
- Remote VoxDB reads are compressed with Turso's embedded libSQL latency characteristics.
- Mesh nodes use scoped role tokens — they don't sync the full vault; they only resolve what their role permits.

archived_date: 2026-04-18
---

## 10. Research bibliography

- [Clavis secrets, env vars, and API key strategy research 2026](clavis-secrets-env-research-2026.md)
- [Clavis as a one-stop secrets manager: research findings 2026](clavis-one-stop-secrets-research-2026.md)
- [Clavis V2 Implementation Plan 2026](clavis-implementation-plan-2026.md)
- [Clavis Cloudless Threat Model V1](clavis-cloudless-threat-model-v1.md)
- [OWASP Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)
- [OWASP Non-Human Identities Top 10 (2025)](https://owasp.org/www-project-non-human-identities-top-10/2025/)
- [The Twelve-Factor App — Config](https://12factor.net/config)
- [Beyond Twelve-Factor: configuration, credentials, and code](https://www.oreilly.com/content/configuration-credentials-and-code-in-cloud-native-apps/)
- [GitGuardian: State of Secrets Sprawl 2025](https://www.gitguardian.com/state-of-secrets-sprawl)
- [Figment crate (layered config for Rust)](https://docs.rs/figment/latest/figment/)
- [config-rs crate](https://docs.rs/config/latest/config/)
- [keyring crate](https://docs.rs/keyring/latest/keyring/)
- [secrecy crate](https://docs.rs/secrecy/latest/secrecy/)
- [RFC 8693: OAuth 2.0 Token Exchange](https://rfc-editor.org/rfc/rfc8693)


