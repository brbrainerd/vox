# SP-1: Action Spine — Typed Arguments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich the clap-derived command catalog so every argument carries its typed kind (flag/value/count), enumerated possible values, and default values — the foundation SP-2's generated forms consume.

**Architecture:** `CommandCatalogArgument` (in `vox-cli`) is the single argument shape. Both the GUI action-manifest builder and the `gui-catalog-parity` CI guard inherit it via serde, so enriching the struct propagates everywhere. We extend the struct, the JSON-schema contract that the guard validates against, and the two hand-written TypeScript mirrors. We do **not** add a new CI guard — `gui-catalog-parity` already validates the generated manifest against the schema.

**Tech Stack:** Rust (clap 4 introspection, serde), JSON Schema (draft-07), TypeScript interfaces.

**Spec:** [`docs/superpowers/specs/2026-06-01-cli-gui-hybrid-spine-design.md`](../specs/2026-06-01-cli-gui-hybrid-spine-design.md) (Unit 1 — Action spine).

---

## File structure

| File | Responsibility | Change |
| --- | --- | --- |
| `crates/vox-cli/src/command_catalog.rs` | SSOT argument shape + clap extraction | Add `ArgValueKind` enum; add 3 fields to `CommandCatalogArgument`; enrich extraction; add unit test |
| `crates/vox-gui/src/commands/action_manifest.rs` | GUI manifest builder | Fix synthetic MCP-payload struct literal to set new fields (compile fix) |
| `crates/vox-cli/src/commands/ci/gui_catalog_parity.rs` | CI guard: regenerate + schema-validate manifest | Add new fields to synthetic MCP-payload JSON; add schema-validation regression test |
| `contracts/gui/action-manifest.v1.schema.json` | Contract the guard validates against | Add + require `value_kind`; add `possible_values`, `default_values` |
| `crates/vox-gui/ui/src/types/catalog.ts` | TS mirror of catalog arg | Add 3 fields to `CommandCatalogArgument` |
| `crates/vox-gui/ui/src/types/actionManifest.ts` | TS mirror of manifest arg | Add 3 fields to `ActionArgument` |

---

## Task 1: Enrich the argument shape + clap extraction

