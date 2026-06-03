# GUI Track A — Self-Surfacing Surface Registry + CI Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one typed surface-registry the single source of truth that (a) drives the GUI sidebar nav and (b) is enforced by a CI gate that *fails the build* when a new top-level CLI group is unclassified — so new front-end needs surface automatically instead of silently.

**Architecture:** A new contract `contracts/gui/surface-registry.v1.yaml` (+ JSON schema) lists every GUI surface and every top-level CLI group with a `representation_tier`. A new `vox ci gui-surface-registry` check enumerates top-level clap groups via the existing `command_catalog::build_catalog()`, backfills unclassified groups as `none` in `--write` mode, and in verify mode fails on any missing group or mis-wired curated surface. The same check generates a typed `surfaceRegistry.generated.ts`; `Sidebar.tsx` renders nav from it; a new `Coverage` GUI surface renders the registry for in-product discoverability.

**Tech Stack:** Rust (clap, serde_yaml, serde_json, regex, anyhow — all already workspace deps), React 18 + TypeScript + Vite + Tailwind (`crates/vox-gui/ui`), the existing `vox ci` check harness.

---

## File Structure

- Create `contracts/gui/surface-registry.v1.yaml` — the SSOT (hand-authored curated entries; `none` entries backfilled by the tool).
- Create `contracts/gui/surface-registry.v1.schema.json` — JSON schema for the SSOT.
- Create `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` — loader, enumerator, backfill, enforcement, TS generator, report writer (mirrors `gui_surface_coverage.rs`).
- Modify `crates/vox-cli/src/commands/ci/mod.rs` — declare the new module.
- Modify `crates/vox-cli/src/commands/ci/cmd_enums.rs` — add the `GuiSurfaceRegistry { write }` variant.
- Modify `crates/vox-cli/src/commands/ci/run_body.rs` — dispatch the new variant.
- Modify `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` — add the check to `run_ssot_drift` (verify mode).
- Create `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` — tool-generated; never hand-edited.
- Modify `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` — render nav from the generated registry.
- Create `crates/vox-gui/ui/src/components/surfaces/Coverage/CoverageView.tsx` — the in-GUI coverage surface.
- Modify `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts` — register `coverage`.
- Modify `crates/vox-gui/ui/src/App.tsx` — add `coverage` to the `View` union and the validation array.

---

## Task 1: Author the surface-registry contract + schema

**Files:**
- Create: `contracts/gui/surface-registry.v1.yaml`
- Create: `contracts/gui/surface-registry.v1.schema.json`

- [ ] **Step 1: Write the JSON schema**

