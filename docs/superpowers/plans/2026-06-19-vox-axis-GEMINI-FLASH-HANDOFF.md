# Vox Axis Rebrand — Gemini Flash Handoff Prompt (2026-06-19)

This is the copy-paste brief for **Gemini Flash 3.5 in Antigravity**. It is committed
so the runner can read it and every doc it references.

**Preconditions (human, before pasting):**
1. **Phase A is done in Claude Code** — the Axis icon set under `crates/vox-gui/icons/`
   is committed. **Phase D (the React/asset/token surface) may be running in parallel
   in Claude Code** — that's fine: Flash's Phase B touches disjoint files and does not
   depend on it. Flash must never touch Phase-D files (see invariant #6).
2. The runner has this branch checked out. If the runner is **remote/cloud**, push the
   branch first so its clone has the committed spec, plan, and icons.

---

## ── COPY-PASTE BELOW THIS LINE ──

You are rebranding the Vox GUI to **"Axis"** (full brand "Vox Axis") for the Vox repo.
All design and step-by-step detail is committed in this checkout. Read it from disk —
do not rely on this message for detail.

### Read first (in this order)
1. `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md` — your own operating limits.
2. `docs/superpowers/specs/2026-06-19-vox-axis-rebrand-design.md` — the design SSOT. Read §2 (scope / what NOT to change) before any code.
3. `docs/superpowers/plans/2026-06-19-vox-axis-rebrand.md` — the plan. **Execute PHASE B only** — tasks **B1, B4, B5, B6** (Phases A + D are already done in Claude Code). Finish with PHASE C (emit the handback block).

### Operating rules (the plan repeats these per task — follow exactly)
- Each task is **atomic + green + committed**. A kill between tasks must leave a compiling, tested tree.
- **Verify before use:** every Step-1 `rg`/read is a BLOCKING gate. Run it, paste the output; if reality differs from the plan, **STOP and report** — do not guess or "fix" the design.
- **Two-strike circuit breaker:** a step fails twice → STOP and report; do not thrash.
- Run the plan's **Mandatory pre-flight** block first. If `crates/vox-gui/icons` is not already rebranded, **STOP** (Phase A was skipped).
- `[PARALLEL-SAFE]` tasks may run in parallel subagents only if they touch disjoint files; never two subagents on one file. `[SEQUENTIAL]` tasks share a file — one at a time. See the plan's task-split table (run order: B1 ∥ B4 → B2 → B3 → B5; B6 anytime).

### Non-negotiable invariants (brand-layer only — do NOT regress)
1. **Do NOT change** `productName` ("Vox") or `identifier` ("org.vox-foundation.gui") in `tauri.conf.json` — only the window `title` → "Axis".
2. **Do NOT rename** the `vox-gui` crate, the `vox` binary, `vox-gui.exe`, or any Rust/TS code identifier. No import/API churn.
3. `vox axis` is a clap **`visible_alias`** of the existing `Gui` variant — not a new command/dispatch.
4. Display text = "Vox Axis"/"Axis"; identifiers = `axis`/`VoxAxis`.
5. Do NOT touch the gamification "Imperator" rank titles — unrelated to branding.
6. **Do NOT touch the front-end brand layer** — `Sidebar.tsx`, `components/brand/AxisMark.tsx`, `tokens/*.json`, `src/styles/tokens.generated.*`, `public/favicon.svg`, `index.html`. Claude built and committed these (Phase D); they are out of your scope. If a test references `AxisMark` or a brand token and it's missing, **STOP** — Phase D wasn't run, do not recreate it.

### Verification ritual (per task, before you commit)
- **GUI TS:** from `crates/vox-gui/ui` → `npx vitest run <path>` then `npx tsc --noEmit`. First line of every new component test: `// @vitest-environment jsdom`.
- **vox-cli Rust:** `cargo test -p vox-cli --features gui --test <name>` → `cargo clippy -p vox-cli --features gui -- -D warnings` → `cargo fmt -p vox-cli`.
- **Docs (Task B6):** `cargo run -p vox-doc-pipeline -- --lint-only` (frontmatter gate; `category` must be exactly `"Contributors"`).
- **Do NOT run `cargo clippy -p vox-gui`** — no task changes its Rust (title is a JSON value, marks are TS), and `--all-targets` breaks on the Tauri build script. Never `cargo fmt --all` (Windows arg-limit).
- A task is done only when its tests are green **and** committed.

### Working location
Use a dedicated git worktree off this branch for isolation if other sessions are active (the repo uses `.claude/worktrees/`).

### When done
Execute **PHASE C** of the plan: confirm the whole tree is green, then emit the
`## VOX-AXIS HANDBACK` markdown block (the plan gives the exact template) as your final
message. Do **NOT** edit `docs/superpowers/antigravity-handoff-ledger.md` yourself — the
ledger is updated in Claude Code from your handback block.

## ── COPY-PASTE ABOVE THIS LINE ──

---

### Notes for the human (not part of the prompt)
- **Run Phase A in Claude Code first** (generate + commit the Axis icons). Then paste the block above into Antigravity.
- If the runner is **remote/cloud**, push the branch (with the committed icons) before pasting.
- When Flash returns the `VOX-AXIS HANDBACK` block, paste it back into Claude Code; that triggers the ledger append (AGH-0010) + code-review, per the plan's "ledger append protocol".
- Suggested cadence: this is one small phase — hand off Phase B whole; it's low-risk and proves the loop.
