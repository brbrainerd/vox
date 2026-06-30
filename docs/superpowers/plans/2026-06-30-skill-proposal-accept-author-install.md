# Skill Proposal: Accept → Author → Install Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Accepting a mined skill proposal authors a valid `SKILL.md` from the stored candidate and installs it into the user skill root.

**Architecture:** Add a `meta: Option<serde_json::Value>` payload to `FeedbackRequest` (carries the serialized `Candidate` at propose time) and a `FeedbackAction::AcceptSkill` variant. A type-agnostic `author_skill_md` in `vox-plugin-host` composes the file; the `vox_resolve_feedback` accept arm deserializes the candidate, authors, and calls the existing `install_to_user_root`. GUI adds a **Save as skill** button.

**Tech Stack:** Rust (vox-orchestrator, vox-plugin-host, vox-orchestrator-mcp), TypeScript/React + vitest (vox-gui).

**Spec:** `docs/superpowers/specs/2026-06-30-skill-proposal-accept-author-install-design.md`

---

## File Structure

- `crates/vox-orchestrator/src/feedback/types.rs` — add `FeedbackRequest.meta`; add `FeedbackAction::AcceptSkill`.
- `crates/vox-orchestrator/src/feedback/store.rs` — `register` gains `meta` param; test helper updated.
- `crates/vox-orchestrator/src/orchestrator/agent/propose.rs` — `propose_skill` gains `meta` param, stores candidate.
- `crates/vox-orchestrator/src/orchestrator/agent/doubt.rs` — pass `None` for new `register` arg.
- `crates/vox-plugin-host/src/user_install.rs` — add `author_skill_md` (+ kebab helper).
- `crates/vox-orchestrator-mcp/Cargo.toml` — add `vox-skill-discovery` dep.
- `crates/vox-orchestrator-mcp/src/feedback_tools.rs` — accept arm + widen `vox_propose_skill` to accept a candidate.
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx` — **Save as skill** button.
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx` — button test.

---

## Task 1: Feedback data model — `meta` payload + `AcceptSkill` action

**Files:**
- Modify: `crates/vox-orchestrator/src/feedback/types.rs`
- Modify: `crates/vox-orchestrator/src/feedback/store.rs`
- Modify: `crates/vox-orchestrator/src/orchestrator/agent/doubt.rs`

- [ ] **Step 1: Add the failing round-trip test for `meta` + `AcceptSkill`**

In `crates/vox-orchestrator/src/feedback/types.rs`, inside the existing
`#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn meta_round_trips_and_accept_skill_tag() {
        let req = FeedbackRequest {
            id: FeedbackId("F-000002".into()),
            kind: FeedbackKind::SkillProposal,
            prompt: "save?".into(),
            options: vec!["Save as skill".into(), "Dismiss".into()],
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
        assert_eq!(back.meta, Some(serde_json::json!({"kind": "RepeatedOperations"})));

        let a = FeedbackAction::AcceptSkill;
        assert!(
            serde_json::to_string(&a)
                .unwrap()
                .contains("\"action\":\"accept_skill\"")
        );
    }
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-orchestrator --lib feedback::types::tests::meta_round_trips_and_accept_skill_tag`
Expected: FAIL — `FeedbackRequest` has no field `meta`, and `FeedbackAction` has no variant `AcceptSkill`.

- [ ] **Step 3: Add the `meta` field and `AcceptSkill` variant**

In `crates/vox-orchestrator/src/feedback/types.rs`:

Add the variant to `FeedbackAction` (after `LetVerify`):

```rust
    LetVerify,
    /// Accept a skill proposal: author a SKILL.md from the item's `meta`
    /// candidate payload and install it into the user skill root.
    AcceptSkill,
```

Add the field to `FeedbackRequest` (after `resolution`):

```rust
    pub resolution: Option<FeedbackResolution>,
    /// Opaque per-item payload. For `SkillProposal` items this holds the
    /// serialized mined `Candidate` used to author a SKILL.md on accept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
```

In the existing `round_trips_with_snake_case_tags` test, add `meta: None,` to
the `FeedbackRequest { ... }` literal (after `resolution: None,`) so it still
compiles.

