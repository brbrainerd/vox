# Skill Proposal — Accept → Author → Install Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Accepting a mined skill proposal authors a valid, step-bearing `SKILL.md` from the persisted candidate and installs it workspace-local under `<ws_root>/.vox/skills/<name>/`.

**Architecture:** The miner already produces a `RepeatedOperations` candidate; SP-4 (T1) repoints its `members` at the real tool sequence, persists the candidate JSON on `FeedbackRequest.meta` at propose time (T2), and on accept (T4) deserializes it, authors a `SKILL.md` via a pure `vox-plugin-host` function (T3), and installs it. A "Save as skill" GUI button (T5) triggers `{action:"accept_skill"}`.

**Tech Stack:** Rust (`vox-skill-discovery`, `vox-orchestrator`, `vox-plugin-host`, `vox-orchestrator-mcp`), TypeScript/React + Vitest (`vox-gui`).

**Windows/process notes (project policy):** never `cargo fmt --all` — use `cargo fmt -p <crate>`. Exclude `vox-gui` from `clippy --all-targets`. GUI uses pnpm, not npm. Treat `.vox` as Vox source. Subagents are read-only in the worktree sandbox: write + commit from the main session. Never pipe cargo to head/grep on Windows — redirect to a temp file and read it.

---

### Task 1: Mined `members` carry the tool sequence, not session anchors

**Why:** Today `op_miner.rs` stores `members: a.anchors` (`"session:s1@…"`) and discards `a.tools` (`["a","b","c"]`). Authoring a `SKILL.md` body from anchors yields a hollow stub. This task makes `members` the actual procedure — a net deletion of the anchor bookkeeping.

