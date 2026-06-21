---
title: "Decentralized Skill + Code Marketplace — Research & Codebase Audit"
description: "Grounding research for a local-first, auto-reviewed, decentralized marketplace that mines repeated Vox code blocks and prompts/agent flows, dedups against existing MCP tools/skills, agentically reviews and tags submissions, and shares opt-in. Audits the existing Vox skill/MCP SSOT spine and synthesizes prior art on agentic code review, skill/prompt registries, decentralized distribution, and the Rust crate stack."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Decentralized Skill + Code Marketplace — Research & Codebase Audit

**Status:** Research + audit (pre-design). Feeds a forthcoming design doc and implementation plan.
**Date:** 2026-06-18.
**Scope decision (user):** *Audit + research first, then design.* Decentralization model = **local-first + optional publish**. Discovery targets = **all four**: repeated `.vox`/code blocks, repeated prompts/agent flows, overlap with existing MCP tools/skills (the SSOT problem), and external skills to import.

---

## 0. Problem statement

We want a system that:

1. **Discovers locally** — mines the codebase and session history for *recurring* code blocks and *recurring* prompts/agent flows, and proposes crystallizing them into reusable Vox snippets or `SKILL.md` skills.
2. **Dedups against what already exists** — when a new skill/tool overlaps an installed one (the **MCP↔skill SSOT** gap), suggest reuse instead of re-authoring.
3. **Agentically reviews + tags** — reviews submissions in the absence of human review, attaching proper frontmatter/tags, as a *quality floor* (not the sole gate).
4. **Shares decentralized, opt-in** — primary value is local; publishing is a deliberate push to a registry, with GUI support for browse/search/publish.

Skills are, in the user's framing, "just good prompts" + optional code; code blocks are reusable Vox (but not exclusively Vox).

---

## 1. Codebase audit — what already exists (the SSOT spine is real)

The marketplace **skeleton already exists**; most of this initiative is *new layers on top* plus closing the MCP↔skill gap. Do not rebuild the spine.

### 1.1 Skill registry (authoritative SSOT)
- `vox-skills` is a **re-export shim** over `vox_plugin_host::skill_registry` (`crates/vox-skills/src/registry.rs:7`).
- `SkillRegistry` (`crates/vox-plugin-host/src/skill_registry.rs:80-347`): in-memory + DB-persisted; `install` / `install_bundle` / `uninstall` / `lookup` / `get` / `search` (keyword over id/name/description/tags) / `list` / `hydrate_from_db` (Codex `skill_manifests` table).
- `RegisteredSkill` (`skill_registry.rs:54-73`): `manifest`, optional raw `body`, `source` = `Plugin { plugin_id } | Bundle | OpenClaw { node_id }`.

### 1.2 Manifest schema
- `SkillManifest` (`crates/vox-plugin-types/src/skill_manifest.rs:14-48`): `id, name, version, author, description, category, permissions, tools, dependencies, homepage, registry, hash (SHA-256), tags`.
- `SkillCategory` (`:78-97`) — 13 named + `Custom(String)`. `SkillPermission` (`:109-126`) — `ReadFiles, WriteFiles, ShellExec, Network, DbRead, DbWrite, Secrets`.
- **Note:** `tools: Vec<String>` already declares which MCP tools a skill exposes — the hook for SSOT linkage exists but is unvalidated.

### 1.3 SKILL.md parser
- `crates/vox-plugin-host/src/skill_parser.rs:53-159`: AgentSkills-compliant YAML (`name`, `description` + `[metadata]` `vox-*`) **and** legacy Vox TOML; TOML-first then YAML fallback. Tolerates `license`, `compatibility`, `allowed-tools` (parsed, unused).
- Native plugin skills (8): compiler, git, memory, orchestrator, rag, testing, testing-validate, v0 (`crates/vox-plugin-skill-*`). External agentskills.io layout discovery: `external_skills.rs:26-69` (first-match-wins shadowing).
- Native prompt-style skills also live in `crates/vox-skills/skills/superpowers/`.