- [ ] **Step 4: Add `meta` to `FeedbackStore::register` and the store test helper**

In `crates/vox-orchestrator/src/feedback/store.rs`, change `register` to accept
`meta` as the final parameter and set it on the struct:

```rust
    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &self,
        kind: FeedbackKind,
        prompt: String,
        options: Vec<String>,
        gates: Vec<TaskId>,
        doubted_task_id: Option<TaskId>,
        info_gain_bits: f64,
        scaled_cost_ms: u64,
        surface: Surface,
        session_id: Option<String>,
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
        inner.items.push(req);
        id
    }
```

In the same file's test `reg` helper, add `None` as the final argument:

```rust
    fn reg(s: &FeedbackStore, surface: Surface, gain: f64) -> FeedbackId {
        s.register(
            FeedbackKind::Clarification,
            "q?".into(),
            vec![],
            vec![TaskId(1)],
            None,
            gain,
            500,
            surface,
            None,
            None,
            1,
            None,
        )
    }
```

- [ ] **Step 5: Update every other `feedback().register(` / `.register(` feedback call site**

Search for feedback-store register calls and append `None` as the new final arg:

Run: `rg "feedback\(\)\.register\(|self\.feedback\(\)\.register\(" crates/vox-orchestrator/src`

In `crates/vox-orchestrator/src/orchestrator/agent/doubt.rs`, locate the
`.register(` call and add `None,` as the final argument (after `created_at_ms`,
before the closing `)`). Do **not** touch `propose.rs` yet — Task 3 handles it.

(Note: `register` calls in `models/`, `heartbeat.rs`, `spawn.rs`,
`catalog_refresh.rs` are on *other* types, not `FeedbackStore` — leave them.)

- [ ] **Step 6: Run the orchestrator lib tests**

Run: `cargo test -p vox-orchestrator --lib feedback`
Expected: PASS — the new test and existing feedback tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/types.rs crates/vox-orchestrator/src/feedback/store.rs crates/vox-orchestrator/src/orchestrator/agent/doubt.rs
git commit -m "feat(orchestrator): FeedbackRequest.meta payload + FeedbackAction::AcceptSkill"
```

---

## Task 2: `author_skill_md` — compose a valid SKILL.md (vox-plugin-host)

**Files:**
- Modify: `crates/vox-plugin-host/src/user_install.rs`

- [ ] **Step 1: Write the failing round-trip test**

At the bottom of `crates/vox-plugin-host/src/user_install.rs`, inside (or
appended to) the `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn author_skill_md_round_trips_and_name_is_install_safe() {
        use crate::skill_parser::parse_skill_md;
        let md = author_skill_md(
            "Read → Edit → Run!", // needs kebab-casing
            "read then edit then run (seen 4x)",
            "custom",
            &["mined".to_string()],
            &["fs.read a.vox:1".to_string(), "fs.write a.vox:2".to_string()],
        );
        // Parses as a valid SKILL.md.
        let bundle = parse_skill_md(&md).expect("authored SKILL.md must parse");
        // Top-level name is kebab-safe and passes the installer's validator.
        assert_eq!(bundle.manifest.name, "read-edit-run");
        validate_skill_name(&bundle.manifest.name).expect("name must be install-safe");
        // Body enumerates the mined members.
        assert!(md.contains("fs.read a.vox:1"));
        assert!(md.contains("fs.write a.vox:2"));
    }
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-plugin-host author_skill_md_round_trips_and_name_is_install_safe`
Expected: FAIL — `author_skill_md` is not defined.

- [ ] **Step 3: Implement `author_skill_md` + kebab helper**

In `crates/vox-plugin-host/src/user_install.rs`, add these public/free
functions (place them above the `#[cfg(test)]` module):

