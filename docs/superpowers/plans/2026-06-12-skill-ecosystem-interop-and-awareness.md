# Skill Ecosystem Interop & Model Awareness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Sandbox caveat:** in Claude worktree sandboxes, dispatched subagents are READ-ONLY (shell/write denied). There, execute inline via superpowers:executing-plans; reserve subagent dispatch for research/review. In a normal session, tracks A–D are independent after Task 0 and can run as parallel subagents (one per track), with a review subagent between waves.

**Goal:** Make Vox's skill system interoperable with the agentskills.io ecosystem (Claude Code, Cursor, Codex, Copilot, …), auto-discover skills from standard directories, surface them in GUI chat as `/slash` commands, and make every model (MENS local + OpenRouter) skill-aware with usage telemetry — all through the existing SSOT (parser in `vox-plugin-host`, types in `vox-plugin-types`, registry in `vox-skills`, catalog in `vox-plugin-catalog`).

**Architecture:** Interop is additive on the existing spine. The spec's YAML SKILL.md is the exchange format (G1 fix landed); `metadata.vox-*` keys carry Vox extensions losslessly. New discovery walks vendor-neutral skill roots into the same `SkillRegistry`. Model awareness replicates the spec's three-tier progressive disclosure: tier-1 name+description catalog injected into `build_system_prompt` (works for prompt-only MENS), tier-2 `vox_skill_use` MCP tool for tool-calling models, explicit-skill body injection for user-pinned skills, telemetry on every activation.

**Tech Stack:** Rust (serde_yaml/toml/serde_json, walkdir, tempfile for tests), MCP via rmcp + `contracts/operations/catalog.v1.yaml` sync commands, React/TS + Vitest (pnpm, NOT npm), Playwright (port 1420).

**Research basis:** `docs/src/architecture/skill-ecosystem-interop-research-2026-06-12.md` (read first — contains the verified ecosystem table and license catalog).

**House rules that bind every task (AGENTS.md):**
- Test-first (AGENTS.md §Test-First Policy). Write the failing test, watch it fail, implement, watch it pass, commit.
- Format with `cargo fmt -p <crate>` or `vox run scripts/fmt.vox` — **never `cargo fmt --all`** (Windows os error 206).
- On this machine build with `-j 4` if sccache-wrapped rustc crashes with 0xffffffff.
- Automation glue = `.vox` scripts only (no .ps1/.sh/.py).
- New `docs/src/**.md` files need YAML frontmatter (title/description/category) at creation time.
- Never hand-edit generated files (tool-registry.canonical.yaml, surfaceRegistry.generated.ts, SUMMARY.md) — run the official sync commands and iterate `vox ci ssot-drift` to convergence.
- No stubs/placeholders; scope down instead.

---

## Task 0: Spec-compliant YAML frontmatter parsing — ✅ LANDED

Commit `3df43e7316` (`crates/vox-plugin-host/src/skill_parser.rs`, `Cargo.toml`): frontmatter normalizes through `serde_json::Value` — TOML first (legacy exact-compat; YAML `key: value` is invalid TOML so no ambiguity), YAML fallback; list-valued `vox-*` metadata accepts arrays or comma-separated strings; category defaults to `Unknown`. 16/16 tests green incl. 4 new YAML tests (real superpowers-style frontmatter, minimal spec shape, optional `license`/`compatibility`/`allowed-tools`, folded multiline description).

- [x] All steps complete.

---

## Track A — Interop auto-discovery (vox-config → vox-plugin-host → vox-cli)

### Task A1: Standard skill-root resolution

**Files:**
- Modify: `crates/vox-config/src/paths.rs` (module that owns `REPO_MEMORY_INDEX_FILE`)
- Test: same file, `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn skill_search_roots_orders_vox_then_agents_then_claude() {
    let ws = std::path::Path::new("/repo");
    let roots = skill_search_roots(ws);
    // Workspace roots come first (highest precedence), then user-home roots.
    let rel: Vec<String> = roots
        .iter()
        .take(3)
        .map(|p| p.strip_prefix(ws).unwrap().to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(rel, vec![".vox/skills", ".agents/skills", ".claude/skills"]);
    // User-home roots mirror the same order under the home dir.
    assert_eq!(roots.len(), 6);
    let home = dirs::home_dir().unwrap();
    assert_eq!(roots[3], home.join(".vox/skills"));
    assert_eq!(roots[4], home.join(".agents/skills"));
    assert_eq!(roots[5], home.join(".claude/skills"));
}
```

- [ ] **Step 2: Run** `cargo test -p vox-config skill_search_roots -j 4` — expect FAIL (function not defined).

- [ ] **Step 3: Implement**

