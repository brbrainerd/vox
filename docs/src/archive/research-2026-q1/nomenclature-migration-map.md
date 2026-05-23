---
title: "Nomenclature migration map (SSOT)"
description: "Canonical English terms, Latin CLI aliases, and legacy identifiers for the Vox codebase."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Nomenclature migration map (SSOT)

**Policy:** Documentation and storage use **English-first** names. **Latin** names remain valid **CLI routes and aliases** where they add identity (see [CLI reference](../reference/cli.md)).

## Concept dictionary

| Canonical (English) | Meaning | Latin / product alias | Legacy / internal tokens |
|---------------------|---------|------------------------|---------------------------|
| **mesh** | Distributed coordination: Populi registry, HTTP control plane, `VOX_MESH_*` | **Populi** (mesh layer) | `mens` in some TOML keys and paths (deprecated; prefer `[mesh]`) |
| **model** | Native ML stack: weights, LoRA/QLoRA, `vox mens` commands | **Mens** | Module path `vox_populi::mens::*`; data dir `mens/` |
| **secrets** | Credential resolution (Clavis) | **Clavis** | `vox clavis` |
| **speech** | STT / audio | **Oratio** | `vox oratio` / `vox speech` |
| **training** | Curriculum / fine-tuning workflows | **Schola** | `vox schola` |

## Crate and path truth (2026-03)

| Incorrect / phantom | Correct |
|---------------------|---------|
| Crate **`vox-mens`** (removed) | **`vox-populi`** with `mens` module: `crates/vox-populi/src/mens/tensor/...` |
| Crate **`vox-codex-api`** | **Codex HTTP** surface in **`vox-db`** (and `vox` CLI); no separate `vox-codex-api` package |
| Split compiler crates (`vox-lexer`, `vox-parser`, …) as workspace members | **`vox-compiler`** monolith: `lexer`, `parser`, `hir`, `typeck`, `codegen_*` modules |

<a id="latin_ns-command-registry-group-labels"></a>
## `latin_ns` (command-registry group labels)

Values come from [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml). They are **telemetry / grouping buckets**, not extra argv you must type. Optional Latin **routes** are `vox fabrica`, `vox diag`, `vox ars`, `vox mens`, `vox recensio` (see [CLI reference](../reference/cli.md)); English paths remain canonical.

| `latin_ns` | Theme (mnemonic) | Example English commands |
|------------|------------------|---------------------------|
| `fabrica` | Workshop / compiler lane | `build`, `check`, `run`, `fmt`, `lsp`, `completions`, `oratio` (speech), `script` (feature-gated) |
| `diag` | Diagnostics lane | `doctor`, `architect`, `stub-check` — Latin: `vox diag …` |
| `ars` | Craft / integrations lane | `clavis`, `snippet`, `share`, `openclaw`, `skill`, `ludus` (and subcommands) |
| `codex` | Database & Codex-shaped workflows | `codex`, `db`, `scientia` (publication pipeline) |
| `ci` | Repository guard suite | `vox ci <subcommand>` |
| `mens` | Model / native ML (`vox mens …`) | `train`, `corpus`, `merge-qlora`, … |
| `recensio` | Review / audit (feature-gated) | `review` |
| `dei` | DEI daemon control plane | `vox dei …` |

**No `latin_ns`:** Some operations omit the field (e.g. `populi`, `island` in the registry). That means they are grouped under English top-level names only; add `latin_ns` only if you introduce a documented Latin umbrella for them.

## `product_lane` (bell-curve grouping metadata)

`product_lane` is distinct from `latin_ns`. It groups commands and docs by the kind of software Vox is optimizing for, not by CLI theme.

| `product_lane` | Meaning | Typical examples |
|----------------|---------|------------------|
| `app` | full-stack app construction | `build`, `run`, `island`, `fabrica` |
| `workflow` | automation and background execution | `script`, `populi` |
| `ai` | generation, review, eval, orchestration, speech | `mens`, `review`, `dei`, `oratio` |
| `interop` | approved bindings and remote capability bridges | `openclaw`, `skill`, `snippet`, `share` |
| `data` | database and publication workflows | `db`, `codex`, `scientia` |
| `platform` | packaging, compliance, diagnostics, and secrets | `pm`, `ci`, `doctor`, `clavis` |

## CLI command migrations

| Old | New | Notes |
|-----|-----|-------|
| `vox ci no-vox-orchestrator-import` | `vox ci no-dei-import` | Alias: `no-vox-orchestrator-import` |
| `vox ci mens-gate` | `vox ci mesh-gate` | Alias: `mens-gate` |
| `vox share review` | `vox share feedback` | Alias: `review` |
| `vox populi local-status` | `vox populi registry-snapshot` | Alias: `local-status` |
| `vox clavis doctor` | `vox clavis status` | Alias: `doctor` |

## Skill bundle ids

| Legacy | Canonical |
|--------|-----------|
| **`vox.mens`** (bundled `populi.skill.md`) | **`vox.populi`** — [`SkillRegistry::get`](../../../crates/vox-skills/src/registry.rs) and `uninstall` treat `vox.mens` as an alias for `vox.populi`. |

## Doc link canonicals

| Broken / misleading | Use instead |
|---------------------|-------------|
| `reference/populi.md` (mesh SSOT) | [`reference/populi.md`](../reference/populi.md) |
| `architecture/mens-ssot.md` | [`reference/populi.md`](../reference/populi.md) |

## Rust symbols (internal disambiguation)

| Previous | Current | Notes |
|----------|---------|--------|
| `vox_compiler::typeck::Severity` | `TypeckSeverity` | Distinct from TOESTUB / lint severities |
| Duplicated `vox_compiler::eval` | `pub use vox_eval::*` | Single SSOT crate: **`vox-eval`** |
| `vox_cli::training::native::VoxTransformer` | `CliDogfoodTransformer` | Avoids clashing with Populi `VoxTransformer` |
| `vox_repository::VoxMeshToml` | `MeshToml` | Type alias (same struct); prefer `MeshToml` in new Rust code |

## Workspace / experimental

| Item | Status |
|------|--------|
| **`crates/vox-py`** | **Excluded** from the root workspace (`Cargo.toml` `[workspace.exclude]`); `docs/src/reference/cli.md` is a bindings guide for when the tree is enabled. |

## See also

- [Glossary: Vox Terminology](../explanation/glossary.md)
- [Command compliance](../reference/command-compliance.md)
- [Governance](../../agents/governance.md) — naming discipline