```rust
/// Kebab-case a free-form name into an install-safe skill name:
/// lowercase, runs of non-`[a-z0-9]` collapse to a single `-`, trimmed of
/// leading/trailing `-`, truncated to 64 chars. Falls back to "mined-skill"
/// if nothing survives.
fn kebab_skill_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for ch in raw.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let s: String = trimmed.chars().take(64).collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "mined-skill".to_string()
    } else {
        s
    }
}

/// Compose a spec-valid `SKILL.md` from mined-candidate fields. Type-agnostic
/// (no dependency on the discovery crate's `Candidate`). The returned string
/// parses via [`parse_skill_md`] and its top-level `name` passes
/// [`validate_skill_name`].
pub fn author_skill_md(
    name: &str,
    description: &str,
    category: &str,
    tags: &[String],
    members: &[String],
) -> String {
    let skill_name = kebab_skill_name(name);
    // TOML strings: escape backslashes and double-quotes only (frontmatter
    // values are single-line).
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let tags_toml = tags
        .iter()
        .map(|t| format!("\"{}\"", esc(t)))
        .collect::<Vec<_>>()
        .join(", ");
    let steps = if members.is_empty() {
        "_No individual steps were captured._".to_string()
    } else {
        members
            .iter()
            .enumerate()
            .map(|(i, m)| format!("{}. {}", i + 1, m))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "---\n\
name = \"{name}\"\n\
description = \"{desc}\"\n\
\n\
[metadata]\n\
\"vox-id\" = \"vox.skill.{name}\"\n\
\"vox-version\" = \"0.1.0\"\n\
\"vox-author\" = \"vox-skill-discovery\"\n\
\"vox-category\" = \"{category}\"\n\
\"vox-tags\" = [{tags}]\n\
---\n\
\n\
# {name}\n\
\n\
{desc}\n\
\n\
## Steps\n\
\n\
{steps}\n",
        name = skill_name,
        desc = esc(description),
        category = esc(category),
        tags = tags_toml,
        steps = steps,
    )
}
```

- [ ] **Step 4: Run the test to confirm it passes**

Run: `cargo test -p vox-plugin-host author_skill_md_round_trips_and_name_is_install_safe`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-plugin-host/src/user_install.rs
git commit -m "feat(plugin-host): author_skill_md composes a spec-valid SKILL.md from mined fields"
```

---

## Task 3: Wire propose (store candidate) + accept (author → install)

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/agent/propose.rs`
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: `crates/vox-orchestrator-mcp/src/feedback_tools.rs`

- [ ] **Step 1: Add `meta` to `propose_skill` and store it (with failing test)**

In `crates/vox-orchestrator/src/orchestrator/agent/propose.rs`, change the
signature and the `register` call:

```rust
    pub fn propose_skill(
        &self,
        name: &str,
        description: &str,
        session_id: Option<String>,
        meta: Option<serde_json::Value>,
    ) -> Option<FeedbackId> {
```

In the `register(...)` call, change the options vec and add `meta` as the final
argument:

```rust
        let fid = self.feedback().register(
            FeedbackKind::SkillProposal,
            prompt,
            vec!["Save as skill".to_string(), "Dismiss".to_string()],
            Vec::new(), // non-blocking: no gates
            None,
            0.0,
            0,
            Surface::NeedsYou,
            session_id,
            None,
            ts,
            meta,
        );
```

Update the existing `propose_skill_registers_needs_you_and_dedups` test: both
`orch.propose_skill(...)` calls gain a final argument. Use a candidate-shaped
value on the first:

```rust
        let meta = Some(serde_json::json!({
            "kind": "RepeatedOperations",
            "members": ["fs.read a.vox:1", "fs.write a.vox:2"],
            "score": 0.9,
            "suggested_action": "Save as a reusable skill",
            "draft_frontmatter": {
                "name": "read-edit-run",
                "description": desc,
                "category": "custom",
                "tags": ["mined"]
            }
        }));
        let f1 = orch.propose_skill("read-edit-run", desc, Some("s1".into()), meta.clone());
        assert!(f1.is_some());
        // ... existing assertions ...
        let f2 = orch.propose_skill("read-edit-run", desc, Some("s1".into()), meta);
        assert!(f2.is_none(), "duplicate proposal must be skipped");
```

Add an assertion that the stored item carries the payload:

```rust
        assert!(
            orch.feedback()
                .open_needs_you()
                .iter()
                .find(|f| f.kind == FeedbackKind::SkillProposal)
                .and_then(|f| f.meta.as_ref())
                .is_some(),
            "accepted candidate payload must be stored on the feedback item"
        );
```

- [ ] **Step 2: Run propose tests to confirm they pass after the edit**

Run: `cargo test -p vox-orchestrator --lib propose`
Expected: PASS.

- [ ] **Step 3: Update the MCP `propose_skill` caller + add the discovery dep**

In `crates/vox-orchestrator-mcp/Cargo.toml`, add under `[dependencies]`:

```toml
vox-skill-discovery.workspace = true
```

(If a `vox-skill-discovery` workspace entry does not exist in the root
`Cargo.toml [workspace.dependencies]`, add `vox-skill-discovery = { path = "crates/vox-skill-discovery" }` there.)

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs`, find the
`vox_propose_skill` tool handler. Its params struct currently has `name` and
`description`. Add an optional candidate field:

```rust
    /// The mined candidate (serialized) to persist for authoring on accept.
    #[serde(default)]
    pub candidate: Option<serde_json::Value>,
```

And pass it through the call (the producer now takes a 4th arg):

```rust
    let fid = state
        .orchestrator
        .propose_skill(&params.name, &params.description, params.session_id.clone(), params.candidate.clone());
```

Build to confirm signatures line up:

Run: `cargo build -p vox-orchestrator-mcp`
Expected: compiles.

- [ ] **Step 4: Write the failing accept integration test**

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs`, inside its
`#[cfg(test)] mod tests` (mirror the existing doubt/overrule test's setup for
constructing the MCP state + orchestrator), add:

```rust
    #[tokio::test]
    async fn accept_skill_proposal_authors_and_installs() {
        let tmp = tempfile::tempdir().unwrap();
        let ws_root = tmp.path();
        // Build MCP state whose workspace root is `ws_root`. Mirror the helper
        // used by the existing doubt test; if that helper hardcodes a root,
        // construct the state directly here so installs land under `ws_root`.
        let state = test_state_with_ws_root(ws_root);

        // Register a skill proposal carrying a candidate payload.
        let candidate = serde_json::json!({
            "kind": "RepeatedOperations",
            "members": ["fs.read a.vox:1", "fs.write a.vox:2"],
            "score": 0.9,
            "suggested_action": "Save as a reusable skill",
            "draft_frontmatter": {
                "name": "read-edit-run",
                "description": "read then edit then run",
                "category": "custom",
                "tags": ["mined"]
            }
        });
        let fid = state
            .orchestrator
            .propose_skill("read-edit-run", "read then edit then run", None, Some(candidate))
            .expect("proposal registered");

        // Resolve with accept_skill.
        let args = serde_json::json!({ "feedback_id": fid.0, "action": "accept_skill" });
        let res = resolve_feedback(&state, args).await.expect("resolve ok");

        // SKILL.md exists under the user skill root.
        let installed = ws_root.join(".vox").join("skills").join("read-edit-run").join("SKILL.md");
        assert!(installed.exists(), "expected installed skill at {}", installed.display());
        let body = std::fs::read_to_string(&installed).unwrap();
        assert!(body.contains("fs.read a.vox:1"));
        let _ = res;
    }
```

> Implementer note: `test_state_with_ws_root` is illustrative. Use whatever the
> existing doubt-resolution test in this file uses to build state; the only
> requirement is that the workspace root used by the accept arm is `ws_root` (a
> tempdir) so the install is isolated. `tempfile` is already a dev-dependency in
> this crate's tests (verify with `rg "tempfile" crates/vox-orchestrator-mcp/Cargo.toml`;
> add `tempfile.workspace = true` under `[dev-dependencies]` if absent).

- [ ] **Step 5: Run it to confirm it fails**

Run: `cargo test -p vox-orchestrator-mcp accept_skill_proposal_authors_and_installs`
Expected: FAIL — the resolve handler does not yet handle `AcceptSkill`.