```rust
/// Skill discovery roots, highest precedence first.
///
/// `.vox/skills` is Vox-native; `.agents/skills` is the vendor-neutral
/// agentskills.io convention (Codex, Cursor, Copilot, Amp); `.claude/skills`
/// is the most widely honored compatibility path. Workspace beats user-home.
/// First id wins on collision during install.
pub fn skill_search_roots(workspace_root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> = [".vox/skills", ".agents/skills", ".claude/skills"]
        .iter()
        .map(|d| workspace_root.join(d))
        .collect();
    if let Some(home) = dirs::home_dir() {
        roots.extend(
            [".vox/skills", ".agents/skills", ".claude/skills"]
                .iter()
                .map(|d| home.join(d)),
        );
    }
    roots
}
```

(If `vox-config` lacks the `dirs` dep, add `dirs = { workspace = true }` — it is already a workspace dep used by vox-plugin-host.)

- [ ] **Step 4: Run** `cargo test -p vox-config skill_search_roots -j 4` — expect PASS.
- [ ] **Step 5: Commit** `feat(config): standard skill discovery roots (.vox/.agents/.claude × workspace/home)`

### Task A2: Discover bare SKILL.md skill directories

A "bare" skill is `<root>/<dir>/SKILL.md` with **no `Plugin.toml`** — the universal
ecosystem layout. Reuse `parse_skill_md` (Task 0) and install into the same
`SkillRegistry` exactly the way `discover.rs:68-81` does for plugin skills.

**Files:**
- Create: `crates/vox-plugin-host/src/external_skills.rs`
- Modify: `crates/vox-plugin-host/src/lib.rs` (add `pub mod external_skills;`)
- Test: inline `#[cfg(test)]` with `tempfile::tempdir()`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &std::path::Path, dir: &str, frontmatter_name: &str) {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("SKILL.md"),
            format!(
                "---\nname: {frontmatter_name}\ndescription: Test skill body for {frontmatter_name}\n---\n\n# Body\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn discovers_yaml_skill_dirs_under_root() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(tmp.path(), "test-driven-development", "test-driven-development");
        write_skill(tmp.path(), "brainstorming", "brainstorming");
        let found = discover_external_skills(&[tmp.path().to_path_buf()]);
        let mut ids: Vec<&str> = found.iter().map(|s| s.bundle.manifest.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["brainstorming", "test-driven-development"]);
        assert!(found[0].bundle.skill_md.contains("# Body"));
    }

    #[test]
    fn first_root_wins_on_id_collision() {
        let hi = tempfile::tempdir().unwrap();
        let lo = tempfile::tempdir().unwrap();
        write_skill(hi.path(), "tdd", "tdd");
        write_skill(lo.path(), "tdd", "tdd");
        let found = discover_external_skills(&[hi.path().to_path_buf(), lo.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.starts_with(hi.path()));
    }

    #[test]
    fn skips_unparseable_and_missing_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("broken");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("SKILL.md"), "no frontmatter at all").unwrap();
        let missing = tmp.path().join("does-not-exist");
        let found = discover_external_skills(&[tmp.path().to_path_buf(), missing]);
        assert!(found.is_empty());
    }
}
```

- [ ] **Step 2: Run** `cargo test -p vox-plugin-host external_skills -j 4` — expect FAIL (module/function not defined).

- [ ] **Step 3: Implement**

```rust
//! Discovery of bare SKILL.md skill directories (agentskills.io layout) —
//! `<root>/<skill-dir>/SKILL.md`, no Plugin.toml. Complements `discover.rs`,
//! which owns Plugin.toml-based plugin skills.

use crate::skill_bundle::VoxSkillBundle;
use crate::skill_parser::parse_skill_md;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A skill found in an external (non-plugin) skill root.
pub struct ExternalSkill {
    /// Directory containing the SKILL.md.
    pub path: PathBuf,
    /// Parsed bundle (manifest + raw body).
    pub bundle: VoxSkillBundle,
}

/// Walk each root's immediate subdirectories for `SKILL.md`, highest-precedence
/// root first; the first skill seen for a given manifest id wins.
pub fn discover_external_skills(roots: &[PathBuf]) -> Vec<ExternalSkill> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort(); // deterministic order within a root
        for dir in dirs {
            let md = dir.join("SKILL.md");
            if !md.is_file() {
                continue;
            }
            match std::fs::read_to_string(&md).map_err(|e| e.to_string()).and_then(|s| {
                parse_skill_md(&s).map_err(|e| e.to_string())
            }) {
                Ok(bundle) => {
                    if dir_name_mismatch(&dir, &bundle.manifest.name) {
                        tracing::warn!(
                            path = %md.display(), name = %bundle.manifest.name,
                            "skill name does not match directory name (spec violation); loading anyway"
                        );
                    }
                    if seen.insert(bundle.manifest.id.clone()) {
                        out.push(ExternalSkill { path: dir, bundle });
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %md.display(), error = %e, "skipping unparseable SKILL.md");
                }
            }
        }
    }
    out
}