Create `contracts/gui/surface-registry.v1.schema.json`:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Vox GUI Surface Registry v1",
  "type": "object",
  "required": ["x_vox_version", "schema_version", "surfaces"],
  "additionalProperties": false,
  "properties": {
    "x_vox_version": { "type": "integer" },
    "schema_version": { "const": 1 },
    "surfaces": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["representation_tier"],
        "additionalProperties": false,
        "properties": {
          "view_key": { "type": ["string", "null"] },
          "cli_group": { "type": ["string", "null"] },
          "representation_tier": {
            "type": "string",
            "enum": ["none", "generic_form", "curated_decorator", "live_backend"]
          },
          "nav_label": { "type": ["string", "null"] },
          "nav_icon": { "type": ["string", "null"] },
          "nav_group": { "type": ["string", "null"] },
          "notes": { "type": ["string", "null"] }
        }
      }
    }
  }
}
```

- [ ] **Step 2: Write the seed registry (curated surfaces only; `none` groups are backfilled by the tool in Task 4)**

Create `contracts/gui/surface-registry.v1.yaml`. Author one entry per existing GUI view (mirror `Sidebar.tsx:88-108` + `App.tsx` `View` union), wiring each to its `cli_group` where one exists. Non-CLI chrome views (`dashboard`, `flow`, `catalog`, `matrix`) carry `cli_group: null`.

```yaml
x_vox_version: 2
schema_version: 1
surfaces:
  # ── Operator chrome (no single CLI group) ──────────────────────────────
  - { view_key: dashboard, cli_group: null, representation_tier: live_backend, nav_label: Dashboard, nav_icon: dashboard, nav_group: operate, notes: orchestrator status }
  - { view_key: flow, cli_group: null, representation_tier: live_backend, nav_label: Agents, nav_icon: flow, nav_group: operate, notes: agent flow graph }
  - { view_key: catalog, cli_group: commands, representation_tier: live_backend, nav_label: Commands, nav_icon: catalog, nav_group: operate, notes: generated command form browser }
  - { view_key: matrix, cli_group: null, representation_tier: live_backend, nav_label: Policies, nav_icon: matrix, nav_group: operate, notes: policy/intention matrix }
  # ── Research / Scientia ────────────────────────────────────────────────
  - { view_key: scientia, cli_group: scientia, representation_tier: curated_decorator, nav_label: Scientia, nav_icon: file, nav_group: research, notes: Phase H queue dashboard }
  - { view_key: claims, cli_group: null, representation_tier: curated_decorator, nav_label: Claims, nav_icon: matrix, nav_group: research, notes: claim ledger (scientia claims) }
  - { view_key: research, cli_group: research, representation_tier: curated_decorator, nav_label: Research, nav_icon: memory, nav_group: research, notes: deep-research surface }
  # ── AI / ML ────────────────────────────────────────────────────────────
  - { view_key: mens, cli_group: mens, representation_tier: curated_decorator, nav_label: Mens, nav_icon: cpu, nav_group: ai }
  - { view_key: populi, cli_group: populi, representation_tier: curated_decorator, nav_label: Populi, nav_icon: flow, nav_group: ai }
  - { view_key: oratio, cli_group: oratio, representation_tier: curated_decorator, nav_label: Oratio, nav_icon: spark, nav_group: ai }
  - { view_key: models, cli_group: model, representation_tier: live_backend, nav_label: Models, nav_icon: cpu, nav_group: ai }
  # ── Build / run ────────────────────────────────────────────────────────
  - { view_key: harness, cli_group: null, representation_tier: live_backend, nav_label: Harness, nav_icon: command, nav_group: build }
  - { view_key: repository, cli_group: repo, representation_tier: live_backend, nav_label: Repository, nav_icon: file, nav_group: build }
  - { view_key: mesh, cli_group: null, representation_tier: live_backend, nav_label: Mesh, nav_icon: cpu, nav_group: build }
  # ── Operate cont. ──────────────────────────────────────────────────────
  - { view_key: gamify, cli_group: ludus, representation_tier: curated_decorator, nav_label: Gamify, nav_icon: spark, nav_group: operate }
  - { view_key: runs, cli_group: null, representation_tier: live_backend, nav_label: Runs, nav_icon: scale, nav_group: operate }
  - { view_key: approvals, cli_group: null, representation_tier: live_backend, nav_label: Approvals, nav_icon: shield, nav_group: operate }
  - { view_key: skills, cli_group: skill, representation_tier: live_backend, nav_label: Skills, nav_icon: catalog, nav_group: build }
  - { view_key: memory, cli_group: memory, representation_tier: live_backend, nav_label: Memory, nav_icon: memory, nav_group: operate }
  - { view_key: settings, cli_group: config, representation_tier: live_backend, nav_label: Settings, nav_icon: settings, nav_group: system }
```

- [ ] **Step 3: Commit**

```bash
git add contracts/gui/surface-registry.v1.yaml contracts/gui/surface-registry.v1.schema.json
git commit -m "feat(gui): add surface-registry SSOT contract + schema"
```

---

## Task 2: The `gui-surface-registry` CI check (pure logic + I/O, TDD)

**Files:**
- Create: `crates/vox-cli/src/commands/ci/gui_surface_registry.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs`

- [ ] **Step 1: Write the failing unit tests**

Create `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` with the types and a `#[cfg(test)]` module first:

```rust
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const REGISTRY_PATH: &str = "contracts/gui/surface-registry.v1.yaml";
const SCHEMA_PATH: &str = "contracts/gui/surface-registry.v1.schema.json";
const REPORT_PATH: &str = "contracts/reports/gui-surface-registry.v1.json";
const GENERATED_TS: &str = "crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts";
const GUI_APP: &str = "crates/vox-gui/ui/src/App.tsx";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationTier {
    None,
    GenericForm,
    CuratedDecorator,
    LiveBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SurfaceEntry {
    #[serde(default)]
    pub view_key: Option<String>,
    #[serde(default)]
    pub cli_group: Option<String>,
    pub representation_tier: RepresentationTier,
    #[serde(default)]
    pub nav_label: Option<String>,
    #[serde(default)]
    pub nav_icon: Option<String>,
    #[serde(default)]
    pub nav_group: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceRegistry {
    pub x_vox_version: u32,
    pub schema_version: u8,
    pub surfaces: Vec<SurfaceEntry>,
}

/// Groups already classified by some entry's `cli_group`.
fn covered_groups(reg: &SurfaceRegistry) -> BTreeSet<String> {
    reg.surfaces
        .iter()
        .filter_map(|e| e.cli_group.clone())
        .collect()
}

/// Top-level clap groups not yet covered by any entry.
pub fn missing_groups(reg: &SurfaceRegistry, top_level: &BTreeSet<String>) -> Vec<String> {
    let covered = covered_groups(reg);
    top_level.difference(&covered).cloned().collect()
}

/// Return a registry with `none`-tier entries appended for every missing group,
/// surfaces sorted by (cli_group, view_key) for deterministic output.
pub fn backfill(mut reg: SurfaceRegistry, missing: &[String]) -> SurfaceRegistry {
    for g in missing {
        reg.surfaces.push(SurfaceEntry {
            view_key: None,
            cli_group: Some(g.clone()),
            representation_tier: RepresentationTier::None,
            nav_label: None,
            nav_icon: None,
            nav_group: None,
            notes: None,
        });
    }
    reg.surfaces.sort_by(|a, b| {
        let ka = (a.cli_group.clone().unwrap_or_default(), a.view_key.clone().unwrap_or_default());
        let kb = (b.cli_group.clone().unwrap_or_default(), b.view_key.clone().unwrap_or_default());
        ka.cmp(&kb)
    });
    reg
}

/// Curated/live entries whose `view_key` is absent or not present in App.tsx.
pub fn wiring_violations(reg: &SurfaceRegistry, app_src: &str) -> Vec<String> {
    let mut v = Vec::new();
    for e in &reg.surfaces {
        if matches!(
            e.representation_tier,
            RepresentationTier::CuratedDecorator | RepresentationTier::LiveBackend
        ) {
            match &e.view_key {
                None => v.push(format!(
                    "{:?} surface for cli_group={:?} has no view_key",
                    e.representation_tier, e.cli_group
                )),
                Some(vk) if !app_src.contains(&format!("'{vk}'")) => {
                    v.push(format!("view_key '{vk}' is not referenced in App.tsx"));
                }
                Some(_) => {}
            }
        }
    }
    v
}

/// Deterministic TypeScript projection of the registry.
pub fn generate_ts(reg: &SurfaceRegistry) -> String {
    let opt = |o: &Option<String>| match o {
        Some(s) => format!("'{}'", s.replace('\'', "\\'")),
        None => "null".to_string(),
    };
    let tier = |t: &RepresentationTier| match t {
        RepresentationTier::None => "none",
        RepresentationTier::GenericForm => "generic_form",
        RepresentationTier::CuratedDecorator => "curated_decorator",
        RepresentationTier::LiveBackend => "live_backend",
    };
    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by `vox ci gui-surface-registry --write`. DO NOT EDIT.\n");
    out.push_str("export type RepresentationTier = 'none' | 'generic_form' | 'curated_decorator' | 'live_backend';\n");
    out.push_str("export interface SurfaceRegistryEntry {\n");
    out.push_str("  viewKey: string | null;\n  cliGroup: string | null;\n  tier: RepresentationTier;\n");
    out.push_str("  navLabel: string | null;\n  navIcon: string | null;\n  navGroup: string | null;\n}\n");
    out.push_str("export const SURFACE_REGISTRY: SurfaceRegistryEntry[] = [\n");
    for e in &reg.surfaces {
        out.push_str(&format!(
            "  {{ viewKey: {}, cliGroup: {}, tier: '{}', navLabel: {}, navIcon: {}, navGroup: {} }},\n",
            opt(&e.view_key),
            opt(&e.cli_group),
            tier(&e.representation_tier),
            opt(&e.nav_label),
            opt(&e.nav_icon),
            opt(&e.nav_group),
        ));
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(entries: Vec<SurfaceEntry>) -> SurfaceRegistry {
        SurfaceRegistry { x_vox_version: 2, schema_version: 1, surfaces: entries }
    }

    #[test]
    fn missing_groups_detects_unclassified() {
        let r = reg(vec![SurfaceEntry {
            view_key: Some("scientia".into()),
            cli_group: Some("scientia".into()),
            representation_tier: RepresentationTier::CuratedDecorator,
            nav_label: None, nav_icon: None, nav_group: None, notes: None,
        }]);
        let top: BTreeSet<String> = ["scientia", "build", "audit"].iter().map(|s| s.to_string()).collect();
        let mut got = missing_groups(&r, &top);
        got.sort();
        assert_eq!(got, vec!["audit".to_string(), "build".to_string()]);
    }

    #[test]
    fn backfill_appends_none_entries_sorted() {
        let r = reg(vec![]);
        let out = backfill(r, &["build".into(), "audit".into()]);
        assert_eq!(out.surfaces.len(), 2);
        assert_eq!(out.surfaces[0].cli_group.as_deref(), Some("audit"));
        assert_eq!(out.surfaces[0].representation_tier, RepresentationTier::None);
    }

    #[test]
    fn wiring_violation_when_view_key_absent_from_app() {
        let r = reg(vec![SurfaceEntry {
            view_key: Some("ghost".into()),
            cli_group: Some("ghost".into()),
            representation_tier: RepresentationTier::CuratedDecorator,
            nav_label: None, nav_icon: None, nav_group: None, notes: None,
        }]);
        let violations = wiring_violations(&r, "switch (activeView) { case 'dashboard': }");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("ghost"));
    }

    #[test]
    fn generate_ts_is_deterministic_and_typed() {
        let r = reg(vec![SurfaceEntry {
            view_key: Some("scientia".into()),
            cli_group: Some("scientia".into()),
            representation_tier: RepresentationTier::CuratedDecorator,
            nav_label: Some("Scientia".into()),
            nav_icon: Some("file".into()),
            nav_group: Some("research".into()),
            notes: None,
        }]);
        let ts = generate_ts(&r);
        assert!(ts.contains("export const SURFACE_REGISTRY"));
        assert!(ts.contains("viewKey: 'scientia'"));
        assert!(ts.contains("tier: 'curated_decorator'"));
    }
}
```