- [ ] **Step 6: Implement the accept arm in `resolve_feedback`**

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs`, in `resolve_feedback`,
add handling for `FeedbackKind::SkillProposal` + `FeedbackAction::AcceptSkill`.
Fetch the item, read `meta`, deserialize into `vox_skill_discovery::Candidate`,
author, write to a tempdir, install:

```rust
    // Accept a mined skill proposal: author a SKILL.md from the stored
    // candidate and install it into the user skill root.
    if matches!(action, FeedbackAction::AcceptSkill) {
        let item = state
            .orchestrator
            .feedback()
            .get(&fid)
            .ok_or_else(|| "unknown feedback id".to_string())?;
        let meta = item
            .meta
            .clone()
            .ok_or_else(|| "skill proposal has no candidate payload".to_string())?;
        let cand: vox_skill_discovery::Candidate = serde_json::from_value(meta)
            .map_err(|e| format!("candidate payload is not a Candidate: {e}"))?;
        let fm = cand.draft_frontmatter.unwrap_or(vox_skill_discovery::DraftFrontmatter {
            name: "mined-skill".into(),
            description: cand.suggested_action.clone(),
            category: "custom".into(),
            tags: vec!["mined".into()],
        });
        let md = vox_plugin_host::author_skill_md(
            &fm.name,
            &fm.description,
            &fm.category,
            &fm.tags,
            &cand.members,
        );
        // Author into a tempdir as <name>/SKILL.md, then install.
        let safe = vox_plugin_host::kebab_skill_name_pub(&fm.name); // see note
        let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let skill_dir = tmp.path().join(&safe);
        std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
        std::fs::write(skill_dir.join("SKILL.md"), &md).map_err(|e| e.to_string())?;
        let installed = vox_plugin_host::install_to_user_root(
            tmp.path().to_string_lossy().as_ref(),
            state.workspace_root(),
            true,
            None,
        )?;
        // Mark resolved.
        let ts = now_ms();
        state.orchestrator.feedback().resolve(
            &fid,
            vox_orchestrator::feedback::FeedbackResolution {
                action: FeedbackAction::AcceptSkill,
                decided_at_ms: ts,
                decided_by: "gui".into(),
            },
        );
        let names: Vec<String> = installed.into_iter().map(|s| s.name).collect();
        return Ok(serde_json::json!({ "installed": names }));
    }
```

> Implementer notes (resolve real names against the file as you go):
> - `state.workspace_root()` — use whatever accessor this crate already uses to
>   reach the workspace root (grep `workspace_root` / `ws_root` in the crate).
>   If none exists, thread the tempdir root used in the test.
> - `now_ms()` — reuse the timestamp helper the doubt arm already uses; if it's
>   inline, inline the same `SystemTime` snippet.
> - The authored file's top-level `name` is kebab-cased *inside* `author_skill_md`,
>   so the directory name must match. Rather than expose a second public helper,
>   prefer: write the file to `tmp.path()` directly and let `install_to_user_root`
>   discover it via `find_skill_dirs` (it walks for any `SKILL.md`), keying the
>   installed dir off the parsed `name`. **Simplify Step 6 accordingly:** create
>   `tmp.path()/skill/SKILL.md` with a fixed inner dir name (e.g. `skill`) — the
>   installed directory is named from the parsed frontmatter `name`, not the
>   source dir name. This drops the need for any `kebab_skill_name_pub`.

  Revised, simpler tempdir block (use this instead of the `safe`/`kebab_skill_name_pub` lines):

```rust
        let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let skill_dir = tmp.path().join("skill");
        std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
        std::fs::write(skill_dir.join("SKILL.md"), &md).map_err(|e| e.to_string())?;
        let installed = vox_plugin_host::install_to_user_root(
            tmp.path().to_string_lossy().as_ref(),
            state.workspace_root(),
            true,
            None,
        )?;