fn dir_name_mismatch(dir: &Path, name: &str) -> bool {
    dir.file_name().map(|d| d.to_string_lossy() != name).unwrap_or(true)
}
```

- [ ] **Step 4: Run** `cargo test -p vox-plugin-host -j 4` — expect PASS (all, incl. Task 0's 16).
- [ ] **Step 5: Commit** `feat(plugin-host): discover bare SKILL.md skill dirs (agentskills.io layout)`

### Task A3: Wire external discovery into `vox skill` CLI + registry

**Files:**
- Modify: `crates/vox-cli/src/commands/extras/ars/discover.rs` (workspace walk → also report standard roots)
- Modify: `crates/vox-cli/src/commands/extras/ars/mod.rs` (`make_registry()` — read its current body first; it builds the `vox_skills::SkillRegistry`)
- Test: `crates/vox-cli/tests/` integration test or `#[cfg(test)]` in ars module, per existing ars test idiom (check `crates/vox-cli/src/commands/extras/ars/` for existing tests and mirror)

- [ ] **Step 1: Write the failing test** — registry assembly includes external skills:

```rust
#[tokio::test]
async fn registry_includes_external_skills_from_standard_roots() {
    let ws = tempfile::tempdir().unwrap();
    let dir = ws.path().join(".agents/skills/brainstorming");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: brainstorming\ndescription: Socratic design refinement\n---\nBody\n",
    )
    .unwrap();
    let registry = make_registry_at(ws.path()).await; // new workspace-rooted variant
    let listed = registry.list(None);
    assert!(listed.iter().any(|s| s.id == "brainstorming"));
}
```

- [ ] **Step 2: Run** `cargo test -p vox-cli --features ars registry_includes_external -j 4` — expect FAIL.

- [ ] **Step 3: Implement** — in `make_registry` (factored as `make_registry_at(ws_root)` with the old name delegating to `make_registry_at(std::env::current_dir()…)`), after existing construction:

```rust
let roots = vox_config::paths::skill_search_roots(ws_root);
for ext in vox_plugin_host::external_skills::discover_external_skills(&roots) {
    // Same conversion discover.rs uses for plugin skills (discover.rs:68-81):
    // external skills expose no MCP tools by default — body is prompt material.
    registry.install_bundle(ext.bundle, /* source = */ Some(ext.path));
}
```

If `SkillRegistry` lacks an `install_bundle` accepting a `VoxSkillBundle`, add it in `crates/vox-skills/src/` (or `vox-plugin-host/src/skill_registry.rs`, wherever `install` lives — follow the `LoadedSkill` conversion in `discover.rs:68-81` verbatim, with `plugin_id = format!("external:{}", bundle.manifest.id)`). Registry ids must not collide with plugin skills: existing `install` semantics (first wins) already guarantee plugin-skills-then-external ordering if external install runs after plugin discovery — preserve that order in `make_registry_at`.

- [ ] **Step 4:** Also extend `ars::discover()` (`discover.rs:8`) to print a second section "External skill roots" listing each `skill_search_roots` dir, found skills, and installed/not-installed status — same `owo_colors` formatting as the existing section.
- [ ] **Step 5: Run** `cargo test -p vox-cli --features ars -j 4` — expect PASS. Manually verify: `vox skill discover` and `vox skill list` from a workspace with `.claude/skills/` shows them.
- [ ] **Step 6: Commit** `feat(cli): vox skill discover/list ingest standard external skill roots`

