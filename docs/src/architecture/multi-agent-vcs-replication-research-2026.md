---
title: "Multi-Agent VCS Replication — Landscape Research (2026-05-03)"
description: "Survey of version-control and CRDT systems evaluated for a multi-agent code-collaboration substrate. Finds no off-the-shelf project provides codebase-scale auto-converging replication; recommends building op-log gossip on top of existing jj-lib + Populi mesh investment (Path 1) over pivoting to Pijul (Path 2) or an Automerge-based hybrid (Path 3). Companion to the spec doc."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Canonical justification for why Vox builds its own op-log gossip protocol on jj rather than adopting Pijul or Automerge. Names the projects evaluated, their fitness scores, and the decision criteria so future agents don't re-run the survey."
sourced_at: "2026-05-03"
vox_relevance:
  - "vox-orchestrator: jj_backend, a2a/ mesh transport, conflict_manager"
  - "vox-populi: P2P transport substrate for op-log gossip"
  - "vox-git: gix-based git bridge (interop layer with external git remotes)"
---

# Multi-Agent VCS Replication — Landscape Research (2026-05-03)

> **Companion spec:** [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) — the architecture this research informs.

## Premise

Vox already runs multiple coding agents (Claude Code instances, MENS workers, mesh peers) against shared codebases. Today, every agent that wants to change code does so in an isolated git worktree (the `.claude/worktrees/<branch>/` pattern), and humans serialize the work back together through ordinary git PRs. As fleet sizes grow — 5, 10, 20 concurrent agents per user, plus humans on the Populi mesh — manual merging becomes the dominant cost.

The goal: **a substrate where agent and human edits auto-converge when they don't semantically conflict, and surface clean, navigable conflicts when they do.** No `<<<<<<<` markers as the default outcome. No PR queue as the only path to integration.

## Question

Does a project exist that provides this out of the box, or must Vox build it?

## Method

Web research conducted 2026-05-03 across active VCS, CRDT, P2P-sync, and multi-agent-coding projects. Sources cited inline. Each candidate scored on:

1. **Native distributed replication** — does the project ship a wire protocol, or is it left as an exercise?
2. **Convergence math** — does the merge model commute when changes are independent, or does it always require a human?
3. **Project health** — is the project actively maintained at a tempo Vox can depend on?
4. **Codebase-scale evidence** — has anyone run the system on a real source tree, not a research demo?

## Findings

### Capsule landscape

| Project | Native replication? | Convergence math | Project health (2026) | Fit |
|---|---|---|---|---|
| **Jujutsu (jj)** | git wire only — no native op-log sync | Local first-class conflicts; partial via op-log | Strong, Google-backed, weekly releases | **Medium** — fantastic local model, mesh layer must be built |
| **Pijul** | ✅ patch push/pull | ✅ Patch theory: independent patches commute | Glacial — ~1 maintainer, no 1.0, 4-year-old roadmap | Medium-high theory, **low practice** |
| **Automerge / Patchwork** | ✅ sync protocol (Automerge-Repo 2.0, May 2025) | ✅ CRDT (op-based) | Active research at Ink & Switch | **Medium** — document-scale only; codebase-scale unproven |
| **Yjs** | ✅ y-websocket / y-webrtc | ✅ CRDT | Very mature | **Low** — wrong shape for VCS (no commit/branch/history) |
| **Sapling (Meta)** | ✅ on Mononoke | git-style merge | Client open, **server (Mononoke) Meta-internal only** | **Low** — server cannot be built on externally |
| **Heptapod (Mercurial+evolve)** | ✅ via hg | Rebase-based, not auto-converging | Active but pivoted to paid SaaS May 2025 | **Low** |
| **Iroh / iroh-docs / Hypercore** | ✅ QUIC P2P, gossip CRDTs | n/a (transport only) | Production-quality (iroh 0.35+, hypercore v10 LTS) | **High as transport, zero as VCS** |
| **Devin / Cursor / Augment / GitHub Agent Mode** | n/a | n/a | Active products (Feb 2026) | **None** — all serialize through ordinary git PRs |