**Files:**
- Modify: `crates/vox-skill-discovery/src/op_miner.rs` (the `Agg` struct ~93-96, the accumulation loop ~109-121, the candidate `members`/`score` ~130-150)
- Test: `crates/vox-skill-discovery/src/op_miner.rs` (existing `tests` module)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/vox-skill-discovery/src/op_miner.rs` (after `detects_sequence_recurring_across_sessions`):

```rust
    #[test]
    fn members_are_the_tool_sequence_not_anchors() {
        let mut ops = Vec::new();
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s1", 10, &["a", "b", "c"]));
        ops.extend(seq("s2", 0, &["a", "b", "c"]));
        let cands = mine_repeated_operations(&ops, &default_opts());
        let abc = cands
            .iter()
            .find(|c| c.draft_frontmatter.as_ref().map(|d| d.name.as_str()) == Some("a-b-c"))
            .expect("expected a-b-c candidate");
        assert_eq!(abc.members, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert!(abc.members.iter().all(|m| !m.starts_with("session:")));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-skill-discovery members_are_the_tool_sequence` (redirect to a temp file on Windows, then read it).
Expected: FAIL — `members` currently equal `["session:s1@0", …]`.

- [ ] **Step 3: Repoint `members` and delete the anchor bookkeeping**

In `crates/vox-skill-discovery/src/op_miner.rs`, remove `anchors` from the `Agg` struct:

```rust
    struct Agg {
        count: usize,
        sessions: BTreeSet<String>,
        tools: Vec<String>,
    }
```

In the aggregation loop, drop the `anchors: Vec::new()` initializer and the entire `if e.anchors.len() < 20 { … }` push block — leave only:

```rust
                let e = agg.entry(gram).or_insert_with(|| Agg {
                    count: 0,
                    sessions: BTreeSet::new(),
                    tools: tools[start..start + len].to_vec(),
                });
                e.count += 1;
                e.sessions.insert((*sid).to_string());
```

In the candidate `.map(|a| { … })`, capture the tool count before moving `a.tools` into `members` (the current `score` line reads `a.tools.len()`, which would be a use-after-move):

```rust
            let arrow = a.tools.join(" → ");
            let name = ngram_name(&a.tools);
            let tool_count = a.tools.len();
            Candidate {
                kind: CandidateKind::RepeatedOperations,
                members: a.tools,
                score: (a.count * tool_count) as f32,
                suggested_action: "Save recurring procedure as a skill".to_string(),
                draft_frontmatter: Some(DraftFrontmatter {
                    name,
                    description: format!(
                        "Recurring procedure: {arrow} (seen {}× across {} sessions)",
                        a.count,
                        a.sessions.len()
                    ),
                    category: "workflow".to_string(),
                    tags: vec!["auto-discovered".to_string(), "operations".to_string()],
                }),
            }
```

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p vox-skill-discovery` (redirect on Windows).
Expected: PASS (new test passes; `detects_sequence_recurring_across_sessions` still passes — it asserts on `description`/`name`, not `members`).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-skill-discovery/src/op_miner.rs
git commit -m "feat(skill-discovery): mined members carry tool sequence, drop session anchors"
```

---

### Task 2: `FeedbackRequest.meta` + `FeedbackAction::AcceptSkill`

**Why:** Persist the candidate JSON on the feedback item, and add the accept action at the orchestrator core. `register` gains a 12th argument threaded through all three existing call sites.

**Files:**
- Modify: `crates/vox-orchestrator/src/feedback/types.rs` (`FeedbackRequest` ~42-56, `FeedbackAction` ~22-32, round-trip test literal ~63-77)
- Modify: `crates/vox-orchestrator/src/feedback/store.rs` (`register` ~32-67, test helper `reg` ~124-138)
- Modify: `crates/vox-orchestrator/src/orchestrator/agent/doubt.rs` (the `register(` call ~89)
- Modify: `crates/vox-orchestrator/src/orchestrator/agent/propose.rs` (signature ~7-12, `register(` call ~28, test ~62/66)

- [ ] **Step 1: Write the failing tests**

In `crates/vox-orchestrator/src/feedback/types.rs`, add to the `tests` module:

```rust
    #[test]
    fn accept_skill_serializes_to_tagged_action() {
        let j = serde_json::to_string(&FeedbackAction::AcceptSkill).unwrap();
        assert!(j.contains("\"action\":\"accept_skill\""), "got {j}");
        let back: FeedbackAction = serde_json::from_str(&j).unwrap();
        assert_eq!(back, FeedbackAction::AcceptSkill);
    }

    #[test]
    fn request_round_trips_with_meta() {
        let req = FeedbackRequest {
            id: FeedbackId("F-000002".into()),
            kind: FeedbackKind::SkillProposal,
            prompt: "p".into(),
            options: vec!["Dismiss".into()],
            gates: vec![],
            doubted_task_id: None,
            info_gain_bits: 0.0,
            scaled_cost_ms: 0,
            surface: Surface::NeedsYou,
            session_id: None,
            agent_id: None,
            created_at_ms: 1,
            resolution: None,
            meta: Some(serde_json::json!({"kind": "RepeatedOperations"})),
        };
        let j = serde_json::to_string(&req).unwrap();
        let back: FeedbackRequest = serde_json::from_str(&j).unwrap();
        assert_eq!(back.meta, req.meta);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-orchestrator accept_skill_serializes` (redirect on Windows).
Expected: FAIL to compile — `AcceptSkill` and `meta` do not exist yet.

- [ ] **Step 3: Add the variant and the field**

In `crates/vox-orchestrator/src/feedback/types.rs`, extend `FeedbackAction`:

```rust
pub enum FeedbackAction {
    Answer {
        option: Option<usize>,
        text: Option<String>,
    },
    Skip,
    Overrule,
    LetVerify,
    /// Accept a `SkillProposal`: author + install the skill from the item's `meta`.
    AcceptSkill,
}
```

Add the field to `FeedbackRequest` (after `resolution`):

```rust
    pub resolution: Option<FeedbackResolution>,
    /// Opaque per-item payload. For `SkillProposal`, the serialized mined `Candidate`.
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
```

Add `meta: None,` to the existing `round_trips_with_snake_case_tags` literal (after `resolution: None,`).

- [ ] **Step 4: Thread `meta` through `register` and its three call sites**

In `crates/vox-orchestrator/src/feedback/store.rs`, add the 12th parameter and set the field:

```rust
        agent_id: Option<AgentId>,
        created_at_ms: u64,
        meta: Option<serde_json::Value>,
    ) -> FeedbackId {
        let mut inner = self.inner.write();
        inner.seq += 1;
        let id = FeedbackId(format!("F-{:06}", inner.seq));
        let req = FeedbackRequest {
            id: id.clone(),
            kind,
            prompt,
            options,
            gates,
            doubted_task_id,
            info_gain_bits,
            scaled_cost_ms,
            surface,
            session_id,
            agent_id,
            created_at_ms,
            resolution: None,
            meta,
        };
```

In the same file's `reg` test helper, add `None,` as the final argument (after `1,`).

In `crates/vox-orchestrator/src/orchestrator/agent/doubt.rs` at the `register(` call (~89), add `None,` as the final argument (after `ts`).

In `crates/vox-orchestrator/src/orchestrator/agent/propose.rs`, add a `meta` parameter and pass it:

```rust
    pub fn propose_skill(
        &self,
        name: &str,
        description: &str,
        session_id: Option<String>,
        meta: Option<serde_json::Value>,
    ) -> Option<FeedbackId> {
```

In its `register(` call, the trailing argument list ends with `ts,` — change to:

```rust
            ts,
            meta,
        );
```

Update the test `propose_skill_registers_needs_you_and_dedups`: both `orch.propose_skill("read-edit-run", desc, Some("s1".into()))` calls gain a trailing `, None` argument.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p vox-orchestrator` (redirect on Windows, then read the file).
Expected: PASS — all feedback tests green, `propose_skill` test green. A compile error here means a `FeedbackRequest` literal or `register` call was missed — fix and re-run.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/types.rs crates/vox-orchestrator/src/feedback/store.rs crates/vox-orchestrator/src/orchestrator/agent/doubt.rs crates/vox-orchestrator/src/orchestrator/agent/propose.rs
git commit -m "feat(orchestrator): FeedbackRequest.meta + FeedbackAction::AcceptSkill"
```

---

### Task 3: `author_skill_md` in `vox-plugin-host`

**Why:** A pure function turning `(name, description, steps)` into a spec-valid `SKILL.md` string, kebab-casing the name so it always passes `validate_skill_name`. Primitive-typed to avoid a `vox-skill-discovery` dependency in `vox-plugin-host`.

**Files:**
- Create: `crates/vox-plugin-host/src/skill_author.rs`
- Modify: `crates/vox-plugin-host/src/lib.rs` (module declaration + re-export)

- [ ] **Step 1: Confirm the parser/validator paths**

Run: `grep -rn "pub fn parse_skill_md\|pub fn validate_skill_name\|pub fn install_to_user_root\|pub struct InstalledUserSkill" crates/vox-plugin-host/src`
Note the module paths — the test's `use` lines below must match.

- [ ] **Step 2: Write the failing test (the file)**

Create `crates/vox-plugin-host/src/skill_author.rs`:

```rust
//! Compose a spec-valid `SKILL.md` from primitive inputs (SP-4 skill authoring).

/// Lowercase, collapse non-`[a-z0-9]` runs to single hyphens, trim/dedupe hyphens.
/// Guarantees the result passes `validate_skill_name` for any input (falls back
/// to `"skill"` if nothing survives).
fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_hyphen = true; // suppress leading hyphen
    for ch in name.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(64).collect();
    let trimmed = trimmed.trim_end_matches('-').to_string();
    if trimmed.is_empty() { "skill".to_string() } else { trimmed }
}

/// Build a TOML-frontmatter `SKILL.md`. `steps` render as a numbered list of
/// inline-code tokens; an empty list yields a valid file with no steps.
pub fn author_skill_md(name: &str, description: &str, steps: &[String]) -> String {
    let name = kebab(name);
    let desc = description.replace('"', "'"); // keep the TOML string valid + single-line
    let steps_md = if steps.is_empty() {
        "_No individual steps were captured._".to_string()
    } else {
        steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. `{}`", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "---\n\
name = \"{name}\"\n\
description = \"{desc}\"\n\
\n\
[metadata]\n\
\"vox-author\" = \"vox-skill-discovery\"\n\
\"vox-category\" = \"workflow\"\n\
\"vox-tags\" = [\"auto-discovered\", \"operations\"]\n\
---\n\
\n\
# {name}\n\
\n\
{desc}\n\
\n\
## Steps\n\
\n\
{steps_md}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    // Adjust these two paths to match Step 1's findings.
    use crate::skill_parser::parse_skill_md;
    use crate::user_install::validate_skill_name;

    #[test]
    fn authored_skill_round_trips_and_is_valid() {
        let md = author_skill_md(
            "Read Edit Run!!",
            "Recurring procedure: read → edit → run (seen 4× across 2 sessions)",
            &["read".into(), "edit".into(), "run".into()],
        );
        let parsed = parse_skill_md(&md).expect("authored SKILL.md must parse");
        assert_eq!(parsed.name, "read-edit-run");
        assert!(validate_skill_name(&parsed.name).is_ok());
        assert!(md.contains("1. `read`"));
        assert!(md.contains("3. `run`"));
    }

    #[test]
    fn empty_steps_still_valid() {
        let md = author_skill_md("proc", "desc", &[]);
        let parsed = parse_skill_md(&md).expect("must parse");
        assert_eq!(parsed.name, "proc");
    }
}
```

> The `parse_skill_md` return shape may expose the name as `parsed.name` or via a frontmatter sub-struct — adjust the assertion to the real accessor (Step 1 output shows it). The function body is independent of this.

- [ ] **Step 3: Wire the module**

In `crates/vox-plugin-host/src/lib.rs`, add alongside the other `mod`/`pub use` lines:

```rust
mod skill_author;
pub use skill_author::author_skill_md;
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-plugin-host skill_author` (redirect on Windows).
Expected: PASS. If `parse_skill_md` rejects TOML frontmatter, switch the emitted block to the YAML form the parser accepts (`name: "…"`, `description: "…"`, nested `metadata:`) and re-run; the round-trip test is the authority on format.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-plugin-host/src/skill_author.rs crates/vox-plugin-host/src/lib.rs
git commit -m "feat(plugin-host): author_skill_md composes spec-valid SKILL.md"
```

---

### Task 4: MCP wiring — accept action, candidate persistence, author+install

**Why:** Add `AcceptSkill` at the MCP boundary (the closed `McpFeedbackAction` enum that gates deserialization), persist the candidate via `ProposeSkillParams.candidate`, and implement the accept arm that authors + installs. `resolve_feedback` returns `String` — the accept arm uses `ToolResult` builders, never `?`.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml` (add `vox-skill-discovery` dep; `tempfile` dev-dep)
- Modify: `crates/vox-orchestrator-mcp/src/params.rs` (`McpFeedbackAction` ~1038, `From` impl ~1048, `ProposeSkillParams` ~392)
- Modify: `crates/vox-orchestrator-mcp/src/feedback_tools.rs` (`resolve_feedback` ~119, `propose_skill` ~194, add helper + tests)

- [ ] **Step 1: Write the failing tests**

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs`, add to the `tests` module:

```rust
    #[test]
    fn mcp_accept_skill_deserializes() {
        let v = serde_json::json!({"action": "accept_skill"});
        let a: crate::params::McpFeedbackAction = serde_json::from_value(v).unwrap();
        let core: vox_orchestrator::feedback::FeedbackAction = a.into();
        assert_eq!(core, vox_orchestrator::feedback::FeedbackAction::AcceptSkill);
    }

    #[test]
    fn author_and_install_writes_workspace_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = serde_json::json!({
            "kind": "RepeatedOperations",
            "members": ["read", "edit", "run"],
            "score": 6.0,
            "suggested_action": "Save recurring procedure as a skill",
            "draft_frontmatter": {
                "name": "read-edit-run",
                "description": "Recurring procedure: read → edit → run (seen 4× across 2 sessions)",
                "category": "workflow",
                "tags": ["auto-discovered", "operations"]
            }
        });
        let names = super::author_and_install_skill(&candidate, tmp.path()).unwrap();
        assert_eq!(names, vec!["read-edit-run".to_string()]);
        let f = tmp.path().join(".vox").join("skills").join("read-edit-run").join("SKILL.md");
        assert!(f.exists(), "expected {f:?} to exist");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-orchestrator-mcp author_and_install_writes` (redirect on Windows).
Expected: FAIL to compile — `author_and_install_skill`, `AcceptSkill`, and possibly `tempfile` do not exist.

- [ ] **Step 3: Add dependencies**

In `crates/vox-orchestrator-mcp/Cargo.toml`, under `[dependencies]`:

```toml
vox-skill-discovery = { path = "../vox-skill-discovery" }
```

and under `[dev-dependencies]` (only if `tempfile` is absent there):

```toml
tempfile = "3"
```

Run `cargo build -p vox-orchestrator-mcp` here to confirm no dependency cycle before writing code.

- [ ] **Step 4: Add `AcceptSkill` to the MCP action enum and `From` impl**

In `crates/vox-orchestrator-mcp/src/params.rs`, extend `McpFeedbackAction`:

```rust
pub enum McpFeedbackAction {
    Answer {
        option: Option<usize>,
        text: Option<String>,
    },
    Skip,
    Overrule,
    LetVerify,
    AcceptSkill,
}
```

and the `From<McpFeedbackAction>` impl's match:

```rust
            McpFeedbackAction::LetVerify => Self::LetVerify,
            McpFeedbackAction::AcceptSkill => Self::AcceptSkill,
        }
    }
}
```

Add a `candidate` field to `ProposeSkillParams`:

```rust
pub struct ProposeSkillParams {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub session_id: Option<String>,
    /// Serialized mined `Candidate`, persisted on the feedback item for accept-time authoring.
    #[serde(default)]
    pub candidate: Option<serde_json::Value>,
}
```

- [ ] **Step 5: Thread the candidate through `propose_skill`**

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs`, update the `propose_skill` call to pass the candidate as the new 4th argument (matching `Orchestrator::propose_skill` from Task 2). Keep whatever orchestrator accessor the existing line uses; only add the argument:

```rust
    let fid = state
        .orchestrator()
        .propose_skill(&params.name, &params.description, params.session_id, params.candidate);
```

- [ ] **Step 6: Add the `author_and_install_skill` helper**

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs` at module scope (not inside `tests`):

```rust
use std::path::Path;

/// Author a `SKILL.md` from a serialized mined `Candidate` and install it
/// workspace-local under `<ws_root>/.vox/skills/<name>/`. Returns installed names.
pub(crate) fn author_and_install_skill(
    candidate: &serde_json::Value,
    ws_root: &Path,
) -> Result<Vec<String>, String> {
    use vox_skill_discovery::candidate::Candidate;
    let cand: Candidate = serde_json::from_value(candidate.clone())
        .map_err(|e| format!("bad candidate payload: {e}"))?;
    let df = cand
        .draft_frontmatter
        .ok_or_else(|| "candidate has no draft frontmatter".to_string())?;
    let md = vox_plugin_host::author_skill_md(&df.name, &df.description, &cand.members);

    // Author into a unique temp dir, then install via the hardened installer.
    let safe: String = df
        .name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let tmp = std::env::temp_dir().join(format!("vox-skill-author-{safe}"));
    let skill_dir = tmp.join("skill");
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
    std::fs::write(skill_dir.join("SKILL.md"), md).map_err(|e| e.to_string())?;

    let installed = vox_plugin_host::install_to_user_root(&tmp, ws_root, false, None)?;
    let _ = std::fs::remove_dir_all(&tmp); // best-effort cleanup
    Ok(installed.into_iter().map(|i| i.name).collect())
}
```

> Verify `install_to_user_root`'s arity/types and `InstalledUserSkill`'s name field against Step 1 of Task 3. If the 4th arg (`skill_filter`) is not `Option<…>` or the field is not `.name`, adjust.

- [ ] **Step 7: Add the accept arm to `resolve_feedback`**

`resolve_feedback` resolves the item via the shared `state.feedback().resolve(...)` before side-effects (existing architecture). After that resolve and the existing Doubt/Overrule block — and before the function's final success `to_json()` — add:

```rust
    if req.kind == vox_orchestrator::feedback::FeedbackKind::SkillProposal
        && matches!(action, vox_orchestrator::feedback::FeedbackAction::AcceptSkill)
    {
        let Some(candidate) = req.meta.as_ref() else {
            return ToolResult::<serde_json::Value>::err("skill proposal has no candidate payload")
                .to_json();
        };
        let Some(ws_root) = state.workspace_root.clone() else {
            return ToolResult::<serde_json::Value>::err("no workspace root; cannot install skill")
                .to_json();
        };
        return match author_and_install_skill(candidate, &ws_root) {
            Ok(names) => {
                ToolResult::ok(serde_json::json!({"resolved": true, "installed": names})).to_json()
            }
            Err(e) => ToolResult::<serde_json::Value>::err(&e).to_json(),
        };
    }
```

> `req` and `action` are already bound earlier in the function; `state.workspace_root` is `Option<PathBuf>`. This early-returns with the installed names instead of the generic `{"resolved": true}`.

- [ ] **Step 8: Run to verify pass**

Run: `cargo test -p vox-orchestrator-mcp feedback` then `cargo test -p vox-orchestrator-mcp author_and_install` (redirect on Windows).
Expected: PASS — `mcp_accept_skill_deserializes`, `author_and_install_writes_workspace_skill`, and the existing `test_feedback_tools_lifecycle` all green.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-orchestrator-mcp/Cargo.toml crates/vox-orchestrator-mcp/src/params.rs crates/vox-orchestrator-mcp/src/feedback_tools.rs
git commit -m "feat(mcp): accept_skill action authors + installs from persisted candidate"
```

---

### Task 5: GUI "Save as skill" button

**Why:** The `skill_proposal` branch of `FeedbackCard.tsx` renders only Dismiss. Add an accept button firing `{action:'accept_skill'}`. No `FeedbackRow`/transport change — the candidate lives server-side; the transport (`feedbackResolve`, `Record<string, unknown>`) is a permissive pass-through.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx` (the `row.kind === 'skill_proposal'` branch ~14-34)
- Test: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.test.tsx`

- [ ] **Step 1: Confirm the component's export + prop shape**

Run: `grep -n "export.*FeedbackCard\|onResolve\|function FeedbackCard\|props" crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`
Note named-vs-default export, the prop name carrying the row, and the `onResolve` signature — the test below must match.

- [ ] **Step 2: Write the failing test**

Create/extend `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.test.tsx` (adjust import + props to Step 1):

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FeedbackCard } from './FeedbackCard';

const proposalRow = {
  feedbackId: 'F-9',
  kind: 'skill_proposal' as const,
  prompt: "Recurring procedure 'read-edit-run'",
  options: ['Dismiss'],
  gates: [],
  doubtedTaskId: null,
  surface: 'needs_you' as const,
  infoGainBits: 0,
};

describe('FeedbackCard skill_proposal', () => {
  it('Save as skill emits accept_skill; Dismiss emits skip', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={proposalRow} onResolve={onResolve} />);
    fireEvent.click(screen.getByRole('button', { name: /save as skill/i }));
    expect(onResolve).toHaveBeenCalledWith('F-9', { action: 'accept_skill' });
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(onResolve).toHaveBeenCalledWith('F-9', { action: 'skip' });
  });
});
```

- [ ] **Step 3: Run to verify failure**

Run (in `crates/vox-gui/ui`): `pnpm vitest run FeedbackCard`
Expected: FAIL — no "Save as skill" button.

- [ ] **Step 4: Add the button**

In the `row.kind === 'skill_proposal'` branch of `FeedbackCard.tsx`, add beside Dismiss (reuse the className of the file's existing primary button for visual consistency):

```tsx
      <button
        type="button"
        aria-label="Save as skill"
        className="ds-btn ds-btn-primary"
        onClick={() => onResolve(row.feedbackId, { action: 'accept_skill' })}
      >
        Save as skill
      </button>
```

- [ ] **Step 5: Run to verify pass**

Run (in `crates/vox-gui/ui`): `pnpm vitest run FeedbackCard`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.test.tsx
git commit -m "feat(gui): Save as skill button on skill proposals"
```

---

## Final verification

- [ ] `cargo test -p vox-skill-discovery -p vox-orchestrator -p vox-plugin-host -p vox-orchestrator-mcp` (redirect to a temp file on Windows; never pipe to head/grep).
- [ ] `cargo clippy -p vox-orchestrator-mcp -p vox-plugin-host -p vox-skill-discovery -p vox-orchestrator -- -D warnings` (exclude `vox-gui`).
- [ ] `cargo fmt -p vox-orchestrator-mcp -p vox-plugin-host -p vox-skill-discovery -p vox-orchestrator` (never `cargo fmt --all`).
- [ ] In `crates/vox-gui/ui`: `pnpm vitest run`.
- [ ] Then invoke **superpowers:finishing-a-development-branch**.

## Spec coverage self-check

- Miner value-gap (members→tools) → Task 1.
- `FeedbackRequest.meta` + `FeedbackAction::AcceptSkill` + `register` threading (3 sites) → Task 2.
- `author_skill_md` (kebab, round-trip, empty steps) → Task 3.
- `McpFeedbackAction::AcceptSkill` + `From`, `ProposeSkillParams.candidate`, `propose_skill` 4th arg, `author_and_install_skill` (global=false), accept arm (`String`/`ToolResult`, resolve-then-act, `workspace_root` guard), `vox-skill-discovery` dep → Task 4.
- GUI button (no `FeedbackRow` change) → Task 5.
- Error handling (missing meta, install failure, empty members, no workspace root) → Task 4 Steps 6-7.