### Task A4: Extend `agentskills-compliance` CI gate to YAML + bundled skills

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/agentskills_compliance.rs`
- Test: gate's existing test module (read it first; mirror its fixture pattern)

- [ ] **Step 1:** Failing test: a fixture YAML SKILL.md under a temp `assets/skills/<name>/` passes the gate; a YAML skill whose `name` ≠ dirname fails; description > 1024 chars fails.
- [ ] **Step 2:** Run gate tests — FAIL.
- [ ] **Step 3:** Implement: gate currently validates `crates/vox-plugin-skill-*/<name>.skill.md` (TOML); add a second walk over `assets/skills/*/SKILL.md` (Track D's bundle dir) applying: frontmatter parses via `parse_skill_md` (now YAML-capable), name regex `^[a-z0-9][a-z0-9-]{0,63}$`, name == directory name (hard error for *bundled* skills — we control them), description 1..=1024.
- [ ] **Step 4:** Tests PASS; run `vox ci agentskills-compliance` locally.
- [ ] **Step 5: Commit** `feat(ci): agentskills-compliance validates YAML bundled skills (name==dir, desc<=1024)`

---

## Track B — Model awareness: MENS + OpenRouter (vox-orchestrator-mcp)

### Task B1: Tier-1 skill catalog in the system prompt

This is the keystone for MENS: catalog injection requires **no tool-calling**, so
the local `/generate` path (`llm_bridge/mod.rs:46-114`) becomes skill-aware purely
through the prompt. Keep it **day-stable and alphabetical** — `build_system_prompt`
deliberately preserves DeepSeek/Anthropic prompt-prefix caching (see the NOTE at
`chat_tools/mod.rs:110-112`); a skill list that reorders per call would bust it.

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/chat_tools/skill_catalog.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/mod.rs` (mod decl + injection in `build_system_prompt` after the Environment section, before ANTI_LAZINESS_RIDER)
- Test: inline `#[cfg(test)]` (pure function — no ServerState needed)

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: &str, d: &str) -> CatalogEntry {
        CatalogEntry { name: n.into(), description: d.into() }
    }

    #[test]
    fn renders_alphabetical_capped_catalog() {
        let txt = render_skill_catalog(
            &[s("zeta", "Last"), s("brainstorming", "Socratic design refinement")],
            64,
        );
        let b = txt.find("brainstorming").unwrap();
        let z = txt.find("zeta").unwrap();
        assert!(b < z, "alphabetical for prompt-prefix cache stability");
        assert!(txt.contains("## Skills"));
        assert!(txt.contains("vox_skill_use"));
    }

    #[test]
    fn empty_registry_renders_nothing() {
        assert_eq!(render_skill_catalog(&[], 64), "");
    }

    #[test]
    fn caps_entry_count_and_description_length() {
        let many: Vec<CatalogEntry> =
            (0..100).map(|i| s(&format!("skill-{i:03}"), &"x".repeat(2000))).collect();
        let txt = render_skill_catalog(&many, 10);
        assert_eq!(txt.matches("\n- ").count(), 10);
        assert!(!txt.contains(&"x".repeat(300)), "descriptions truncated");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p vox-orchestrator-mcp skill_catalog -j 4` — FAIL.

- [ ] **Step 3: Implement**

```rust
//! Tier-1 skill disclosure for the system prompt (agentskills.io model):
//! name + description only (~100 tokens/skill). Tier-2 (full body) is loaded
//! via the `vox_skill_use` tool, or inline for an explicitly selected skill.

pub(crate) struct CatalogEntry {
    pub name: String,
    pub description: String,
}

const DESC_CAP: usize = 256;

/// Render the `## Skills` system-prompt section. Alphabetical and content-stable
/// so it never busts prompt-prefix caches. Empty input → empty string.
pub(crate) fn render_skill_catalog(entries: &[CatalogEntry], max: usize) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&CatalogEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    sorted.truncate(max);
    let mut out = String::from(
        "\n\n## Skills\nInstalled skills (name — when to use). To apply one, call the \
         `vox_skill_use` tool with its name to load the full instructions, then follow them. \
         If tools are unavailable, state which skill applies and proceed by its description.\n",
    );
    for e in sorted {
        let mut d = e.description.replace('\n', " ");
        if d.len() > DESC_CAP {
            d.truncate(DESC_CAP);
            d.push('…');
        }
        out.push_str(&format!("- {} — {}\n", e.name, d));
    }
    out
}
```

In `build_system_prompt` (after the Environment block at `mod.rs:90-93`):

```rust
let skill_entries: Vec<skill_catalog::CatalogEntry> = state
    .orchestrator
    .skill_registry()
    .list(None)
    .into_iter()
    .map(|s| skill_catalog::CatalogEntry { name: s.name, description: s.description })
    .collect();
prompt.push_str(&skill_catalog::render_skill_catalog(&skill_entries, 64));
```

If `Orchestrator` lacks a public `skill_registry()` accessor (the registry lives on
`monitor.rs` / `orchestrator/core/mod.rs` via `new_registry_arc()`), add the
one-line accessor returning the `Arc<vox_skills::SkillRegistry>` clone.

- [ ] **Step 4: Run** `cargo test -p vox-orchestrator-mcp -j 4` — PASS.
- [ ] **Step 5: Commit** `feat(mcp): inject tier-1 skill catalog into chat system prompt (cache-stable)`

### Task B2: `vox_skill_use` tool (tier-2 progressive disclosure) + activation telemetry

**Files:**
- Modify: `contracts/operations/catalog.v1.yaml` — add operation `skill.use` (mcp_name `vox_skill_use`, product_lane `interop`, tier `core`, safety_class read-only) **then run the official sync commands** (`vox ci operations-sync`, command/capability sync, regenerate `contracts/mcp/tool-registry.canonical.yaml`; iterate `vox ci ssot-drift` to convergence — do NOT hand-edit generated YAML)
- Create: handler in the module that implements the other `vox_skill_*` tools (locate via `rg "vox_skill_list" crates/vox-orchestrator-mcp/src` and mirror its registration + input-schema pattern in `input_schemas.rs`)
- Test: alongside the existing `vox_skill_list` handler tests (same file/idiom)

- [ ] **Step 1: Failing test:** handler returns the registered skill's body for `{"name": "brainstorming"}` (install a fixture skill into the test registry first), structured error for unknown name, and emits one `skill_activated` telemetry event (assert via the crate's telemetry test capture idiom — `rg "telemetry" crates/vox-plugin-host/src/telemetry.rs` for the producer pattern).
- [ ] **Step 2:** FAIL.
- [ ] **Step 3: Implement** the handler:

```rust
/// vox_skill_use { name } → { name, description, body }
/// Tier-2 disclosure: returns the full SKILL.md body for one skill.
pub async fn skill_use(state: &ServerState, name: &str) -> Result<serde_json::Value, McpToolError> {
    let reg = state.orchestrator.skill_registry();
    let Some(skill) = reg.list(None).into_iter().find(|s| s.name == name || s.id == name) else {
        return Err(McpToolError::invalid_params(format!(
            "unknown skill '{name}'; call vox_skill_list for available names"
        )));
    };
    vox_telemetry::event(
        "skill_activated",
        &[("skill", skill.id.as_str()), ("source", "tool")],
    );
    Ok(serde_json::json!({
        "name": skill.name,
        "description": skill.description,
        "body": reg.body(&skill.id),
    }))
}
```

(Adapt error type / telemetry call / `reg.body()` accessor names to what the
neighboring `vox_skill_info` handler actually uses — read it first; add a
`body(&id)` registry accessor if only the manifest is exposed today.)

- [ ] **Step 4:** PASS; `vox ci ssot-drift` green; tool appears in the HTTP gateway read-role allowlist (`http_gateway/mod.rs:56-90` — add `vox_skill_use`, it is read-only).
- [ ] **Step 5: Commit** `feat(mcp): vox_skill_use tier-2 skill loading + skill_activated telemetry`

### Task B3: Explicit skill pinning works on every model (incl. MENS)

The GUI already submits `active_skill` (Loquela payload, `Loquela.tsx:286`), but
`vox_chat_message` ignores it. When a skill is pinned, inject its **full body**
into the system prompt — no tool round-trip, so prompt-only MENS honors it.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/params.rs` (`ChatMessageParams`: add `#[serde(default)] pub skill: Option<String>`)
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat.rs` (`chat_message`: pass param through), `mod.rs` (`build_system_prompt` gains `pinned_skill: Option<&str>` parameter; update its other call sites — find via `rg "build_system_prompt" crates/vox-orchestrator-mcp`)
- Test: extend `skill_catalog.rs` tests with `render_pinned_skill`

- [ ] **Step 1: Failing test:**

```rust
#[test]
fn pinned_skill_section_contains_full_body() {
    let txt = render_pinned_skill("tdd", "# TDD\nRED-GREEN-REFACTOR.");
    assert!(txt.contains("## Active skill: tdd"));
    assert!(txt.contains("RED-GREEN-REFACTOR"));
    assert!(txt.contains("follow these instructions"));
}
```

- [ ] **Step 2:** FAIL. **Step 3: Implement** `render_pinned_skill(name, body) -> String` in `skill_catalog.rs` (header + "The user pinned this skill — follow these instructions for this task." + body, body capped at 32 KiB), look up the body via the registry in `build_system_prompt` when `pinned_skill` is `Some`, and emit `skill_activated` with `("source", "pinned")`. Tier-1 catalog still renders (other skills remain discoverable).
- [ ] **Step 4:** `cargo test -p vox-orchestrator-mcp -j 4` PASS. End-to-end check: `vox_chat_message` with `"skill": "test-driven-development"` against the local MENS endpoint (`VOX_LOCAL_ENDPOINT`) — response reflects the skill instructions.
- [ ] **Step 5: Commit** `feat(mcp): pinned-skill body injection — skills work on prompt-only MENS path`

### Task B4: Skill performance tracking surface

Telemetry events from B2/B3 give activations. Close the loop minimally now;
scoreboard/ARM integration is a later phase (see Phase Next).

- [ ] **Step 1:** Failing test: `vox skill list --stats` includes an `activations` column sourced from the telemetry store (read `crates/vox-telemetry` for the local query API; if none exists for counters, persist a simple `skill_stats.json` next to the registry DB via the existing `PluginStateBackend`).
- [ ] **Step 2-4:** RED → implement → GREEN (`cargo test -p vox-cli --features ars -j 4`).
- [ ] **Step 5: Commit** `feat(cli): vox skill list --stats shows activation counts`

---

## Track C — GUI: /slash skills + palette (pnpm, vitest)

### Task C1: Dynamic slash entries from the live skill registry

**Files:**
- Create: `crates/vox-gui/ui/src/lib/slashCommands.ts`
- Create: `crates/vox-gui/ui/src/lib/slashCommands.test.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from 'vitest';
import { buildSlashEntries, BUILTIN_SLASH } from './slashCommands';

describe('buildSlashEntries', () => {
  it('appends skills after builtins as /name with kind=skill', () => {
    const entries = buildSlashEntries([
      { id: 'vox.tdd', name: 'test-driven-development', description: 'RED-GREEN-REFACTOR' },
    ]);
    expect(entries.slice(0, BUILTIN_SLASH.length)).toEqual(BUILTIN_SLASH);
    const skill = entries.find(e => e.cmd === '/test-driven-development');
    expect(skill).toMatchObject({ kind: 'skill', skillId: 'vox.tdd', desc: 'RED-GREEN-REFACTOR' });
  });

  it('dedupes a skill that collides with a builtin command', () => {
    const entries = buildSlashEntries([{ id: 'x', name: 'plan', description: 'collides with /plan' }]);
    expect(entries.filter(e => e.cmd === '/plan')).toHaveLength(1);
    expect(entries.find(e => e.cmd === '/plan')!.kind).toBe('builtin');
  });

  it('tolerates malformed skill records', () => {
    expect(buildSlashEntries([{} as any, null as any])).toEqual(BUILTIN_SLASH);
  });
});
```

- [ ] **Step 2: Run** `pnpm -C crates/vox-gui/ui test slashCommands` — FAIL.

- [ ] **Step 3: Implement**

```ts
export interface SlashEntry {
  cmd: string;
  desc: string;
  icon: string;
  kind: 'builtin' | 'skill';
  skillId?: string;
}

// The 8 verbs previously hardcoded in Loquela.tsx (LQ_SLASH) move here verbatim.
export const BUILTIN_SLASH: SlashEntry[] = [
  { cmd: '/plan',     desc: 'Draft a multi-step plan without executing',   icon: 'flow',   kind: 'builtin' },
  { cmd: '/spawn',    desc: 'Spin up a sub-agent on this branch',          icon: 'agent',  kind: 'builtin' },
  { cmd: '/audit',    desc: 'Socrates citation + invariant audit on file', icon: 'shield', kind: 'builtin' },
  { cmd: '/verify',   desc: 'Run rule-pack + property tests',              icon: 'check',  kind: 'builtin' },
  { cmd: '/doubt',    desc: 'Inject doubt at threshold N',                 icon: 'alert',  kind: 'builtin' },
  { cmd: '/memory',   desc: 'Query Mnemosyne (RAG over project memory)',   icon: 'memory', kind: 'builtin' },
  { cmd: '/rollback', desc: 'Revert to last durable checkpoint',           icon: 'back',   kind: 'builtin' },
  { cmd: '/diff',     desc: 'Show pending diff staged by agent',           icon: 'file',   kind: 'builtin' },
];

interface SkillRecord { id?: string; name?: string; description?: string }

/** Builtins first (stable), then installed skills as /skill-name, deduped by cmd. */
export function buildSlashEntries(skills: (SkillRecord | null | undefined)[]): SlashEntry[] {
  const out = [...BUILTIN_SLASH];
  const taken = new Set(out.map(e => e.cmd));
  for (const s of skills ?? []) {
    const name = s?.name?.trim();
    if (!name) continue;
    const cmd = `/${name}`;
    if (taken.has(cmd)) continue;
    taken.add(cmd);
    out.push({ cmd, desc: s?.description ?? '', icon: 'bolt', kind: 'skill', skillId: s?.id ?? name });
  }
  return out;
}
```

In `Loquela.tsx`: delete `LQ_SLASH` (lines 24-33), `const allSlash = useMemo(() => buildSlashEntries(skills), [skills]);`, point `filteredSlash` at `allSlash`, and in `insertSlash` (line ~275): for `kind === 'skill'`, call `setActiveSkill(skills.find(s => s.id === entry.skillId) ?? entry.skillId)` and clear the slash text instead of inserting it (the skill rides in the payload's `active_skill`, which Task B3 now honors end-to-end).

- [ ] **Step 4: Run** `pnpm -C crates/vox-gui/ui test` — PASS (incl. existing `loquelaContext.test.ts`, `chatCorrelation.test.ts`).
- [ ] **Step 5: Commit** `feat(gui): Loquela slash menu lists installed skills as /skill-name`

### Task C2: Skills in the Cmd+K command palette

**Files:** Modify `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx` (+ a vitest for a new pure `mergeSkillResults(query, skills, results)` helper in `src/lib/`).

- [ ] **Step 1:** Failing test: query "tdd" surfaces a `{ kind: 'skill' }` result for a skill named `test-driven-development` (substring/word match on name+description); empty query yields none; backend results stay first.
- [ ] **Step 2-3:** RED → implement helper + wire into the palette's result assembly (`CommandPalette.tsx:101-113`), selecting a skill result opens the Skills surface with that skill focused (the existing `'skill'` locator kind, `open_locator` handler at line 71).
- [ ] **Step 4:** `pnpm -C crates/vox-gui/ui test` PASS.
- [ ] **Step 5: Commit** `feat(gui): command palette surfaces installed skills`

### Task C3: Discovered-but-uninstalled skills in SkillsPluginsView

**Files:** add `vox_skill_discover` MCP operation (same SSOT sync flow as B2 — catalog.v1.yaml → sync → ssot-drift; handler wraps `discover_external_skills(skill_search_roots(ws))` returning `{path, name, description, installed}` JSON), then a third tab "Discovered" in `SkillsPluginsView.tsx` with one-click install (existing `vox_skill_install` HITL flow).

- [ ] **Step 1:** Rust failing test for the handler (fixture root → JSON listing, `installed:false`). **Step 2-4:** RED → GREEN (`cargo test -p vox-orchestrator-mcp -j 4`).
- [ ] **Step 5:** Frontend tab + vitest for its data mapper; `pnpm -C crates/vox-gui/ui test` PASS.
- [ ] **Step 6:** `vox ci gui-surface-registry` + `vox ci ssot-drift` green.
- [ ] **Step 7: Commit** `feat(gui+mcp): vox_skill_discover + Discovered tab with one-click install`

---

## Track D — Bundle the license-verified skill set

**Only what the research verified as redistributable.** Sources pinned by SHA; every
skill dir carries upstream LICENSE + provenance. **Excluded, do not vendor:**
anthropics docx/pdf/pptx/xlsx (proprietary, no-redistribution), and
obra/superpowers-developing-for-claude-code (asserts MIT, LICENSE file missing).

### Task D1: Vendor skills into `assets/skills/`

- [ ] **Step 1:** Write the vendoring script as **VoxScript** (AGENTS.md §VoxScript-First): `scripts/vendor-skills.vox` reading `assets/skills/SOURCES.toml`:

```toml
# assets/skills/SOURCES.toml — provenance SSOT for vendored skills.
[[source]]
repo = "https://github.com/obra/superpowers"
license = "MIT"
pin = "<SHA at vendoring time>"
skills = [
  "test-driven-development", "systematic-debugging", "writing-plans",
  "executing-plans", "verification-before-completion", "requesting-code-review",
  "receiving-code-review", "brainstorming", "subagent-driven-development",
  "dispatching-parallel-agents", "using-git-worktrees",
  "finishing-a-development-branch", "writing-skills", "using-superpowers",
]

