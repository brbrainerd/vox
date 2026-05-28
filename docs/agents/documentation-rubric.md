---
title: "Documentation Rubric"
description: "Agent support documentation for documentation rubric"
category: "contributor"
status: "current"
training_eligible: true
---
# Documentation rewrite rubric (Vox)

Use this rubric when improving comments, rustdoc, or prose docs so edits stay aligned with repo SSOT and remain useful for humans and LLM agents.

## Read first (SSOT and style)

| Source | Use for |
|--------|---------|
| [AGENTS.md](../../AGENTS.md) | Architecture, terminology, `missing_docs` policy, Codex naming; optional `last_updated` in frontmatter is secondary to Git history (`documentation-governance.md`) |
| [docs/src/reference/style-guide.md](../src/reference/style-guide.md) | Voice, snippets, headings, cross-links |
| [docs/src/adr/002-diataxis-doc-architecture.md](../src/adr/002-diataxis-doc-architecture.md) | Doc taxonomy, **required frontmatter**, `training_eligible` |
| [docs/src/adr/004-codex-arca-turso-ssot.md](../src/adr/004-codex-arca-turso-ssot.md) | Codex / Arca / Turso / env vars |
| [docs/src/reference/external-repositories.md](../src/reference/external-repositories.md) | Repository id, layout, multi-repo wording |
| [docs/agents/governance.md](governance.md) | Review expectations, TOESTUB-sensitive patterns |

## What to keep

Keep a comment or doc line only if it answers at least one of:

- **Why** this exists (intent, not a restatement of the name).
- **Why** this order/branch matters (correctness or performance).
- **What invariant** must hold for callers or maintainers.
- **What contract** this satisfies (HTTP, DB, MCP, file layout, etc.).
- **What fails** and how (errors, partial success, retries).
- **Where SSOT lives** if the detail belongs elsewhere (ADR, AGENTS, another module).

## What to rewrite

| Pattern | Action |
|---------|--------|
| Field doc repeats the identifier (`/// Foo` on `foo`) | Replace with role, units, valid range, or link to parent struct contract |
| Enum variant doc repeats the variant name | Replace with when this variant is produced/consumed |
| Long duplicate architecture | Replace with one line + link to ADR / AGENTS |
| Section banners (`// ---`) in huge files | Keep only if they are the primary navigation aid; otherwise fold into `//!` |
| Clap `///` on CLI fields | Treat as **user-facing help**: optimize for invocation clarity, not library rustdoc tone |

## What to delete

- Comments that only narrate the next line of code (`// increment i`).
- Stale or misleading comments (prefer delete over wrong).
- Victory / done claims that trigger governance noise (see [governance.md](governance.md)).

## Rustdoc vs inline `//`

- **`///` / `//!`**: public API, types, and anything an embedder or tool imports.
- **`//`**: algorithm steps, temporary reasoning, or internal-only caveats for maintainers. Do not duplicate full public contracts in `//` if they belong in rustdoc.

## Markdown pages under `docs/src/`

- Preserve **YAML frontmatter** per ADR 002; refresh `last_updated` when you touch substantive meaning (optional hint — CI may warn when it diverges from Git by policy).
- Prefer linking to SSOT over copying long architecture sections.
- For `training_eligible: true` pages, treat code blocks as training data: keep them accurate and minimal.

## Conflict resolution

If prose disagrees with **AGENTS.md** or an **ADR**, update the SSOT document (or add an ADR), then align other text. Do not “fix” architecture only in random comments.

## LLM migration anti-patterns (hybrid CI / scripts)

| Anti-pattern | Fix |
|--------------|-----|
| Claiming migration “done” without matching **tests** or **`vox ci`** guards | Tie claims to `crates/vox-cli/tests/*`, `ci.yml`, and `check-docs-ssot` artifacts. |
| Reintroducing **Python** or retired **`scripts/docs/*`** for inventory / gates | Use **`vox ci doc-inventory`** and **`vox ci mens-gate`** only. |
| **Fat wrappers** that re-implement guard logic | One-line delegates to **`vox ci …`** / `cargo run -p vox-cli -- ci …`; see [command surface duals](../src/ci/command-surface-duals.md). |
| Docs that cite **non-existent CLI** (`vox clean`, etc.) | Cross-check `ref-cli.md` and [command surface duals](../src/ci/command-surface-duals.md). |
| Omitting **`script-execution`** from compile matrices | `FEATURE_SETS` in `commands/ci/mod.rs` must include the script lane. |
| **Daemon / CLI drift** (e.g. `vox-compilerd` ignoring flags the CLI exposes) | Extend shared params (e.g. `run` **`mode`**) and add contract tests. |

## Before / after examples (mechanical → useful)

**Struct field**

- Before: `/// Weight` on `weight: f64`
- After: `/// Mix repeat factor; each source line is emitted ceil(max(weight,0)).max(1) times.`

**Enum variant**

- Before: `/// Primary` on `Primary`
- After: `/// Resolved from `data_dir/train.jsonl` when that file exists.`

**Markdown API page**

- Before: three paragraphs restating what the crate README already says.
- After: one **Authoritative sources** table (ADR / `AGENTS.md` / key `crates/...` paths) + task-oriented sections below.

## Machine-readable inventory (`doc-inventory.json`)

Regenerate with **`vox ci doc-inventory generate`** (CI/bootstrap: `cargo run -p vox-cli --quiet -- ci doc-inventory generate`). Verify with **`vox ci doc-inventory verify`**. Schema v3 adds `first_read_for_agents`, and each `symbol_hints[].hints[]` entry may include `containing_symbol`, `doc_preview`, `comment_type`, and `quality_tag` — use these fields to batch rewrites by comment class instead of raw line density alone.