### Key sources

- Jujutsu operation log (no replication protocol): <https://jj-vcs.github.io/jj/latest/operation-log/>, [CHANGELOG](https://github.com/jj-vcs/jj/blob/main/CHANGELOG.md)
- Pijul status: [Nest changes](https://nest.pijul.com/pijul/pijul/changes), [Linux Magazine 2025](https://www.linux-magazine.com/Issues/2025/292/Pijul), comparative analysis at [debugg.ai](https://debugg.ai/resources/git-successor-jujutsu-jj-sapling-pijul-stacked-diffs-monorepos-2025)
- Automerge-Repo 2.0 release: <https://automerge.org/blog/2025/05/13/automerge-repo-2/>
- Patchwork (Ink & Switch): <https://www.inkandswitch.com/patchwork/notebook/08/>, [Dispatch 004](https://www.inkandswitch.com/newsletter/dispatch-004/)
- Mononoke (Meta-internal): <https://github.com/facebook/sapling/blob/main/eden/mononoke/README.md>
- Iroh: <https://docs.iroh.computer/what-is-iroh>, [iroh-docs](https://github.com/n0-computer/iroh-docs), [distribits 2025 talk](https://www.distribits.live/talks/2025/bruynooghe-iroh-p2p-quic-transport-and/)
- Hypercore: <https://github.com/holepunchto/hypercore>
- ICSE 2025 — "Semantic-Aware Replicated Data Types for Improved Conflict Resolution in Near-Synchronous Code Collaboration" (referenced via debugg.ai survey)
- Augment Code 2026 product survey: <https://www.augmentcode.com/tools/8-top-ai-coding-assistants-and-their-best-use-cases>

### Per-project notes

#### Jujutsu (jj)

- **Local model is correct.** Op-log gives lock-free local concurrency (multiple jj processes against shared FS won't corrupt). First-class conflicts — a file in conflict is a persisted state, not a transient diff.
- **No native wire protocol.** `jj git push` falls back to git-pack-style transport. There is no `jj op push` / `jj op pull` / `jj-cloud`.
- **Google's internal usage** runs against Piper/CitC; that backing-store integration is not public. We cannot adopt their distributed story.
- **Strength of fit: medium.** All the local primitives we need exist; the mesh layer is greenfield.

#### Pijul

- **Patch theory is exactly the math we want.** When five agents add non-overlapping functions to the same file, the patches commute and converge with zero merge step.
- **Push/pull is real.** SSH and HTTP transports exchange patches (not snapshots), making the wire protocol structurally closer to "ship an op-log" than git's pack model.
- **The Nest is a centralized hub.** No built-in P2P mesh — we'd still need the gossip layer.
- **Project health is the killer.** One active maintainer, no 1.0, IDE/tooling story stagnant since 2022. HN consensus: "academically beautiful, mainstream-irrelevant" ([cohost: pijul is dead, long live jj](https://debugg.ai/resources/git-successor-jujutsu-jj-sapling-pijul-stacked-diffs-monorepos-2025)).
- **Strength of fit: medium-high theory, low practice.** Adopting Pijul means betting Vox's foundation on a project with one maintainer. That bet compounds for years.

#### Automerge / Patchwork

- **Automerge 3 (2025)** cut memory ~10x. Automerge-Repo 2.0 (May 2025) shipped improved sync over WebSocket / MessageChannel / custom adapters.
- **Patchwork** (Litt, Sonnentag, vanHardenberg, Wiggins, Henry — Ink & Switch) explicitly explores "universal version control" with branches, history, diffs as CRDT primitives, including AI-bot-on-branch experiments.
- **Document-scale.** Maintainers note the sync server "struggles with large documents" because docs load fully in memory. No production system uses Automerge as a whole-source-tree substrate.
- **Strength of fit: medium.** Right research direction, wrong scale today. Adopting it means co-investing in research, not shipping product.

#### Yjs

- y-websocket and y-webrtc are mature for collaborative editing (Monaco/CodeMirror bindings ship), but everything is per-document. No project-wide commit graph, no branching, no checkout-this-version.
- **Strength of fit: low.** Wrong shape for VCS. Could be an in-progress live-edit layer if we wanted that, but we don't yet.

#### Sapling / Mononoke

- Sapling client is open and usable on top of git. Mononoke (server) is **"used in production within Meta but not yet supported for external usage"** — this is the killer. Without Mononoke, we have a polished CLI on top of git, not a remote-first system.
- **Strength of fit: low.** Server-side cannot be built on externally.

#### Iroh / Hypercore / Earthstar

- **Iroh** is the most interesting transport candidate. iroh-blobs hit production at 0.35; iroh-docs offers eventually-consistent KV-over-blobs+gossip. QUIC + holepunching is the P2P-mesh transport story.
- **Holepunch / Hypercore** is also active and explicitly LTS at v10.
- These are the right *pipes*. They don't include a merge brain.
- **Strength of fit: high as transport, zero as VCS.**

#### Multi-agent code-collab products (2025-2026)

- **No one has shipped a shared-state architecture.** Cursor's "concurrent agents," GitHub Agent Mode (Feb 2026), Augment Intent — all serialize through ordinary git PRs. Devin runs in sandbox VMs with no shared state.
- The whitespace is real. Vox would be first.

## Conclusion

There is no off-the-shelf project that hits the target. Every viable substrate either lacks the convergence math (Yjs, Sapling, git itself), lacks the project velocity to depend on (Pijul), lacks codebase-scale evidence (Automerge), or — in jj's case — has the local model right and leaves replication as an exercise.

**Three paths remain:**

1. **Path 1 — Stay on jj, build op-log gossip on Iroh / Populi.** Smallest delta from current investment in [`jj_backend.rs`](../../../crates/vox-vcs/src/jj_backend.rs) and [`a2a/`](../../../crates/vox-orchestrator/src/a2a/). The protocol is the work; jj's local semantics handle the rest. Effort: ~1–2 quarters.
2. **Path 2 — Pivot to Pijul.** Native patch theory removes the convergence-math work. But we'd take a project dependency on a single-maintainer project at risk of dormancy. Larger refactor + ecosystem risk.
3. **Path 3 — Hybrid: Automerge live + jj durable.** Ink & Switch's research direction adapted for code. Highest effort (two systems, semantic-merge layer on top), bets on unsolved research.

### Recommendation: Path 1

- Path 1 keeps every piece Vox has built — `jj_backend.rs`, the [A2A mesh](../../../crates/vox-orchestrator/src/a2a/), the [Populi transport](populi-mesh-north-star-2026.md), the [conflict manager](../../../crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs) — and adds the missing piece (op-log gossip) as a self-contained protocol.
- Pijul's patch-theory ideas inform the merge algorithm without requiring a project dependency. The ICSE 2025 "Semantic-Aware Replicated Data Types" paper is the closest published work on the agent-aware merge problem and informs the conflict-classification design.
- Path 2 is rejected on project-health grounds (single-maintainer dependency too risky).
- Path 3 is rejected as research-grade for the v1 horizon; revisit after Path 1 ships and we have empirical conflict-rate data.

## Cross-references

- **Companion spec:** [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) — turns this recommendation into a concrete architecture.
- **Mesh foundation:** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md), [`populi-mesh-a2a-durability-spec-2026.md`](populi-mesh-a2a-durability-spec-2026.md).
- **Orchestrator context:** [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md) §multi-agent coherence.
- **Existing code surfaces:** [`crates/vox-vcs/src/jj_backend.rs`](../../../crates/vox-vcs/src/jj_backend.rs), [`crates/vox-orchestrator/src/a2a/`](../../../crates/vox-orchestrator/src/a2a/), [`crates/vox-orchestrator/src/mcp_tools/vcs_tools/`](../../../crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs), [`crates/vox-git/`](../../../crates/vox-git/).
- **External:** Iroh docs <https://docs.iroh.computer>, jj op-log <https://jj-vcs.github.io/jj/latest/operation-log/>, Pijul <https://pijul.org>, Automerge <https://automerge.org>.