**Files:**
- Modify: `crates/vox-cli/src/command_catalog.rs:23-49` (struct), `:312-325` (extraction)
- Modify: `crates/vox-gui/src/commands/action_manifest.rs:280-287` (compile fix)
- Test: `crates/vox-cli/src/command_catalog.rs` (tests module, append)

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/command_catalog.rs` (before the closing `}`):

```rust
    #[test]
    fn catalog_arguments_carry_value_kind_and_possible_values() {
        let catalog = build_catalog();
        let commands = catalog
            .entries
            .iter()
            .find(|e| e.path == ["commands"])
            .expect("`commands` subcommand present in default build");
        // `--format` is a value_enum (text|json) → Value kind with possible values.
        let format = commands
            .arguments
            .iter()
            .find(|a| a.name == "format")
            .expect("`commands` has a `format` argument");
        assert_eq!(format.value_kind, ArgValueKind::Value);
        assert!(
            format.possible_values.iter().any(|v| v == "json"),
            "format should expose enum value 'json'; got {:?}",
            format.possible_values
        );
        // `--recommended` is a bool flag → Flag kind, no values.
        let recommended = commands
            .arguments
            .iter()
            .find(|a| a.name == "recommended")
            .expect("`commands` has a `recommended` argument");
        assert_eq!(recommended.value_kind, ArgValueKind::Flag);
        assert!(recommended.possible_values.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli catalog_arguments_carry_value_kind_and_possible_values`
Expected: FAIL — compile error `no field value_kind on type CommandCatalogArgument` / `cannot find value ArgValueKind`.

- [ ] **Step 3: Add the `ArgValueKind` enum and struct fields**

In `crates/vox-cli/src/command_catalog.rs`, immediately above `pub struct CommandCatalogArgument` (currently line 23), insert:

```rust
/// Typed argument kind for GUI form generation, derived from the clap action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgValueKind {
    /// Boolean flag (clap `SetTrue`/`SetFalse`) — presence-only, no value.
    Flag,
    /// Takes one or more values (clap `Set`/`Append`).
    Value,
    /// Repeatable counter (clap `Count`), e.g. `-vvv`.
    Count,
}
```

Then replace the `CommandCatalogArgument` struct body (lines 23-31) with:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CommandCatalogArgument {
    pub name: String,
    pub short: Option<char>,
    pub long: Option<String>,
    pub help: Option<String>,
    pub required: bool,
    pub takes_value: bool,
    /// Typed argument kind for GUI form generation (flag vs value vs count).
    pub value_kind: ArgValueKind,
    /// Enumerated accepted values (clap `value_enum` / possible values); empty when unconstrained.
    #[serde(default)]
    pub possible_values: Vec<String>,
    /// Default values clap applies when the argument is omitted; empty when none.
    #[serde(default)]
    pub default_values: Vec<String>,
}
```

- [ ] **Step 4: Enrich the extraction in `push_catalog_entry`**

In `crates/vox-cli/src/command_catalog.rs`, replace the `arguments: cmd.get_arguments().map(...).collect(),` block (currently lines 312-325) with:

```rust
        arguments: cmd
            .get_arguments()
            .map(|arg| {
                let action = arg.get_action();
                let value_kind = match action {
                    clap::ArgAction::SetTrue | clap::ArgAction::SetFalse => ArgValueKind::Flag,
                    clap::ArgAction::Count => ArgValueKind::Count,
                    _ => ArgValueKind::Value,
                };
                CommandCatalogArgument {
                    name: arg.get_id().to_string(),
                    short: arg.get_short(),
                    long: arg.get_long().map(|s| s.to_string()),
                    help: arg.get_help().map(|s| s.to_string()),
                    required: arg.is_required_set(),
                    takes_value: matches!(
                        action,
                        clap::ArgAction::Set | clap::ArgAction::Append
                    ),
                    value_kind,
                    possible_values: arg
                        .get_possible_values()
                        .iter()
                        .map(|pv| pv.get_name().to_string())
                        .collect(),
                    default_values: arg
                        .get_default_values()
                        .iter()
                        .map(|v| v.to_string_lossy().into_owned())
                        .collect(),
                }
            })
            .collect(),
```

- [ ] **Step 5: Fix the GUI builder's synthetic MCP-payload literal (compile fix)**

In `crates/vox-gui/src/commands/action_manifest.rs`, the synthetic payload argument is a struct literal (currently lines 280-287). Add the three new fields so it compiles. Replace that literal with:

```rust
            arguments: vec![vox_cli::command_catalog::CommandCatalogArgument {
                name: "payload".to_string(),
                short: None,
                long: Some("payload".to_string()),
                help: Some("JSON payload for MCP tool invocation".to_string()),
                required: false,
                takes_value: true,
                value_kind: vox_cli::command_catalog::ArgValueKind::Value,
                possible_values: Vec::new(),
                default_values: Vec::new(),
            }],
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p vox-cli catalog_arguments_carry_value_kind_and_possible_values`
Expected: PASS.

- [ ] **Step 7: Confirm the workspace still compiles (GUI builder fix)**

Run: `cargo check -p vox-gui`
Expected: compiles (no `missing fields ... value_kind` error). If `vox-gui` requires a feature/toolchain unavailable locally, run `cargo check -p vox-cli` at minimum and note the skip.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/command_catalog.rs crates/vox-gui/src/commands/action_manifest.rs
git commit -m "feat(vox-cli): typed argument kind + possible/default values in command catalog"
```

---

## Task 2: Enforce typed args in the manifest schema + regression test

**Files:**
- Modify: `contracts/gui/action-manifest.v1.schema.json:60-74` (arguments definition)
- Modify: `crates/vox-cli/src/commands/ci/gui_catalog_parity.rs:198-206` (synthetic payload JSON)
- Test: `crates/vox-cli/src/commands/ci/gui_catalog_parity.rs` (new tests module)

- [ ] **Step 1: Write the failing test**

Append to the end of `crates/vox-cli/src/commands/ci/gui_catalog_parity.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_manifest_validates_with_typed_args() {
        let repo_root = vox_repository::resolve_repo_root_for_ci();
        let manifest = generated_manifest_payload(&repo_root).expect("build manifest payload");

        // Schema validation (same schema the guard enforces).
        let schema_path = repo_root.join("contracts/gui/action-manifest.v1.schema.json");
        let schema_raw = std::fs::read_to_string(&schema_path).expect("read schema");
        let schema_val: serde_json::Value =
            serde_json::from_str(&schema_raw).expect("parse schema");
        let validator =
            vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
                .expect("compile schema");
        vox_jsonschema_util::validate(&manifest, &validator, "action manifest schema")
            .expect("generated manifest must validate against schema");

        // Every argument (CLI-derived and synthetic) must carry value_kind.
        let actions = manifest
            .get("actions")
            .and_then(|v| v.as_array())
            .expect("actions array");
        let mut saw_arg = false;
        for action in actions {
            if let Some(args) = action.get("arguments").and_then(|v| v.as_array()) {
                for arg in args {
                    assert!(
                        arg.get("value_kind").and_then(|v| v.as_str()).is_some(),
                        "argument missing value_kind in action {:?}",
                        action.get("id")
                    );
                    saw_arg = true;
                }
            }
        }
        assert!(saw_arg, "expected at least one argument in the manifest");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli generated_manifest_validates_with_typed_args`
Expected: FAIL — the synthetic MCP-payload argument (built as JSON in `generated_manifest_payload`) has no `value_kind`, so the `value_kind` assertion fails (and, once the schema requires it in Step 3, schema validation fails too).

- [ ] **Step 3: Add the new fields to the schema and require `value_kind`**

In `contracts/gui/action-manifest.v1.schema.json`, replace the `arguments` definition (currently lines 60-74) with:

```json
          "arguments": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["name", "required", "takes_value", "value_kind"],
              "properties": {
                "name": { "type": "string", "minLength": 1 },
                "short": { "type": ["string", "null"] },
                "long": { "type": ["string", "null"] },
                "help": { "type": ["string", "null"] },
                "required": { "type": "boolean" },
                "takes_value": { "type": "boolean" },
                "value_kind": { "enum": ["flag", "value", "count"] },
                "possible_values": {
                  "type": "array",
                  "items": { "type": "string" }
                },
                "default_values": {
                  "type": "array",
                  "items": { "type": "string" }
                }
              }
            }
          }