- [ ] **Step 2: Declare the module**

In `crates/vox-cli/src/commands/ci/mod.rs`, add alongside the other `pub mod` declarations (find `pub mod gui_surface_coverage;` and add after it):

```rust
pub mod gui_surface_registry;
```

- [ ] **Step 3: Run the tests to verify they fail (module not yet compiled into binary path / functions correct)**

Run: `cargo test -p vox-cli gui_surface_registry -- --nocapture`
Expected: PASS for the four unit tests (pure functions are fully defined). If a compile error occurs, fix the module wiring, then re-run.

> Note: these four tests cover pure logic and should pass immediately once the module compiles. The "failing test first" discipline is satisfied at the integration level in Task 3 (the check is not yet runnable as a subcommand).

- [ ] **Step 4: Add the `run()` entry point**

Append to `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` (before the `#[cfg(test)]` module). This mirrors the load+validate pattern in `gui_catalog_parity.rs:254-290` and the write/diff pattern in `gui_surface_coverage.rs:220-264`:

```rust
fn top_level_groups_from_catalog() -> BTreeSet<String> {
    crate::command_catalog::build_catalog()
        .entries
        .into_iter()
        .filter(|e| e.path.len() == 1)
        .map(|e| e.path[0].clone())
        .collect()
}

fn load_registry(repo_root: &Path) -> Result<SurfaceRegistry> {
    let path = repo_root.join(REGISTRY_PATH);
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    // Validate against the JSON schema first (parse YAML → JSON value).
    let yaml_val: serde_json::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    let schema_raw = fs::read_to_string(repo_root.join(SCHEMA_PATH)).context("read registry schema")?;
    let schema_val: serde_json::Value = serde_json::from_str(&schema_raw).context("parse registry schema")?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, SCHEMA_PATH)
        .context("compile surface-registry schema")?;
    vox_jsonschema_util::validate(&yaml_val, &validator, "surface-registry schema")
        .context("validate surface-registry against schema")?;
    serde_yaml::from_str(&raw).with_context(|| format!("deserialize {}", path.display()))
}

pub fn run(repo_root: &Path, write: bool) -> Result<()> {
    let parsed = load_registry(repo_root)?;
    let top_level = top_level_groups_from_catalog();
    let missing = missing_groups(&parsed, &top_level);
    let resolved = backfill(parsed, &missing);

    let app_src = fs::read_to_string(repo_root.join(GUI_APP)).context("read App.tsx")?;
    let violations = wiring_violations(&resolved, &app_src);

    let ts = generate_ts(&resolved);
    let report = serde_json::json!({
        "schema_version": 1,
        "top_level_groups": top_level.iter().collect::<Vec<_>>(),
        "surface_count": resolved.surfaces.len(),
        "unclassified_backfilled": missing,
        "wiring_violations": violations,
    });
    let report_str = serde_json::to_string_pretty(&report)? + "\n";

    if write {
        let yaml_out = serde_yaml::to_string(&resolved)? ;
        fs::write(repo_root.join(REGISTRY_PATH), yaml_out).context("write registry yaml")?;
        fs::write(repo_root.join(GENERATED_TS), &ts).context("write generated ts")?;
        fs::write(repo_root.join(REPORT_PATH), report_str).context("write report")?;
        println!("gui-surface-registry: wrote registry, generated TS, and report");
        return Ok(());
    }

    if !missing.is_empty() {
        return Err(anyhow!(
            "gui-surface-registry: {} unclassified top-level CLI group(s) [{}] — run `vox ci gui-surface-registry --write`, then set representation_tier in {}",
            missing.len(),
            missing.join(", "),
            REGISTRY_PATH
        ));
    }
    if !violations.is_empty() {
        return Err(anyhow!(
            "gui-surface-registry: wiring violations:\n  - {}",
            violations.join("\n  - ")
        ));
    }
    let existing_ts = fs::read_to_string(repo_root.join(GENERATED_TS)).with_context(|| {
        format!("read {GENERATED_TS} (run `vox ci gui-surface-registry --write`)")
    })?;
    if existing_ts.trim() != ts.trim() {
        return Err(anyhow!(
            "gui-surface-registry: {GENERATED_TS} drift (run `vox ci gui-surface-registry --write`)"
        ));
    }
    println!("gui-surface-registry: registry and generated TS are up to date");
    Ok(())
}
```