[[source]]
repo = "https://github.com/obra/superpowers-lab"
license = "MIT"
pin = "<SHA>"
skills = ["finding-duplicate-functions", "mcp-cli", "using-tmux-for-interactive-commands"]
# windows-vm intentionally deferred: skill files are MIT but it orchestrates
# externally-licensed components (dockur/windows, Windows 11).

[[source]]
repo = "https://github.com/anthropics/skills"
license = "Apache-2.0"
pin = "<SHA>"
skills = ["mcp-builder", "skill-creator"]
# docx/pdf/pptx/xlsx are PROPRIETARY (no redistribution) — never add them here.
```

The script clones each repo at `pin` into a temp dir, copies `skills/<name>/`
(SKILL.md + references/scripts/assets) into `assets/skills/<name>/`, copies the
repo LICENSE into each skill dir as `LICENSE.upstream`, and rewrites nothing.

- [ ] **Step 2:** Run `vox run scripts/vendor-skills.vox`; spot-check 3 skills parse: `vox skill install assets/skills/test-driven-development/SKILL.md` then `vox skill info`.
- [ ] **Step 3:** Reconcile the 4 stale copies at `crates/vox-skills/skills/superpowers/` — delete them in favor of `assets/skills/` (single source of truth) and update whatever loads that dir (find via `rg "skills/superpowers" crates/`).
- [ ] **Step 4:** `vox ci agentskills-compliance` (A4 now validates `assets/skills/`) — green. The licensing pre-check from AGENTS.md/legal hygiene: `ATTRIBUTIONS.md` (repo root, or extend if it exists — check first) gains one row per source repo.
- [ ] **Step 5: Commit** `feat(skills): vendor 19 license-verified skills (superpowers MIT ×17, anthropics Apache-2.0 ×2)`

### Task D2: Register the bundle in the SSOT catalog + ship via discovery

- [ ] **Step 1:** Failing test: `vox-plugin-catalog` test asserting every `assets/skills/<dir>` has a `catalog.toml` `[[skill-bundle]]` entry (id, source repo, license, pin) and vice versa — parity in both directions.
- [ ] **Step 2-3:** RED → add entries + the parity check (extend the catalog crate's existing validation tests; follow the `[[plugin]]` entry idiom at `catalog.toml:81-88`).
- [ ] **Step 4:** Add the bundled dir as the **lowest-precedence** discovery root: in `skill_search_roots` callers (A3), append the resolved `assets/skills` (dev: workspace-relative; installed: skip if absent). Test: bundled skill appears in `vox skill list` from a clean workspace; a workspace `.vox/skills/` skill with the same id shadows it.
- [ ] **Step 5:** `cargo test -p vox-plugin-catalog -p vox-cli --features ars -j 4` PASS. **Commit** `feat(catalog): bundled skill-set parity + lowest-precedence discovery root`

---

## Track E — SSOT hygiene, docs, integration verification

### Task E1: Where-things-live + reference docs

- [ ] Add rows to `docs/src/architecture/where-things-live.md`: `external_skills.rs` (bare SKILL.md interop discovery), `assets/skills/` (vendored license-verified bundle), `skill_catalog.rs` (tier-1/tier-2 prompt disclosure). Update `docs/src/reference/skill_marketplace.md` (discovery roots table, /slash usage, `vox_skill_use`, `--stats`). Frontmatter rule applies only to *new* docs/src files (research doc already compliant).
- [ ] **Commit** `docs(skills): where-things-live rows + marketplace reference for interop discovery`

### Task E2: Full-system verification sweep

- [ ] `cargo test -p vox-plugin-host -p vox-skills -p vox-config -p vox-orchestrator-mcp -p vox-plugin-catalog -j 4` — all green.
- [ ] `cargo test -p vox-cli --features ars -j 4` — green.
- [ ] `pnpm -C crates/vox-gui/ui test` — green; `pnpm -C crates/vox-gui/ui test:e2e` if the dev server runs.
- [ ] Gates: `vox ci agentskills-compliance`, `vox ci plugin-skill-parity`, `vox ci ssot-drift`, `vox ci gui-surface-registry`, `cargo run -p vox-arch-check`, `VOX_FMT_CHECK=1 vox run scripts/fmt.vox`.
- [ ] Live smoke (the user-visible claims): (1) drop a skill into `.claude/skills/`, `vox skill list` sees it; (2) GUI Loquela `/` shows it; (3) chat with MENS tier — response acknowledges the pinned skill; (4) chat with an OpenRouter tool-capable model — model calls `vox_skill_use`; (5) `vox skill list --stats` shows the activations.
- [ ] **Commit** any fixes, then run superpowers:requesting-code-review before merge; finish via superpowers:finishing-a-development-branch.

---

## Execution strategy (parallelism)

- **Wave 1 (independent, run in parallel):** A1+A2 · B1 · C1 · D1. Four workers, one per track — file sets are disjoint.
- **Wave 2 (after Wave 1):** A3+A4 · B2+B3 · C2 · D2. B2 and C3 both touch `catalog.v1.yaml` + sync — serialize those two (B2 first), or batch both operations into one sync pass.
- **Wave 3:** B4 · C3 · E1 · E2 (E2 last, single worker).
- In a normal session use superpowers:dispatching-parallel-agents per wave with superpowers:requesting-code-review between waves. A Workflow harness fits Wave-boundary verification fan-out (one reviewer agent per track, adversarial verify on "tests actually ran"). In the current read-only-subagent sandbox: execute inline, keep the wave order, commit per task.

## Phase Next (explicitly out of scope here, tracked for follow-up)

1. **Scoreboard integration:** join `skill_activated` telemetry with task outcomes (success_rate per skill×model×task-category) in the model-selection scoreboard — feeds auto-suggestion ranking.
2. **Auto-selection:** orchestrator proposes top-K skills per task from embeddings over name+description (gate behind doubt threshold); today's model is description-based selection by the LLM itself, which is the ecosystem-standard mechanism.
3. **MCP `skill://` distribution** once SEP-2640 merges (serve bundled skills over Resources; consume remote skill indexes).
4. **Permission enforcement at activation** (manifest `vox-permissions` → sandbox policy) and dependency resolution.
5. **Sourcing pass 2:** evaluate the unverified registries (awesome lists, Vercel open-agents, skillport/openskills) with the same license-verification bar.