### 1.4 Marketplace client (no server yet)
- `SkillsRegistryClient` (`crates/vox-skills/src/registry_api.rs:23-141`): `search` / `download` / `publish` (API-key) / `get_manifest` against `SKILLS_REGISTRY_BASE = https://raw.githubusercontent.com/vox-foundation/vox/main/skills` (`lib.rs:44`).
- **No server-side**: no publish-validation pipeline, storage backend, indexing, or dedup. `downloads`/`stars` are placeholder fields. **No GUI surface.**

### 1.5 Sandbox & trust model
- `crates/vox-skills/src/sandbox/policy.rs:1-137`: `TrustLevel` = `Trusted | Community | Untrusted`; `ApprovalGuard::check` gates Community on `approved`. `resolve_policy`: `Trusted && !Shell → Permissive (host)`, else `Container` (OCI via `vox-container`, Docker/Podman; `OpenClawSidecarSandbox` fallback).
- This local sandbox is the **key differentiator** (see §3.D-4): we can run review + capability analysis + dedup *before execution, on the user's machine*.

### 1.6 Existing agentic review to reuse
- **Visual AI review** (`vox-orchestrator-mcp/src/visus_review/mod.rs:1-86`; `vox ci gui-visual-review`): SHA-256 screenshot cache → review only New/Changed; advisory, never gates. The **cache-on-content-hash → review-only-changed** pattern is directly reusable for skills.
- **Code review engine** (`vox-code-audit/src/review/mod.rs:48-72`): `ReviewClient` over OpenAI/OpenRouter/Ollama; Markdown/SARIF/Terminal output; 60+ static "TOESTUB" detectors. Default model Sonnet 4.6. **Not currently a skill.**

### 1.7 MCP tool registry
- `vox-mcp-registry` built from `contracts/mcp/tool-registry.canonical.yaml`; `SKILL_TOOLS` includes `vox_skill_install/uninstall/list/search/info/parse/use/discover` (`crates/vox-mcp-registry/src/lib.rs:43-52`).
- **Bridge** `vox-orchestrator-mcp/src/plugin_skills_bridge.rs:12-53` installs discovered plugin skills but **does not validate** that a skill's declared `tools` exist in the MCP registry, and offers **no reverse index** (which skills expose tool X).

### 1.8 Crate layering (where new code goes)
- Layers L0–L5 (`docs/src/architecture/layers.toml:71-189`). Skill crates: `vox-plugin-types` (L1), `vox-mcp-registry` (L2), `vox-plugin-host`/`vox-skills`/`vox-container`/`vox-code-audit`/`vox-orchestrator-mcp` (L3), `vox-plugin-skill-*` (L4).
- A new **`vox-skill-marketplace` (L3)** fits the `vox-code-audit → vox-cli-ci` pattern: depends on `vox-plugin-host` + `vox-code-audit` + `vox-db`; consumed by MCP/CLI (L5) + GUI.

### 1.9 Gap ledger (what is genuinely missing)
1. **MCP↔skill SSOT validation + reverse index** (declared `tools` unverified).
2. **Discovery/dedup engine** — no clone/near-duplicate detection (no MinHash/SimHash/AST hashing anywhere).
3. **Submission auto-review for skills** (review exists for code/GUI, not wired as a skill gate).
4. **Marketplace backend** — publish-validation, storage, index, federation.
5. **GUI marketplace surface.**
6. **Signing/provenance** for skills (OpenClaw mesh has envelopes; skills do not).
7. **Real versioning** — semver field exists, no commit-hash/env-tag model, no dependency resolution/cycle detection.
8. **Metrics/anti-abuse** — placeholder downloads/stars; no anti-typosquat, no staleness demotion.

---

## 2. Prior art — agentic review, registries, distribution

### 2.A Agentic / automated code review as a gate
- Commercial reviewers (**CodeRabbit**, **Qodo Merge/PR-Agent** OSS, Greptile, Bito) converge on **severity × category** stratification. CodeRabbit's own framing assumes a human gate and degrades at scale.
- **Most reusable:** Anthropic's `claude-code-action` + `claude-code-security-review` — emit *severity + CWE + vulnerable snippet + ready fix with false-positive filtering*. Same logic as `/security-review`.
- **Deterministic floor first:** Danger JS (hard-fail rules), reviewdog (comment-on-changed-lines), linters/SAST — cheap, reproducible, hard to game.
- **LLM gating patterns:** multi-agent/adversarial ensembles beat single agents when detectors are conditionally independent (CodeX-Verify, arXiv 2511.16708); confidence thresholds (~0.6 intervene; ~0.95 stop debating).
- **Trust recipe:** deterministic floor → ensemble LLM with *calibrated system-level confidence* → gate only on high-confidence + high-severity → escalate the uncertain middle to a human → measure **acceptance rate** (not comment volume) as the KPI. Don't stack overlapping reviewers (causes bulk-dismiss).