```

- [ ] **Step 7: Run the integration test to confirm it passes**

Run: `cargo test -p vox-orchestrator-mcp accept_skill_proposal_authors_and_installs`
Expected: PASS — `read-edit-run/SKILL.md` exists under `<ws_root>/.vox/skills`.

- [ ] **Step 8: Clippy + commit**

Run: `cargo clippy -p vox-orchestrator-mcp -p vox-orchestrator -- -D warnings`
Expected: no warnings.

```bash
git add crates/vox-orchestrator/src/orchestrator/agent/propose.rs crates/vox-orchestrator-mcp/Cargo.toml crates/vox-orchestrator-mcp/src/feedback_tools.rs Cargo.toml
git commit -m "feat(mcp): accept skill proposal -> author SKILL.md -> install_to_user_root"
```

---

## Task 4: GUI — "Save as skill" button

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`

- [ ] **Step 1: Ensure deps installed**

Run: `cd crates/vox-gui/ui && pnpm install`
Expected: completes (project uses pnpm, not npm).

- [ ] **Step 2: Add the failing test**

In `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`,
add inside `describe('FeedbackCard', ...)`:

```tsx
  it('skill_proposal: Save as skill resolves with accept_skill action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={proposal} onResolve={onResolve} onOpenContext={() => {}} />);
    fireEvent.click(screen.getByText('Save as skill'));
    expect(onResolve).toHaveBeenCalledWith('F-9', { action: 'accept_skill' });
  });
```

- [ ] **Step 3: Run it to confirm it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`
Expected: FAIL — no element with text "Save as skill".

- [ ] **Step 4: Add the button**

In `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`, in the
`if (row.kind === 'skill_proposal')` branch, add a Save button *before* the
existing Dismiss button inside the `<div className="flex gap-1.5 flex-wrap">`:

```tsx
        <div className="flex gap-1.5 flex-wrap">
          <button
            type="button"
            aria-label="Save this proposal as a skill"
            className="text-[11px] font-semibold px-2.5 py-1 rounded border border-emerald-400/30 text-emerald-300 bg-emerald-400/10 hover:bg-emerald-400/20"
            onClick={() => onResolve(row.feedbackId, { action: 'accept_skill' })}
          >
            Save as skill
          </button>
          <button
            type="button"
            aria-label="Dismiss this skill proposal"
            className="text-[11px] font-semibold px-2.5 py-1 rounded border border-zinc-700 text-zinc-400 hover:bg-white/[0.02]"
            onClick={() => onResolve(row.feedbackId, { action: 'skip' })}
          >
            Dismiss
          </button>
        </div>
```

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`
Expected: PASS — all FeedbackCard tests (Save, Dismiss, clarification, doubt).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx
git commit -m "feat(gui): Save as skill button on skill proposals"
```

---

## Final Verification

- [ ] `cargo test -p vox-orchestrator -p vox-plugin-host -p vox-orchestrator-mcp --lib` — all green.
- [ ] `cargo clippy -p vox-orchestrator -p vox-plugin-host -p vox-orchestrator-mcp -- -D warnings` — clean.
- [ ] `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/NeedsYou` — green.
- [ ] `cargo fmt -p vox-orchestrator -p vox-plugin-host -p vox-orchestrator-mcp` (NEVER `cargo fmt --all` on Windows).
- [ ] Then: superpowers:finishing-a-development-branch.

---

## Self-Review Notes

- **Spec coverage:** meta payload (T1) ✓, AcceptSkill (T1) ✓, author_skill_md (T2) ✓, accept arm + propose wiring (T3) ✓, GUI button (T4) ✓, error cases (T3 Step 6 — missing meta returns error, item stays open) ✓.
- **Type consistency:** `author_skill_md(name, description, category, tags: &[String], members: &[String])` used identically in T2 test, T2 impl, and T3 accept arm. `FeedbackAction::AcceptSkill` (no fields) consistent in T1/T3/T4. `Candidate`/`DraftFrontmatter` field names match `vox-skill-discovery/src/candidate.rs`.
- **Known soft spots flagged for the implementer (resolve against real source during execution):** the MCP test state constructor (`test_state_with_ws_root`) and `state.workspace_root()` accessor are illustrative — grep the existing doubt-resolution test + crate for the real names. The simplified tempdir block in T3 Step 6 (fixed inner dir `skill`, install keys off parsed frontmatter name) is the path to take; it removes any need to expose a second kebab helper.
