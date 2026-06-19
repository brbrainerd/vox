---
title: "Local Skill/Code Discovery + Dedup Engine — Design Spec (Subsystem A)"
description: "Design for the local-first discovery+dedup engine: a pure L2 similarity core (vox-similarity) plus an L3 orchestrator (vox-skill-discovery) that mines repeated .vox code blocks and prompt/agent flows, dedups candidates against installed skills/MCP tools and the external registry, and reports advisory Candidates with draft frontmatter. Wedge sub-project of the decentralized skill+code marketplace."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Local Skill/Code Discovery + Dedup Engine — Design Spec (Subsystem A)

**Status:** Approved design (brainstorming output). Feeds the Antigravity/Gemini-targeted implementation plan.
**Research basis:** [`../../src/architecture/skill-code-marketplace-research-and-audit-2026-06-18.md`](../../src/architecture/skill-code-marketplace-research-and-audit-2026-06-18.md).
**Execution target:** Gemini 3.5 Flash inside Google Antigravity — see [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) and [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## 1. Scope

The wedge sub-project (A) of a four-part marketplace (A discovery+dedup · B agentic submission review · C decentralized distribution · D GUI). This spec covers **A only**. Decisions locked in brainstorming:

- **Trigger:** on-demand command (`vox skill discover`); engine is a reusable library callable later by GUI/CI.
- **Decentralization posture:** local-first; this subsystem does not publish.
- **v1 sources (all four):** repeated `.vox`/code blocks · repeated prompts/agent flows (session transcripts) · dedup vs installed skills/MCP tools · surface external skills to import. Plan phases them (code+installed → prompts → registry).
- **Structure:** layered — pure L2 `vox-similarity` + L3 `vox-skill-discovery`.
- **Posture:** advise-not-gate, opt-in; never installs/executes/publishes; transcript mining opt-in and local-only.

## 2. Crates

### 2.1 `vox-similarity` (L2, pure — no IO)
- `Fragment { id, kind: FragmentKind, content_hash: Blake3Hash, signature: Signature, text: String }`.
- `Signature` — simhash (u64) + minhash bands; optional tlsh2 digest. Deterministic from `text`.
- `LshIndex` — `insert(Fragment)`, `query_near(&Fragment) -> Vec<&Fragment>` by band collision.
- `cluster(&[Fragment], thresholds) -> Vec<Cluster>` — groups recurring fragments (members ≥ N).
- `overlap(query: &Fragment, &LshIndex, thresholds) -> Vec<Match>` — one-vs-many dedup lookup with confirmed score (simhash Hamming band → tlsh2/strsim confirm).
- No filesystem, DB, network, or tree-sitter. 100% table-testable.

### 2.2 `vox-skill-discovery` (L3, orchestration + IO)
- **Source adapters → `Fragment`s:**
  - `CodeBlockMiner` — tree-sitter `.vox`, normalized AST blocks ≥ K tokens (rename-normalized identifiers).
  - `PromptFlowMiner` — recurring prompt/flow text from `vox-db` session transcripts (opt-in).
  - `InstalledCatalog` — `SkillRegistry` manifests (+ raw bodies) and `vox-mcp-registry` tools as reference Fragments.
  - `RegistrySource` — external skills via `SkillsRegistryClient` (offline-tolerant).
- **`Candidate`** — `kind ∈ {RepeatedCode, RepeatedPrompt, DuplicatesInstalled, ImportableExternal}`, `members: Vec<Location>` (file:line / skill id / registry id), `score`, `suggested_action`, `draft_frontmatter: Option<SkillFrontmatter>` (advisory `name`/`description`/`category`/`tags`).
- **`Reporter`** — content-hash-keyed cache in `vox-db` (review-only-changed, modeled on `visus_review`); emits terminal / JSON / markdown.
- **CLI** `vox skill discover [--source code,prompts,installed,registry] [--format terminal|json|markdown] [--scaffold]`. `--scaffold` writes a draft `SKILL.md` only; never installs/publishes.

## 3. Data flow
`scan selected sources → Fragments → blake3 exact-dedup → LshIndex → {cluster for recurrence} + {overlap vs installed/registry} → Candidates → cache + report`.

## 4. SSOT byproducts (free wins)
- `InstalledCatalog` validates each skill's declared `tools` against `vox-mcp-registry`; emits a finding on drift and builds a `tool → skills` reverse index. Partial closure of the MCP↔skill SSOT gap at no extra cost.

## 5. Config (`vox-config`)
`min_block_tokens` (K, default 40), `min_occurrences` (N, default 3), simhash Hamming band, tlsh2/strsim confirm threshold. All overridable in `Vox.toml`.

## 6. Edge cases / error handling
- Tree-sitter parse failure → skip file with warning, continue.
- No transcripts / registry offline → that source yields zero; others proceed (per-source graceful degradation).
- Empty repo / below threshold → clean "no candidates", exit 0.
- Engine is read-only except `--scaffold` (writes a draft file) and the `vox-db` cache.

## 7. Testing
- `vox-similarity`: deterministic units — known clones cluster, dissimilar don't, threshold boundaries, signature stability across runs.
- `vox-skill-discovery`: golden-fixture repo with planted duplicate `.vox` blocks → asserted Candidates; mock `InstalledCatalog` for dedup; offline `RegistrySource`; transcript fixture for `PromptFlowMiner`.

## 8. Non-goals (this subsystem)
Publishing, signing, federation, agentic review of submissions, and the GUI surface — those are subsystems B/C/D with their own specs.

## 9. Execution-target shaping (Antigravity / Gemini 3.5 Flash)
The implementation plan derived from this spec MUST: make every task atomic + green + committed; precede every symbol/path reference with a verify (`rg`/read) step and inline exact signatures; keep tasks self-contained (repeat context); tag each task `[PARALLEL-SAFE]`/`[SEQUENTIAL]` by file-write disjointness; apply a two-strike circuit breaker; one decision per step; VoxScript-only automation; `cargo fmt -p <crate>` (never `--all`); frontmatter on any `docs/src/` file.
</content>