### 2.B Skill / prompt registries (what to standardize)
- **Claude Agent Skills (SKILL.md):** `name`/`description` required; `allowed-tools`/`disable-model-invocation` optional; **no version/license field**; git-only versioning; workspace trust dialog, no signing.
- **Official MCP Registry:** JSON-Schema'd `server.json`; REST/OpenAPI for downstream aggregators; **federated subregistries**; **namespace auth** (GitHub OAuth / DNS-TXT) — but explicitly **separates metadata hosting from trust** and delegates scanning downstream.
- **Smithery/Glama/mcp.so:** browse only, no vetting (>1/3 of 8k servers had SSRF). **Agensi:** adds an 8-point automated security scan tier.
- **LangSmith Prompt Hub / PromptHub:** the only registries with **real versioning** — immutable content-hash commit + mutable env tag + diff (LangSmith); git-style branch/commit/merge/rollback (PromptHub). Borrow this.
- **Lessons:** separate metadata from trust; namespace auth solves *authorship not safety*; **signing is absent everywhere** (a differentiation gap to own).

### 2.C Decentralized / local-first distribution (composite of proven primitives)
1. **Content-address every artifact** (Cargo cksum, Nix narHash, IPFS CID) — integrity + dedup, host-independent.
2. **Serve via dumb static/CDN index or git-federated taps** (Cargo **sparse index**, **Homebrew taps**) — anyone can mirror/host; publishing optional.
3. **Anchor canonical truth in an append-only transparency log** (Go **sumdb** inclusion/consistency proofs, Sigstore **Rekor**) — untrusted hosts serve bytes; integrity/non-equivocation stay global.
4. **Sign for provenance:** minisign/ed25519 (offline, local-first) or cosign+Sigstore (CI identity — JSR's choice); SLSA for build provenance.
5. **Govern keys with TUF** (role-based threshold signing; survives compromise/rollback).
- **Anti-pattern:** Deno raw-URL imports (host decentralization without content-addressing or a verifiable index) — Deno abandoned it for JSR.

### 2.D Rust crate stack (maturity-rated)
- **Clone detection:** `tree-sitter` (AST normalize; mature) → `simhash`/`gaoya` (LSH candidate gen; gaoya niche) → `strsim` (ubiquitous) / `tlsh2` (fuzzy confirm; niche). `probminhash` for weighted Jaccard. Avoid `minhash-rs`.
- **Content addressing:** **`blake3`** (CIDs; very mature); `sha2` only for OCI/Sigstore interop (note: manifest `hash` is currently SHA-256).
- **Signing:** **`minisign`/`rsign2`** (local-first, low risk); `sigstore` optional tier (pre-1.0); `ed25519-dalek` 2.x (not 3.0 RC).
- **Storage/search:** **`tantivy`** (FTS) + **`redb`** (ACID embedded KV — prefer over stalled `sled`).
- **P2P (future):** **`iroh`+`iroh-blobs`** (1.0, 2026-06-15) for content-addressed sync; `libp2p` only if a DHT is needed (pre-1.0).
- **Frontmatter:** `gray_matter` + `serde_yaml_ng` (**never plain `serde_yaml`** — unmaintained). Current parser uses `serde_yaml`/`toml`; migration candidate.

### 2.E Similar full attempts — failure modes to design against
- **No marketplace combines auto-review + dedup at scale without spam/quality/security leakage.** All mature ones converged on **automated pre-screen + verification tiers + human review + user reporting.**
- **FlowGPT** (pure community rating → quality collapse), **GPT Store** (list-first-moderate-later → spam + near-dup copycats), **Hugging Face** (real malware/pickle scanning still bypassed; flags-don't-block; answer = safetensors "safe-by-construction"), **npm/VS Code Marketplace** (typosquat + auto-executing install hooks).
- **Anthropic `claude-plugins-community`** — the direct analog — uses **three tiers** (community automated screen → Anthropic Verified human → official curated). They explicitly **did not trust automation alone.**

---

## 3. Synthesis → design constraints (carry into the design doc)

1. **Tiered trust, not auto-review-as-truth.** Auto-review is a *floor*; layer a human-verified badge + curated tier (mirror Anthropic's three tiers). Map onto existing `TrustLevel` (`Untrusted/Community/Trusted`).
2. **Gate before listing** (hold-then-publish), never publish-then-takedown.
3. **Review pipeline:** deterministic floor (compile/lint/`vox-code-audit` detectors) → ensemble LLM review (reuse `vox-code-audit::review` + `claude-code-security-review` shape: severity+CWE+fix+FP-filter) → gate on high-confidence high-severity → escalate middle → KPI = acceptance rate. Reuse the **content-hash cache → review-only-changed** pattern from `visus_review`.
4. **Exploit the local-first edge:** run review + capability analysis + **dedup-against-installed in the existing `vox-container` sandbox, pre-execution, on the user's machine.** Turns HF's advisory-server weakness into a hard pre-execution gate — our differentiator.
5. **Safe-by-construction:** declarative manifest + explicit capability/permission declarations + **no arbitrary code execution on install** (kills the npm attack class). Scanner is bypassable → always pair with runtime sandboxing.
6. **Dedup at submission *and* discovery time:** semantic (tree-sitter → simhash/gaoya → tlsh2/strsim) + lexical/name-collision (typosquat) detection; enforce unique, non-confusable names. Same engine powers local "repeated block/prompt" mining.
7. **Distribution:** content-address (`blake3`) → static sparse index + optional git-tap federation → append-only signed log (Go-sumdb/Rekor pattern) → sign with `minisign` (local) / Sigstore (CI) → TUF key governance. Keep P2P (`iroh`) as an *optional transport behind a signed index*, not the source of truth. Avoid raw-URL imports.
8. **Versioning:** borrow LangSmith's immutable content-hash commit + mutable env tag + diff (no skill registry has real versioning — own it).
9. **Surfacing:** do **not** rank by popularity (attack surface); weight by verification + provenance + freshness (last-updated, compiles-against-current); auto-demote stale entries.
10. **Close the MCP↔skill SSOT:** validate skill `tools` against `vox-mcp-registry` at submission; build a reverse index (tool → skills); treat MCP tools and skills as two projections of one catalog.

---

## 4. Open questions for the design session

- **Discovery trigger/UX:** background daemon vs on-demand `vox` command vs IDE/GUI nudge? How aggressive (advise-not-gate, per our auto-derivation hygiene)?
- **Mining source of truth:** codebase only, or also session transcripts (prompts/agent flows)? Transcripts raise privacy/provenance questions before any publish.
- **First sub-project:** the four subsystems (local discovery+dedup engine / agentic submission review / decentralized distribution+signing / GUI) likely need their own spec→plan cycles. Which is the wedge? (Recommendation: the **local discovery+dedup engine**, since it is the local-first core, has no server dependency, and its similarity engine is reused by submission-time dedup.)
- **"Not limited to Vox":** how language-agnostic must clone detection be at v1? (tree-sitter supports many grammars; scope to Vox first?)
- **Registry identity:** reverse-DNS + GitHub OAuth (MCP-registry model) acceptable, or tie to existing OpenClaw mesh identity/envelopes?

---

## 5. Source pointers

Codebase: see file:line citations in §1. External prior art: Anthropic `claude-code-action` / `claude-code-security-review` / `claude-plugins-community` (three-tier); Official MCP Registry (`modelcontextprotocol.io/registry`); LangSmith Prompt Hub versioning; Cargo sparse-index RFC 2789; Go sumdb; Sigstore/Rekor + TUF + SLSA; Homebrew taps; JSR (Deno) post-mortem on raw-URL imports; Hugging Face pickle-scan bypass (ReversingLabs/JFrog); GPT Store spam (TechCrunch). Crates: blake3, minisign/rsign2, ed25519-dalek, tantivy, redb, tree-sitter, simhash/gaoya, strsim, tlsh2, probminhash, iroh/iroh-blobs, gray_matter/serde_yaml_ng.
</content>
</invoke>