```

- [ ] **Step 4: Add the new fields to the synthetic payload JSON**

In `crates/vox-cli/src/commands/ci/gui_catalog_parity.rs`, replace the `"arguments": [{ ... }]` block in `generated_manifest_payload` (currently lines 198-206) with:

```rust
            "arguments": [{
                "name": "payload",
                "short": null,
                "long": "payload",
                "help": "JSON payload for MCP tool invocation",
                "required": false,
                "takes_value": true,
                "value_kind": "value",
                "possible_values": [],
                "default_values": []
            }],
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli generated_manifest_validates_with_typed_args`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add contracts/gui/action-manifest.v1.schema.json crates/vox-cli/src/commands/ci/gui_catalog_parity.rs
git commit -m "feat(gui-manifest): require typed value_kind + possible/default values in action-manifest schema"
```

---

## Task 3: Update the TypeScript mirrors

**Files:**
- Modify: `crates/vox-gui/ui/src/types/catalog.ts:3-10`
- Modify: `crates/vox-gui/ui/src/types/actionManifest.ts:8-15`

- [ ] **Step 1: Update `catalog.ts`**

In `crates/vox-gui/ui/src/types/catalog.ts`, replace the `CommandCatalogArgument` interface (lines 3-10) with:

```ts
export type ArgValueKind = 'flag' | 'value' | 'count';

export interface CommandCatalogArgument {
  name: string;
  short: string | null;
  long: string | null;
  help: string | null;
  required: boolean;
  takes_value: boolean;
  value_kind: ArgValueKind;
  possible_values?: string[];
  default_values?: string[];
}
```

- [ ] **Step 2: Update `actionManifest.ts`**

In `crates/vox-gui/ui/src/types/actionManifest.ts`, replace the `ActionArgument` interface (lines 8-15) with:

```ts
export type ArgValueKind = 'flag' | 'value' | 'count';

export interface ActionArgument {
  name: string;
  short: string | null;
  long: string | null;
  help: string | null;
  required: boolean;
  takes_value: boolean;
  value_kind: ArgValueKind;
  possible_values?: string[];
  default_values?: string[];
}
```

- [ ] **Step 3: Typecheck the GUI sources**

Run (from repo root): `npx --prefix crates/vox-gui/ui tsc --noEmit -p crates/vox-gui/ui/tsconfig.json`
Expected: no type errors. If the local TS toolchain is not installed, run `pnpm --dir crates/vox-gui/ui install` first; if TS tooling cannot run in this environment, note the skip and rely on the `gui-catalog-parity` guard plus manual review.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/types/catalog.ts crates/vox-gui/ui/src/types/actionManifest.ts
git commit -m "feat(gui): mirror typed argument fields in TypeScript catalog + manifest types"
```

---

## Final verification

- [ ] **Run the two new unit tests:**

Run: `cargo test -p vox-cli catalog_arguments_carry_value_kind_and_possible_values generated_manifest_validates_with_typed_args`
Expected: both PASS.

- [ ] **Run the full GUI parity guard:**

Run: `cargo run -p vox-cli -- ci gui-catalog-parity`
Expected: PASS.
Contingency: the audit (`vox-gui-capability-audit-2026.md`) noted `gui-version-sync` could fail on a stale `tauri.conf.json` version. That failure is **pre-existing and out of SP-1 scope**. If it occurs, the version drift is not introduced by this work — record it and (only if the user approves) run `cargo run -p vox-cli -- ci gui-version-sync --write` separately. Do not bundle a version bump into SP-1.

---

## Self-review notes

- **Spec coverage:** Implements Unit 1's typed-argument requirement (`args` with `kind`/enum values/defaults). The `surface` field and the `output.schema_ref`/`duration` overlay are intentionally **deferred** — `surface` is derivable from `cli_path[0]` and is consumed by SP-2 (decorator routing), so adding it now would be YAGNI; `output_kind` already exists at a coarse grain and a finer output contract is a later overlay. No new CI guard is added because `gui-catalog-parity` already regenerates + schema-validates the manifest.
- **Type consistency:** `ArgValueKind` serializes snake_case (`flag`/`value`/`count`), matching the schema enum and both TS unions. The struct field order in Task 1 matches the schema property set in Task 2 and the TS interfaces in Task 3.
- **Always-green commits:** Task 1 fixes the only compile-breaking call site (the `vox-gui` struct literal); the `gui_catalog_parity.rs` JSON payload compiles without the new fields and is updated in Task 2 alongside making the schema require them (clean red→green).