- [ ] **Step 5: Compile and run the unit tests again**

Run: `cargo test -p vox-cli gui_surface_registry`
Expected: PASS (4 tests). Fix any compile errors (e.g., `vox_jsonschema_util` is already a dependency of `vox-cli`, used by `gui_catalog_parity.rs`).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/gui_surface_registry.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): gui-surface-registry check — enumerate, backfill, enforce, generate TS"
```

---

## Task 3: Register and dispatch the new check

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs:71-77` (after the `GuiSurfaceCoverage` variant)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs:63` (after the `GuiSurfaceCoverage` arm)
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:518` (inside `run_ssot_drift`)

- [ ] **Step 1: Add the clap variant**

In `crates/vox-cli/src/commands/ci/cmd_enums.rs`, immediately after the `GuiSurfaceCoverage { write: bool }` variant:

```rust
    /// Generate or verify the GUI surface registry (forces every CLI group to be classified).
    #[command(name = "gui-surface-registry")]
    GuiSurfaceRegistry {
        /// Write/update the registry, generated TS, and report. Without this flag, verify only.
        #[arg(long)]
        write: bool,
    },
```

- [ ] **Step 2: Add the dispatch arm**

In `crates/vox-cli/src/commands/ci/run_body.rs`, immediately after the `CiCmd::GuiSurfaceCoverage { write } => ...` arm:

```rust
        CiCmd::GuiSurfaceRegistry { write } => super::gui_surface_registry::run(&root, write),
```

- [ ] **Step 3: Add to the ssot-drift bundle (verify mode)**

In `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs`, inside `run_ssot_drift`, immediately after the `gui_surface_coverage::run(root, false)?;` line:

```rust
    crate::commands::ci::gui_surface_registry::run(root, false)?;
```

- [ ] **Step 4: Build to verify the new subcommand wires up**

Run: `cargo build -p vox-cli`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs
git commit -m "feat(ci): register + dispatch gui-surface-registry; add to ssot-drift"
```

---

## Task 4: Backfill the registry and generate the TS artifact

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml` (tool-backfilled)
- Create: `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` (tool-generated)
- Create: `contracts/reports/gui-surface-registry.v1.json` (tool-generated)

- [ ] **Step 1: Create the generated dir placeholder**

The generated TS lives under a new `generated/` folder. Ensure it exists:

Run: `New-Item -ItemType Directory -Force crates/vox-gui/ui/src/generated | Out-Null`

- [ ] **Step 2: Run the generator (backfills `none` groups + writes TS + report)**

Run: `cargo run -p vox-cli -- ci gui-surface-registry --write`
Expected output: `gui-surface-registry: wrote registry, generated TS, and report`. The YAML now contains a `representation_tier: none` entry for every top-level CLI group that had no curated surface.

- [ ] **Step 3: Verify the round-trip passes in verify mode**

Run: `cargo run -p vox-cli -- ci gui-surface-registry`
Expected: `gui-surface-registry: registry and generated TS are up to date`. If it reports wiring violations, fix the offending `view_key` in the YAML (it must match a `'<view_key>'` literal in `App.tsx`) and re-run `--write`.

- [ ] **Step 4: Commit the generated artifacts**

```bash
git add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts contracts/reports/gui-surface-registry.v1.json
git commit -m "chore(gui): backfill surface registry + generate typed projection"
```

> **Maintenance note:** add `surfaceRegistry.generated.ts` to the team's "never hand-edit, always regenerate" list (the same policy as `*.generated.md`). Reclassify backfilled `none` entries to `generic_form`/`curated_decorator`/`live_backend` over time; the gate only requires *presence*, not a non-`none` tier.

---

## Task 5: Render the sidebar nav from the generated registry

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:88-108`

- [ ] **Step 1: Import the generated registry and the icon map**

At the top of `Sidebar.tsx`, add (the `Icon` import already exists; add the registry import):

```tsx
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
```

- [ ] **Step 2: Replace the hardcoded `<nav>` block with a data-driven render**

Replace the entire `<nav className="flex flex-col gap-1"> ... </nav>` block (lines 88-108) with:

```tsx
        <nav className="flex flex-col gap-1">
          {SURFACE_REGISTRY
            .filter(e => e.viewKey && e.navLabel && e.navGroup !== 'system')
            .map(e => {
              const IconCmp = (Icon as Record<string, any>)[e.navIcon ?? 'file'] ?? Icon.file;
              return (
                <NavItem
                  key={e.viewKey as string}
                  collapsed={collapsed}
                  active={view === e.viewKey}
                  onClick={() => setView(e.viewKey)}
                  icon={<IconCmp className="size-4" />}
                  label={e.navLabel as string}
                  badge={e.viewKey === 'flow' ? agentsCount : undefined}
                />
              );
            })}
        </nav>
