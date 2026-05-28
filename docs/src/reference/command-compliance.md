---
title: "Command compliance"
description: "Official documentation for Command compliance for the Vox language. Detailed technical reference, architecture guides, and implementation"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Command compliance

## Command projection pipeline (SSOT chain)

When changing what users type at `vox …`, treat these artifacts as one pipeline (edit → regenerate → verify):

| Stage | Artifact | How it changes |
|-------|----------|----------------|
| Human SSOT | `contracts/operations/catalog.v1.yaml` | Edit CLI rows under each operation's `cli:` block (`path`, `handler_rust`, `latin_ns`, …). |
| Generated registry | `contracts/cli/command-registry.yaml` | Regenerate with **`vox ci operations-sync --target cli --write`** (do not hand-edit except via bootstrap workflows documented elsewhere). |
| Embedded registry | `crates/vox-cli/src/command_contract.rs` (`include_str!`) | Rebuilt when the YAML changes (see `crates/vox-cli/build.rs` rerun-if-changed). |
| Runtime parse tree | `crates/vox-cli/src/lib.rs` (`VoxCliRoot` / `Cli`) | Keep user-visible paths aligned with registry rows; dispatch lives in `cli_dispatch/` (including Latin lane rewrites). |
| Discoverability output | `crates/vox-cli/src/command_catalog.rs` | Derived from the live **clap** tree — registry parity alone does not prove nested dispatch works. |
| Compliance gate | **`vox ci command-compliance`** | Validates registry ↔ docs ↔ `lib.rs` ↔ compilerd/dei surfaces ↔ MCP ↔ capability rows (see table below). |

**`vox ci command-compliance`** validates the machine-readable registry **`contracts/cli/command-registry.yaml`** (JSON Schema: **`contracts/cli/command-registry.schema.json`**) against:

| Check | Source |
|-------|--------|
| Top-level `vox` subcommands exist in `Cli` | `crates/vox-cli/src/lib.rs` |
| Doc needles for `ref_cli_required` operations | Canonical body: `docs/src/reference/cli.md`. Legacy redirect `docs/src/ref-cli.md` (if present) is merged into the compliance read for stable links — checks **always** run (no skip). `vox ci …` and `vox codex` subcommands are validated **only inside** their `### \`vox ci …\`` / `### \`vox codex\`` sections (not whole-file substring matches) |
| Top-level reachability table rows | `docs/src/reference/cli.md` under **CLI command reachability** (legacy `cli-reachability.md` merged there; rows skipped for `completions`, `fabrica`, `mens`, `ars`, `recensio`, and when `reachability_required: false`) |
| Registry metadata enums | `latin_ns` and `product_lane` values are validated against the command-registry schema and `vox-cli` validators |
| `product_lane` required on `vox-cli` rows | Active / deprecated `surface: vox-cli` operations must declare `product_lane` (retired/internal rows exempt from handler checks only) |
| Feature-growth projection gate | `docs/src/architecture/feature-growth-boundaries.md` must name `projection_parity` / `projection_triplet_is_deterministic` and the `cargo test -p vox-compiler --test projection_parity` reproducer |
| Rust ecosystem policy gate docs | `docs/src/reference/rust-ecosystem-support-contract.md` must include both `vox ci rust-ecosystem-policy` and `cargo test -p vox-compiler --test rust_ecosystem_support_parity` |
| Compiler daemon RPC method names | `crates/vox-cli/src/compilerd.rs` |
| DeI daemon RPC method ids | `crates/vox-cli/src/dei_daemon.rs` |
| MCP tool registry vs schema + handlers | `contracts/mcp/tool-registry.canonical.yaml` validated against **`contracts/mcp/tool-registry.schema.json`** (requires `product_lane` per tool); tool names vs `handle_tool_call` in **`crates/vox-orchestrator-mcp/src/dispatch.rs`**; **`crates/vox-orchestrator-mcp/src/lib.rs`** must `pub use vox_mcp_registry::TOOL_REGISTRY`; handler arms parsed inside `match name { … }` up to the first line that matches `^\s*_\s*=>` (indent-tolerant). Wire aliases are SSOT **`contracts/mcp/tool-wire-aliases.v1.yaml`** (compiled into `TOOL_WIRE_ALIASES` via `vox-orchestrator-mcp/build.rs`), validated by this gate — not duplicated in `match`. |
| Capability registry | **`contracts/capability/capability-registry.yaml`** (**generated** from the operations catalog) vs **`contracts/capability/capability-registry.schema.json`**; cross-check curated `cli_paths` against **active** `vox-cli` paths and `mcp_tool` names against the MCP registry; capability exemption paths must exist. Edit [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml) (`capability:` block + rows), then `vox ci operations-sync --target capability --write`. See [Capability registry SSOT](../archive/research-2026-q1/capability-registry-ssot.md). Regenerate **`contracts/capability/model-manifest.generated.json`** with **`vox ci capability-sync --write`** after registry changes |
| Operations catalog parity | Single human-edited **`contracts/operations/catalog.v1.yaml`** vs **`contracts/operations/catalog.v1.schema.json`**; verifies committed MCP + CLI + capability YAML match catalog projections, dispatch/`input_schemas.rs`/read-role governance, and updates `contracts/reports/operations-catalog-inventory.v1.json` (`vox ci operations-verify`; bootstrap rows via `vox ci operations-sync --target catalog --write`) |
| Script duals | `command-surface-duals.md` or `scripts/README.md` must mention each `script_duals` canonical CLI and script stem |

**CI {** `.github/workflows/ci.yml` runs this gate after **`vox ci check-docs-ssot`** (after **`vox ci line-endings`** and other early guards; see [workflow enumeration](../ci/workflow-enumeration.md)).

**Definition of done** for a new shipped CLI operation: follow the **Command projection pipeline** table above (catalog → sync → clap/dispatch), then registry row + docs + **`command-compliance`** green (see [`cli.md`](cli.md) CLI design rules).

For fast local policy iteration across this lane, use **`vox ci policy-smoke`** (`cargo check -p vox-orchestrator`, in-process command-compliance, then the same `cargo test -p vox-compiler --test rust_ecosystem_support_parity` used by **`vox ci rust-ecosystem-policy`**).

