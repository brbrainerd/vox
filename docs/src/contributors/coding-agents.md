---
title: "Coding Agent Instructions"
description: "Quick-reference heuristics, TOESTUB rule table, and pre-commit checklist for AI coding agents operating on Vox."
category: "Contributors"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "High-signal quick-reference loaded as agent context; each section is a directly actionable rule."

schema_type: "TechArticle"
---

# Coding Agent Instructions

Quick-reference for AI agents operating on the Vox codebase. Deep rationale lives in the linked SSOTs — this file stays thin by design.

## Stale documentation risk

1. **Check SSOT inventories first** — verify similar features aren't retired. Cross-reference `AGENTS.md` and [legacy-retirement-roadmap.md](../archive/research-2026-q1/legacy-retirement-roadmap.md).
2. **Beware renamed crates** — `vox-dei` was retired; the orchestrator is `vox-orchestrator`. See the retired surfaces table in `AGENTS.md`.
3. **Do not hallucinate surfaces** — if a crate isn't in [`where-things-live.md`](../architecture/where-things-live.md) or `AGENTS.md`, do not assume it exists.
4. **Search before modifying** — use `grep_search` and `view_file` before touching large modules.

## Structural limits (enforced by TOESTUB, fail CI)

| Limit | Value | Rule ID |
| --- | --- | --- |
| Max file length (non-blank lines) | 500 | `arch/god_object` |
| Max methods per struct/impl | 12 | `arch/god_object` |
| Max files per directory | 20 | `arch/sprawl` |
| No `todo!()` / `unimplemented!()` | Zero in production | `stub/todo` |
| No hollow functions | No trivially-default returns | `skeleton/hollow-fn` |
| No hardcoded secrets | Use vox-secrets | `security/hardcoded-secret` |
| No CRLF line endings | LF only | `cross-platform/crlf` |
| `pub fn` in golden `.vox` must have a test | `@test` block referencing the fn | `skeleton/no-test-for-pub-fn` |

Full fix guide: [TOESTUB contributor guide](toestub-contributor-guide.md).
Policy SSOT: [Architectural governance](../../agents/governance.md).

## AI inner-loop: reduce Cargo overhead

Coding agents often spawn **many shells**. Each distinct **`CARGO_TARGET_DIR`** is a **separate incremental cache** — the workspace [`.cargo/config.toml`](../../../.cargo/config.toml) pins **`target/`** for a reason.

**Do:**

- Run **`vox ci dev-loop-audit`** at the start of a focused session (or **`--json`** for tooling).
- Prefer **`cargo check -p <crate>`** and **`cargo nextest run -p <crate> --profile ci`** (or a **filtered** **`cargo test`**) while iterating.
- Use **`vox ci pre-push`** (optionally **`--quick`**) when you intend to **push**, not after every edit.
- Emit timings occasionally: **`vox ci pre-push --report-json target/local/pre-push-last.json`**.

**Avoid:**

- Switching **`CARGO_TARGET_DIR`** between **`target`**, **`target-agent-ssot`**, **`target-ci-*`** during the same task unless intentionally isolating.
- Using **`vox ci pre-push`** as the first compile check after a one-crate change.

Full rationale and thresholds: [AI dev loop overhead (2026)](../architecture/ai-dev-loop-overhead-2026.md).

## Pre-commit victory checklist

Run these before marking any task complete. Tiers are ordered — fix earlier tiers first.

```bash
# Tier 1 — zero stubs
cargo run -p vox-cli --features stub-check -- stub-check crates/<your-crate>

# Tier 3 — compile (prefer scoped — avoids workspace-wide churn)
cargo check -p <your-crate>

# Tier 5 — unit tests (for code changes)
cargo test -p <your-crate>
# Or: cargo nextest run -p <your-crate> --profile ci

# Tier 6 — .vox parse rate (if .vox files changed)
cargo run -p vox-cli -- corpus eval --mode ast examples/golden/

# Tier 7 — CI guards
cargo run -p vox-cli -- ci ssot-drift
cargo run -p vox-cli -- ci line-endings

# Tier 7b — orphan snapshot cleanup (if .snap files changed)
cargo run -p vox-cli -- snapshot orphans
```

Full 9-tier model: [`vox_agentic_loop_and_mens_plan.md`](../archive/research-2026-q1/vox_agentic_loop_and_mens_plan.md) §9-Tier Victory Conditions.

## Task brief format: failing test first

When assigning a task to an AI agent — or starting a task yourself — express the requirement as a **failing test before any implementation brief**. This is the most unambiguous spec format available:

**For Rust changes:**

```rust
#[test]
fn share_tunnel_url_includes_port() {
    let url = TunnelUrl::new("localhost", 7700);
    assert_eq!(url.to_string(), "vox://localhost:7700");
}
// impl TunnelUrl does NOT exist yet — the test is the spec
```

**For Vox golden examples:**

```vox
// vox:skip
@test
fn greet_returns_full_name() {
    let result = greet("Ada", "Lovelace");
    assert result == "Hello, Ada Lovelace";
}
// fn greet does NOT exist yet — write it until this passes
```

Paste the failing test as the first content of any implementation request. Benefits:

- Eliminates ambiguity about inputs, outputs, and edge cases.
- Acts as an automated linter on AI-generated output — code that doesn't pass the test is wrong, regardless of how plausible it looks.
- Keeps the agent in a verifiable loop: write → run → diagnose → fix.
- Test-covered implementation enters the MENS corpus as a higher-quality positive example.

When a task cannot be expressed as a failing test (e.g., pure doc updates, config tweaks), state that explicitly rather than skipping the check silently.

**Scaffold shortcut (Vox):** `vox new fn <name> [--params "x: int"] [--returns int] [--in path/to/file.vox]` writes a paired `fn` + `@test` block in one keystroke. The emitted test references undefined placeholders (`_`, `_expected`) so the function won't compile until you fill in the expected behavior — the friction reducer for [AGENTS.md §Test-First Policy](../../../AGENTS.md). Pass `--stdout` to pipe the stub into your editor instead of writing to disk.

## Snapshot discipline

Insta snapshot tests (`.snap` files under `crates/*/src/tests/snapshots/`) are
part of the test suite. When your change alters compiler output, codegen output,
or diagnostic formatting, the snapshot changes are part of the same commit as
the code change — never in a follow-up "fix snapshots" commit.

**Workflow:**

```bash
# Run the affected tests — they will fail with a snapshot diff
cargo test -p vox-compiler

# Accept the new snapshots
cargo insta review      # interactive
cargo insta accept      # accept all pending (use only if you've reviewed the diff)

# Stage and commit the .snap files alongside your code change
git add crates/vox-compiler/src/tests/snapshots/
git commit --amend --no-edit   # or include in the same commit
```

**Orphan `.snap` files** (snapshots for deleted or renamed tests) must be
deleted. CI treats orphans as a warning via `cargo insta test --unreferenced=reject`.
See [contribution-loop.md](contribution-loop.md) for the orphan-check step.

**Never commit a snapshot you haven't read.** Blind `insta accept` on a failing
test masks real regressions. Review the diff: if the new output is correct,
accept; if not, fix the code.

## Corpus quality signal

Your code changes feed the MENS training pipeline. Stub-free, test-covered,
parse-passing contributions become positive training examples. Stubs and parse
failures become negative examples — the model learns to avoid those patterns.

See [contribution loop](contribution-loop.md) for the full flywheel and the @test-first gate.

## Panic prevention (do not shortcut)

- Do **not** use `git reset --hard`, `git restore`, or `git clean` to silence failing tests.
- Do **not** delete tests to fix a test failure — fix the code.
- Do **not** add `#[allow(...)]` or `// toestub-ignore(...)` without a written reason.
- Do **not** claim task completion adjacent to `todo!()` or empty bodies.
- Do **not** run `cargo insta accept` without reading the snapshot diff first.

Research: [AI agent panic and shortcut pathology](../archive/research-2026-q1/research-ai-panic-shortcuts-2026.md).

## Key SSOTs

| Need | SSOT |
| --- | --- |
| Secrets / credentials | [Secrets SSOT](../reference/secrets-ssot.md) |
| CLI command additions | [CLI design rules SSOT](../archive/research-2026-q1/cli-design-rules-ssot.md) |
| Retired symbols / crates | [AGENTS.md §Retired Surfaces](../../../AGENTS.md) |
| God-object refactor protocol | [God object defactor checklist](../archive/research-2026-q1/god-object-defactor-checklist.md) |
| `.vox` code in docs | [Documentation governance](documentation-governance.md) |
| Testing file conventions | [Testing standard](../archive/research-2026-q1/testing-standard.md) |
| Cryptography | [Cryptography SSOT](../architecture/cryptography-ssot-2026.md) |

## Enforcement

Operations are checked by `AGENTS.md` + CI. Prefer decomposition over shell cleverness. When in doubt, read the SSOT.