```

The bottom-of-sidebar `settings` `<NavItem>` (line 136) stays as-is (it is `nav_group: system` and is intentionally excluded above).

- [ ] **Step 3: Build the frontend to verify it compiles and renders**

Run (from repo root): `pnpm --dir crates/vox-gui/ui build`
Expected: Vite build exits 0. (This is the GUI's canonical lint/typecheck per project convention.)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx
git commit -m "refactor(gui): drive sidebar nav from generated surface registry"
```

---

## Task 6: Add the in-GUI Coverage surface

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Coverage/CoverageView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`
- Modify: `crates/vox-gui/ui/src/App.tsx` (`View` union + validation array)
- Modify: `contracts/gui/surface-registry.v1.yaml` (add the `coverage` surface) then re-run Task 4 generator

- [ ] **Step 1: Write the Coverage surface**

Create `crates/vox-gui/ui/src/components/surfaces/Coverage/CoverageView.tsx`:

```tsx
import React from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { SURFACE_REGISTRY, RepresentationTier } from '../../../generated/surfaceRegistry.generated';

const TIER_STYLE: Record<RepresentationTier, { label: string; cls: string }> = {
  none:              { label: 'Unrepresented', cls: 'text-zinc-500 ring-white/10' },
  generic_form:      { label: 'Generic form',  cls: 'text-cyan-300 ring-cyan-400/25' },
  curated_decorator: { label: 'Curated',       cls: 'text-emerald-300 ring-emerald-400/25' },
  live_backend:      { label: 'Live backend',  cls: 'text-brass ring-brass/30' },
};

export function CoverageView(_props: SurfaceDecoratorProps) {
  const rows = [...SURFACE_REGISTRY].sort((a, b) =>
    (a.cliGroup ?? a.viewKey ?? '').localeCompare(b.cliGroup ?? b.viewKey ?? ''));
  const counts = rows.reduce<Record<string, number>>((acc, r) => {
    acc[r.tier] = (acc[r.tier] ?? 0) + 1; return acc;
  }, {});
  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Surface Coverage</h2>
      <div className="flex flex-wrap gap-2 text-[11px]">
        {(Object.keys(TIER_STYLE) as RepresentationTier[]).map(t => (
          <span key={t} className={`rounded-full px-2 py-0.5 ring-1 ${TIER_STYLE[t].cls}`}>
            {TIER_STYLE[t].label}: {counts[t] ?? 0}
          </span>
        ))}
      </div>
      <div className="overflow-auto rounded-lg border border-white/10">
        <table className="w-full text-left text-[12px]">
          <thead className="text-zinc-500">
            <tr><th className="p-2">CLI group</th><th className="p-2">View</th><th className="p-2">Tier</th></tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i} className="border-t border-white/5">
                <td className="p-2 font-mono text-zinc-300">{r.cliGroup ?? '—'}</td>
                <td className="p-2 text-zinc-400">{r.viewKey ?? '—'}</td>
                <td className="p-2"><span className={`rounded px-1.5 py-0.5 ring-1 ${TIER_STYLE[r.tier].cls}`}>{TIER_STYLE[r.tier].label}</span></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Register the decorator**

In `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`, add the import and the `coverage` entry to `surfaceDecorators`:

```tsx
import { CoverageView } from './Coverage/CoverageView';
```

Add inside the `surfaceDecorators` object (after `claims: ClaimsView,`):

```tsx
  coverage: CoverageView,
```

