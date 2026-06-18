# Track A — Naked-Objects Auto-GUI Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` to execute task-by-task, and `crates/vox-skills/skills/superpowers/test-driven-development.skill.md` for each task. Steps use checkbox (`- [ ]`) syntax.

> **🤖 EXECUTION TARGET — READ FIRST.** This plan is written to be run end-to-end by **Gemini 3.5 Flash inside Google Antigravity**. Antigravity is unreliable on long tasks (≈48% real-world completion; mid-task termination leaves no checkpoint; quota is a hard cutoff) and Gemini 3.5 Flash hallucinates APIs and has weak long-context recall. The plan is therefore engineered against those failure modes. **You MUST obey the Operating Rules below on every task.** Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff guide (native skills, parallel dispatch): [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** Finish a task only when its tests pass AND you commit. A crash between tasks must leave a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use (anti-hallucination).** Before any code step that references a symbol/type/path, run the `rg`/read step in that task and confirm it exists. If reality differs from the plan, STOP and report — do not invent.
3. **Self-contained.** Everything you need is in the task. Do not rely on remembering earlier tasks.
4. **Two-strike circuit breaker.** If a step's verification fails twice, STOP, write a one-paragraph handoff note (what failed, last good commit), and hand back. Do not loop.
5. **Parallel dispatch.** Tasks are tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`. Only dispatch parallel subagents for `[PARALLEL-SAFE]` tasks whose **Files** sets are disjoint. Never let two subagents write the same file. See handoff §3/§5.2.
6. **Vox house rules.** Never `cargo fmt --all` (use `cargo fmt -p <crate>`). Automation is `.vox`, not `.ps1/.sh/.py`. `.md` under `docs/src/` needs YAML frontmatter.
7. **Verification ritual** before each commit (use the `verification-before-completion` skill — `crates/vox-skills/skills/superpowers/verification-before-completion.skill.md`): `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` (TOESTUB/no-stub compliance) → `cargo fmt -p <crate>`, pasting real output. Self-review with the `requesting-code-review` skill before committing.
8. **Rollback on broken tree.** If a task aborts mid-edit (Antigravity termination) leaving a non-compiling tree, `git reset --hard HEAD` to the last green commit, then re-attempt that single task from scratch. Never build forward on a broken tree.
9. **Skill references.** Design choices → `brainstorming` skill; parallel waves → `dispatching-parallel-agents` skill; isolation → `using-git-worktrees` skill. All under `crates/vox-skills/skills/superpowers/` (see handoff §4).
10. **Rust implementation constraints** — obey design §5b: `vox_compiler::ast::span::Span` in fixtures (not `vox_ast`); no `set_var`/global state in tests (inject params); no `.unwrap()` in library code; deterministic output; `cargo run -p vox-arch-check` must pass.

**Goal:** Extend Vox's existing type→form codegen with richer typed field inference (enums, branded scalars) and an **opt-in** naked-objects generator that turns a `@table` **listed in an opt-in registry** into list/detail/edit React views — zero hand-written UI, and **never generated from persistence alone**.

**Architecture:** Pure additive string-emission codegen in `vox-codegen-ts`, mirroring [`form_emit.rs`](../../../crates/vox-codegen-ts/src/form_emit.rs). Admin generation is a pure function `emit_admin(&HirTable) -> String`, unit-testable in isolation, wired in only for tables named in `contracts/gui/admin-registry.yaml` (opt-in; no grammar change) and behind the `VOX_EMIT_ADMIN` global switch.

**Tech Stack:** Rust; `vox-codegen-ts`; HIR types from `vox-compiler` (`HirType`, `HirForm`, `HirFormField`, `HirFieldConstraint`, `HirTable`, `HirTableField`); `vox-ast::Span`.

**Design:** [`../../src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md`](../../src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md) §2 (opt-in correction in §2.2).

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-codegen-ts/src/form_emit.rs` | `@form`→React; field-type inference | Modify (Tasks 1–2) |
| `crates/vox-codegen-ts/src/admin_emit.rs` | `HirTable`→admin list/detail/edit | Create (Tasks 3–5) |
| module-decl file (`lib.rs`/`emitter.rs`) | register `admin_emit` | Modify (Task 3) |
| `contracts/gui/admin-registry.yaml` | opt-in allowlist of table names | Create (Task 6) |
| `crates/vox-codegen-ts/src/emitter.rs:301` | wire admin output (registry+flag gated) | Modify (Task 6) |
| `docs/src/architecture/where-things-live.md` | register module | Modify (Task 7) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "mod form_emit|pub mod form_emit" crates/vox-codegen-ts/src/` — note the exact module-decl file + visibility.
- `rg -n "impl DefId|pub fn from_raw|pub fn new" crates/vox-compiler/src/hir/def_map.rs` — note the real `DefId` constructor.
- `cargo run -p vox-arch-check` — baseline must pass.

---

## Task 1 `[SEQUENTIAL]`: Branded-scalar field-type inference

Maps branded scalar names to precise HTML input types. `HirType::Named("email")` is how a branded scalar arrives (no new HIR variant).

**Files:**
- Modify: `crates/vox-codegen-ts/src/form_emit.rs:187-194`
- Test: `crates/vox-codegen-ts/src/form_emit.rs` (new `#[cfg(test)]` module at end)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "fn hir_type_to_input_type" crates/vox-codegen-ts/src/form_emit.rs`. Confirm the function exists around line 187 and matches `int/float/decimal→number, bool→checkbox, timestamp→datetime-local, _→text`. If not, STOP and report.

- [ ] **Step 2: Write the failing test.** Append to `form_emit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span; // re-exported path used across vox-codegen-ts; NOT vox_ast (design §5b.1)
    use vox_compiler::hir::HirType;
    use vox_compiler::hir::nodes::form::{HirForm, HirFormField};

    fn field(name: &str, ty: HirType) -> HirFormField {
        HirFormField { name: name.into(), ty, label: None, required: false,
            hidden: false, default: None, constraints: vec![], span: Span::new(0, 0) }
    }
    fn form_with(fields: Vec<HirFormField>) -> HirForm {
        HirForm { name: "T".into(), fields, on_submit: None, success_redirect: None,
            error_message: None, span: Span::new(0, 0) }
    }

    #[test]
    fn branded_scalars_render_typed_inputs() {
        let out = emit_form(&form_with(vec![
            field("e", HirType::Named("email".into())),
            field("u", HirType::Named("url".into())),
            field("p", HirType::Named("phone".into())),
        ]));
        assert!(out.contains("type=\"email\""), "email:\n{out}");
        assert!(out.contains("type=\"url\""), "url:\n{out}");
        assert!(out.contains("type=\"tel\""), "tel:\n{out}");
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-codegen-ts branded_scalars_render_typed_inputs` → FAIL (email/url/phone fall through to `text`).

- [ ] **Step 4: Implement.** Replace `hir_type_to_input_type` body (`form_emit.rs:187-194`):

```rust
fn hir_type_to_input_type(ty: &HirType) -> &'static str {
    match ty {
        HirType::Named(t) if t == "int" || t == "float" || t == "decimal" => "number",
        HirType::Named(t) if t == "bool" => "checkbox",
        HirType::Named(t) if t == "timestamp" => "datetime-local",
        HirType::Named(t) if t == "email" => "email",
        HirType::Named(t) if t == "url" => "url",
        HirType::Named(t) if t == "phone" => "tel",
        _ => "text",
    }
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-codegen-ts branded_scalars_render_typed_inputs` → PASS.

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-codegen-ts -- -D warnings`; `cargo fmt -p vox-codegen-ts`; then:

```bash
git add crates/vox-codegen-ts/src/form_emit.rs
git commit -m "feat(codegen-ts): typed inputs for branded scalars (email/url/phone)"
```

---

## Task 2 `[SEQUENTIAL]` (same file as Task 1): Enum fields render as `<select>`

`HirFieldConstraint::Enum(Vec<HirExpr>)` already exists. Render a `<select>` of string-literal variants.

**Files:** Modify `crates/vox-codegen-ts/src/form_emit.rs` (render loop ~142-181 + helper); Test: same `tests` module.

- [ ] **Step 1 (verify-before-use):** Run `rg -n "enum HirFieldConstraint" crates/vox-compiler/src/hir/nodes/form.rs`. Confirm a variant `Enum(Vec<HirExpr>)` exists. If not, STOP.

- [ ] **Step 2: Failing test.** Add to `tests`:

```rust
#[test]
fn enum_constraint_renders_select() {
    use vox_compiler::hir::nodes::form::HirFieldConstraint;
    use vox_compiler::hir::nodes::expr::HirExpr;
    let mut f = field("role", HirType::Named("Role".into()));
    f.constraints = vec![HirFieldConstraint::Enum(vec![
        HirExpr::StringLit("admin".into(), Span::new(0, 0)),
        HirExpr::StringLit("user".into(), Span::new(0, 0)),
    ])];
    let out = emit_form(&form_with(vec![f]));
    assert!(out.contains("<select"), "select:\n{out}");
    assert!(out.contains(">admin<"), "admin opt:\n{out}");
    assert!(out.contains(">user<"), "user opt:\n{out}");
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-codegen-ts enum_constraint_renders_select`.

- [ ] **Step 4: Implement.** Add helper near `hir_type_to_input_type`:

```rust
use vox_compiler::hir::nodes::expr::HirExpr;

fn enum_variants(f: &HirFormField) -> Option<Vec<String>> {
    f.constraints.iter().find_map(|c| match c {
        HirFieldConstraint::Enum(exprs) => Some(
            exprs.iter().filter_map(|e| match e {
                HirExpr::StringLit(s, _) => Some(s.clone()),
                _ => None,
            }).collect()),
        _ => None,
    })
}
```

In the render loop (`for f in &visible {` ~line 142), the loop already declares `let label = …;` and `let req_marker = …;` once at its top. Insert the select branch **immediately after those two existing lines and before `let input_type = …;`** — it **reuses** `label`/`req_marker` (do NOT redeclare them) and ends with `continue;` so the `<input>` path below is skipped for enum fields. There is nothing to delete; the existing single declarations serve both branches:

```rust
        if let Some(variants) = enum_variants(f) {
            let options: String = variants.iter()
                .map(|v| format!("<option value=\"{v}\">{v}</option>")).collect();
            out.push_str(&format!(
                "      <label className=\"vox-form-field\">\n\
                 \x20       <span>{label}{req_marker}</span>\n\
                 \x20       <select value={{{fname} ?? \"\"}} onChange={{e => set_{fname}(e.target.value)}} aria-invalid={{!!errors.{fname}}}>\n\
                 \x20         <option value=\"\"></option>{options}\n\
                 \x20       </select>\n\
                 \x20       {{errors.{fname} && <span id=\"{fname}-error\" role=\"alert\" className=\"vox-form-error\">{{errors.{fname}}}</span>}}\n\
                 \x20     </label>\n",
                fname = f.name));
            continue;
        }
```

- [ ] **Step 5: Run → PASS, then full suite.** `cargo test -p vox-codegen-ts` (all green, incl. existing).

- [ ] **Step 6: Verify + commit.** clippy/fmt as Rule 7, then:

```bash
git add crates/vox-codegen-ts/src/form_emit.rs
git commit -m "feat(codegen-ts): render enum-constrained fields as <select>"
```

---

## Task 3 `[SEQUENTIAL]`: Admin list view + module registration

New module. `emit_admin_list(&HirTable) -> String` = read-only `<table>`.

**Files:** Create `crates/vox-codegen-ts/src/admin_emit.rs`; Modify module-decl file (Pre-flight) to add `mod admin_emit;`.

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub struct HirTable|pub struct HirTableField" crates/vox-compiler/src/hir/nodes/decl.rs`. Confirm `HirTable { id, name, fields: Vec<HirTableField>, primary_key, is_extern, source, is_pub, is_deprecated, span }` and `HirTableField { name, type_ann, span }`. Note the real `DefId` constructor from Pre-flight.

- [ ] **Step 2: Create the file with a failing test:**

```rust
//! Naked-objects admin codegen: HirTable → React list/detail/edit views.
//! Opt-in only (see admin-registry.yaml). Design §2.2.
use vox_compiler::hir::nodes::decl::HirTable;

pub fn emit_admin_list(table: &HirTable) -> String { let _ = table; String::new() }

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span; // re-exported path used across vox-codegen-ts; NOT vox_ast (design §5b.1)
    use vox_compiler::hir::HirType;
    use vox_compiler::hir::nodes::decl::{HirTable, HirTableField};
    use vox_compiler::hir::def_map::DefId;

    fn table() -> HirTable {
        HirTable {
            id: DefId::from_raw(0), // ← replace with the real constructor from Step 1 if different
            name: "User".into(),
            fields: vec![
                HirTableField { name: "name".into(), type_ann: HirType::Named("string".into()), span: Span::new(0,0) },
                HirTableField { name: "email".into(), type_ann: HirType::Named("email".into()), span: Span::new(0,0) },
            ],
            primary_key: None, is_extern: false, source: None,
            is_pub: true, is_deprecated: false, span: Span::new(0,0),
        }
    }
    #[test]
    fn list_view_has_component_and_columns() {
        let out = emit_admin_list(&table());
        assert!(out.contains("export function UserList()"), "name:\n{out}");
        assert!(out.contains(">name<"), "name col:\n{out}");
        assert!(out.contains(">email<"), "email col:\n{out}");
        assert!(out.contains("<table"), "table:\n{out}");
    }
}
```

Add `mod admin_emit;` to the module-decl file (from Pre-flight).

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-codegen-ts list_view_has_component_and_columns`.

- [ ] **Step 4: Implement.** Replace `emit_admin_list`:

```rust
pub fn emit_admin_list(table: &HirTable) -> String {
    let name = &table.name;
    let headers: String = table.fields.iter().map(|f| format!("<th>{}</th>", f.name)).collect();
    let cells: String = table.fields.iter()
        .map(|f| format!("<td>{{String(row.{} ?? \"\")}}</td>", f.name)).collect();
    format!(
        "export function {name}List() {{\n\
         \x20 const rows = useQuery(api.{nl}.list) ?? [];\n\
         \x20 return (<table className=\"vox-admin-list\">\n\
         \x20   <thead><tr>{headers}</tr></thead>\n\
         \x20   <tbody>{{rows.map((row: any) => (<tr key={{row._id}}>{cells}</tr>))}}</tbody>\n\
         \x20 </table>);\n}}\n",
        name = name, nl = name.to_lowercase())
}
```

- [ ] **Step 5: Run → PASS.** then Rule 7 verify.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-codegen-ts/src/admin_emit.rs crates/vox-codegen-ts/src/lib.rs
git commit -m "feat(codegen-ts): emit_admin_list — naked-objects list view from HirTable"
```

---

## Task 4 `[SEQUENTIAL]` (same new file): Admin edit form reusing form_emit

`emit_admin_edit` builds a `HirForm` from table fields and delegates to `emit_form` (DRY → inherits Task 1/2 typed inputs).

**Files:** Modify `crates/vox-codegen-ts/src/admin_emit.rs`.

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub fn emit_form|mod form_emit" crates/vox-codegen-ts/src/`. Confirm `emit_form` is `pub` and the module path to use (`crate::form_emit::emit_form`). Confirm `HirForm`/`HirFormField` field names via `rg -n "pub struct HirForm" -A14 crates/vox-compiler/src/hir/nodes/form.rs`.

- [ ] **Step 2: Failing test.** Add to `admin_emit.rs` tests:

```rust
#[test]
fn edit_form_reuses_form_emit_and_typed_inputs() {
    let out = emit_admin_edit(&table());
    assert!(out.contains("export function UserEdit()"), "name:\n{out}");
    assert!(out.contains("type=\"email\""), "typed email via form_emit:\n{out}");
}
```

- [ ] **Step 3: Run → FAIL** (function missing). `cargo test -p vox-codegen-ts edit_form_reuses_form_emit_and_typed_inputs`.

- [ ] **Step 4: Implement.**

```rust
use vox_compiler::hir::nodes::form::{HirForm, HirFormField};

pub fn emit_admin_edit(table: &HirTable) -> String {
    let fields: Vec<HirFormField> = table.fields.iter().map(|f| HirFormField {
        name: f.name.clone(), ty: f.type_ann.clone(), label: None, required: false,
        hidden: false, default: None, constraints: vec![], span: f.span,
    }).collect();
    let form = HirForm {
        name: format!("{}Edit", table.name), fields,
        on_submit: Some(format!("api.{}.upsert", table.name.to_lowercase())),
        success_redirect: None, error_message: None, span: table.span,
    };
    crate::form_emit::emit_form(&form)
}
```

- [ ] **Step 5: Run → PASS** (email input proves DRY reuse), then Rule 7.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-codegen-ts/src/admin_emit.rs
git commit -m "feat(codegen-ts): emit_admin_edit reuses form_emit for table edit forms"
```

---

## Task 5 `[SEQUENTIAL]` (same new file): `emit_admin` entry point

**Files:** Modify `crates/vox-codegen-ts/src/admin_emit.rs`.

- [ ] **Step 1: Failing test.**

```rust
#[test]
fn emit_admin_composes_list_and_edit() {
    let out = emit_admin(&table());
    assert!(out.contains("export function UserList()"), "list:\n{out}");
    assert!(out.contains("export function UserEdit()"), "edit:\n{out}");
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-codegen-ts emit_admin_composes_list_and_edit`.

- [ ] **Step 3: Implement.**

```rust
pub fn emit_admin(table: &HirTable) -> String {
    let mut out = emit_admin_list(table);
    out.push('\n');
    out.push_str(&emit_admin_edit(table));
    out
}
```

- [ ] **Step 4: Run → PASS**, Rule 7, commit.

```bash
git add crates/vox-codegen-ts/src/admin_emit.rs
git commit -m "feat(codegen-ts): emit_admin entry point composing list + edit views"
```

---

## Task 6 `[SEQUENTIAL]`: Opt-in registry + emitter wiring

**Opt-in correction:** admin UI is emitted ONLY for tables named in `contracts/gui/admin-registry.yaml` AND only when `VOX_EMIT_ADMIN=1`. Persistence alone never generates UI (design §2.2; [design hygiene §1](../../src/architecture/auto-derivation-design-hygiene-2026-06-18.md)).

**Files:** Create `contracts/gui/admin-registry.yaml`; Modify `crates/vox-codegen-ts/src/emitter.rs:301`.

- [ ] **Step 1 (verify-before-use):** Run `rg -n "forms_content|hir.forms|fn emit" crates/vox-codegen-ts/src/emitter.rs | head`. Note where `forms_content` is built (~301) and where it is concatenated into the output, and how `emit`/the emit fn is invoked in existing tests (`rg -n "#\[test\]|fn emit" crates/vox-codegen-ts/src/emitter.rs`).

- [ ] **Step 2: Create the registry file** `contracts/gui/admin-registry.yaml`:

```yaml
# Opt-in allowlist: tables that should get an auto-generated admin UI.
# A @table NOT listed here gets NO UI (persistence != UI intent).
admin_tables: []
```

- [ ] **Step 3: Failing test — PURE helper, NO env mutation.** Test a pure function `admin_content_for(tables, enabled, allow)` that takes the flag and allowlist as *parameters* (env is read only by the public emitter, never in tests — avoids flaky/parallel-unsafe `set_var`). Construct `HirTable`s directly (reuse the `table()` fixture shape from Task 3, inlined here so this task is self-contained):

```rust
#[cfg(test)]
mod admin_wiring_tests {
    use super::*;
    use vox_compiler::ast::span::Span; // re-exported path used across vox-codegen-ts; NOT vox_ast (design §5b.1)
    use vox_compiler::hir::HirType;
    use vox_compiler::hir::nodes::decl::{HirTable, HirTableField};
    use vox_compiler::hir::def_map::DefId;

    fn user_table() -> HirTable {
        HirTable { id: DefId::from_raw(0), name: "User".into(),
            fields: vec![HirTableField { name: "name".into(), type_ann: HirType::Named("string".into()), span: Span::new(0,0) }],
            primary_key: None, is_extern: false, source: None, is_pub: true, is_deprecated: false, span: Span::new(0,0) }
    }

    #[test]
    fn admin_content_respects_flag_and_allowlist() {
        let tables = vec![user_table()];
        let allow = vec!["User".to_string()];
        assert!(admin_content_for(&tables, false, &allow).is_empty(), "off → nothing");
        assert!(admin_content_for(&tables, true, &[]).is_empty(), "on but not in registry → nothing");
        assert!(admin_content_for(&tables, true, &allow).contains("export function UserList()"), "on + allowed → admin");
    }
}
```

> `DefId::from_raw(0)` — replace with the real constructor from Pre-flight if different.

- [ ] **Step 4: Run → FAIL.** `cargo test -p vox-codegen-ts admin_content_respects_flag_and_allowlist` (function missing → compile error).

- [ ] **Step 5: Implement.** Add the pure helper to `emitter.rs`:

```rust
/// Pure: emit admin surfaces for opted-in tables. Flag + allowlist are injected
/// (env/registry are read only by the public emit fn) so this is deterministically testable.
fn admin_content_for(tables: &[vox_compiler::hir::nodes::decl::HirTable], enabled: bool, allow: &[String]) -> String {
    if !enabled { return String::new(); }
    tables.iter()
        .filter(|t| allow.iter().any(|n| n == &t.name))
        .map(super::admin_emit::emit_admin)
        .collect()
}

fn load_admin_registry() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = std::env::var("VOX_ADMIN_REGISTRY")
        .unwrap_or_else(|_| "contracts/gui/admin-registry.yaml".to_string());
    let src = std::fs::read_to_string(&path)?;
    let cfg: AdminRegistry = serde_yaml::from_str(&src)?;
    Ok(cfg.allow)
}
```

At `emitter.rs:301`, after `forms_content`, call the helper and concatenate at the output-assembly site found in Step 1:

```rust
    let admin_enabled = std::env::var("VOX_EMIT_ADMIN").as_deref() == Ok("1");
    let registry = load_admin_registry().unwrap_or_else(|e| {
        tracing::warn!("admin registry unavailable, defaulting to empty list: {e}");
        vec![]
    });
    let admin_content = admin_content_for(&hir.tables, admin_enabled, &registry);
```

> Implement `load_admin_registry` per Step 1 (real YAML read). The pure `admin_content_for` is what the test pins; the public path just feeds it env + registry.

- [ ] **Step 6: Run → PASS, full suite, Rule 7, arch-check.** `cargo test -p vox-codegen-ts`; `cargo run -p vox-arch-check`; clippy/fmt.

- [ ] **Step 7: Commit.**

```bash
git add crates/vox-codegen-ts/src/emitter.rs contracts/gui/admin-registry.yaml
git commit -m "feat(codegen-ts): opt-in admin UI via registry + VOX_EMIT_ADMIN gate"
```

---

## Task 7 `[PARALLEL-SAFE]` (docs only): where-things-live row

**Files:** Modify `docs/src/architecture/where-things-live.md`. (Disjoint from all code tasks → can run in parallel with any remaining doc task.)

- [ ] **Step 1:** Open the file, match its exact table columns, add:

```markdown
| Naked-objects admin UI codegen (opt-in table → list/detail/edit React) | `crates/vox-codegen-ts/src/admin_emit.rs` |
```

- [ ] **Step 2: Commit.**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register admin_emit in where-things-live"
```

---

## Parallelization summary (for the Antigravity orchestrator)

- **Tasks 1→2→3→4→5→6 are a strict SEQUENTIAL chain** (1–2 share `form_emit.rs`; 3–5 share `admin_emit.rs`; 6 depends on 3–5). Run on ONE agent in order.
- **Task 7 is PARALLEL-SAFE** (docs only) — dispatch any time after Task 5.
- Net: this plan is mostly sequential by nature (shared files). Do not force-parallelize 1–6; you will clobber. See handoff §3.

---

## Self-Review

- **Spec coverage:** field inference (T1–2), opt-in naked-objects list+edit (T3–6, opt-in via registry per design §2.2), docs (T7). Detail view + CRUD endpoint codegen + `@admin` grammar = **Deferred** (below).
- **Deferred (YAGNI):** `emit_admin_detail`; real CRUD endpoint codegen (T4 references `api.<t>.upsert`/`.list` — assumed to exist until a separate endpoint plan); `@admin` annotation grammar (registry is the v1 opt-in mechanism, no grammar change); nested struct→fieldset, `list<T>`→repeating block.
- **Placeholder scan:** T6 has one verify-then-fill point (`load_admin_registry`'s real YAML read) with the exact `rg` + explicit fallback; the env-mutating test was removed in favor of a pure `admin_content_for` helper (no flaky `set_var`). `DefId::from_raw` is the only fixture value to confirm against Pre-flight. All other code is complete.
- **Type consistency:** `emit_admin_list/_edit/admin` consistent T3–6; `HirTableField.type_ann` and `HirFormField.ty` verified against `decl.rs`/`form.rs`; `HirFieldConstraint::Enum` verified.
- **Antigravity fit:** every task atomic+committed; verify-before-use steps present; circuit-breaker rule stated; parallel tags applied.

## Execution Handoff

Track A only; independent of Track B. Lower-risk — recommended to execute first. Use the in-repo skills in the header; for missing skills (brainstorming/parallel-dispatch/verification/code-review/worktrees) see [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md) §5.