- [ ] **Step 3: Add `coverage` to the App.tsx View union and validation array**

In `crates/vox-gui/ui/src/App.tsx`, add `| 'coverage'` to the `View` union (after `'settings'`), and add `'coverage'` to the validation array at line ~230 (`['dashboard', ... 'settings']` → include `'coverage'`).

- [ ] **Step 4: Add the `coverage` surface to the registry and regenerate**

In `contracts/gui/surface-registry.v1.yaml`, add:

```yaml
  - { view_key: coverage, cli_group: null, representation_tier: live_backend, nav_label: Coverage, nav_icon: scale, nav_group: system }
```

Then regenerate and verify:

Run: `cargo run -p vox-cli -- ci gui-surface-registry --write`
Run: `cargo run -p vox-cli -- ci gui-surface-registry`
Expected: up-to-date, no violations (`'coverage'` is now in the View union, so wiring passes).

> Note: `nav_group: system` keeps Coverage out of the main `<nav>` loop; reach it via the bottom group or the command palette. To pin it in the main nav, change `nav_group` to `operate` and re-run the generator.

- [ ] **Step 5: Build**

Run: `pnpm --dir crates/vox-gui/ui build`
Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Coverage/CoverageView.tsx crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts crates/vox-gui/ui/src/App.tsx contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts contracts/reports/gui-surface-registry.v1.json
git commit -m "feat(gui): in-product surface coverage view"
```

---

## Task 7: Full verification gates

- [ ] **Step 1: Architecture check**

Run: `cargo run -p vox-arch-check`
Expected: no new violations (no new crates were added; only an existing-crate module + a contract).

- [ ] **Step 2: The new gate (verify mode) + existing GUI gates**

Run: `cargo run -p vox-cli -- ci gui-surface-registry`
Run: `cargo run -p vox-cli -- ci gui-surface-coverage`
Run: `cargo run -p vox-cli -- ci gui-catalog-parity`
Expected: all three pass.

- [ ] **Step 3: Rust unit tests + frontend build**

Run: `cargo test -p vox-cli gui_surface_registry`
Run: `pnpm --dir crates/vox-gui/ui build`
Expected: tests pass; build exits 0.

- [ ] **Step 4: Prove the gate bites (negative test, then revert)**

Temporarily delete one backfilled `representation_tier: none` line for an arbitrary CLI group from `contracts/gui/surface-registry.v1.yaml`, then:

Run: `cargo run -p vox-cli -- ci gui-surface-registry`
Expected: FAIL with "unclassified top-level CLI group(s)". Restore the line (or re-run `--write`) and confirm it passes again. This proves new CLI groups will fail the build until classified.

- [ ] **Step 5: Final commit (if the negative test left changes)**

```bash
git add -A
git commit -m "test(ci): confirm gui-surface-registry fails on unclassified groups"
```

---

## Self-Review

- **Spec coverage:** SSOT registry (Task 1), CI gate that fails on unclassified groups (Tasks 2–3, proven in Task 7 Step 4), generated TS driving nav (Tasks 4–5), in-GUI discoverability (Task 6). All four parts of the keystone recommendation are covered.
- **Type consistency:** `RepresentationTier` snake_case values match between the schema enum, the Rust enum (`#[serde(rename_all = "snake_case")]`), `generate_ts`, and the TS `RepresentationTier` type. `SurfaceRegistryEntry` field names (`viewKey`/`cliGroup`/`tier`/`navLabel`/`navIcon`/`navGroup`) match between `generate_ts` output and both `Sidebar.tsx` and `CoverageView.tsx` consumers.
- **No placeholders:** every code step contains complete code; every command has expected output.

## Deferred (out of scope, intentionally)

- Generating the `App.tsx` `View` union and `renderView` switch from the registry (larger refactor; the parity gate already prevents silent drift by requiring `view_key ∈ App.tsx`).
- Grouped/collapsible nav sections by `nav_group` (the data is present; this is a visual enhancement).
