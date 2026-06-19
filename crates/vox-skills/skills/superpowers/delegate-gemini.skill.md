---
name: delegate-gemini
description: Use to offload high-volume code generation or heavy refactoring to a sandboxed Gemini agent via the native agy delegation tools - you stay architect, agy does the typing. Worktree-isolated, auto-accepting, auto-logged to the handoff ledger.
---

# Delegate to Gemini (Antigravity `agy`)

**Announce at start:** "I'm using the delegate-gemini skill to offload implementation to agy."

You are the **architect**; `agy` (Gemini) is the **hands**. Delegate token-heavy generation; never delegate the thinking.

## Pre-flight
Call `vox_agy_doctor` first. If status != "ready", follow its `remediation` (install `agy`, add to PATH, complete the one-time interactive Google Sign-In) before delegating. We never store Google credentials.

Also call `vox_credentials_status` to see which inference providers are payable right now (OpenRouter is one of many). The agy entry in the `delegation` section of that response mirrors the doctor output.

## When to use
- Large, mechanical, or repetitive implementation you can specify with zero ambiguity.
- NOT for architecture, security-sensitive code, or anything you cannot precisely specify.

## Protocol (Brain → Hands → Auditor)
1. **Plan (Brain).** Write a deterministic spec: exact file paths, target structs/functions (confirm they exist with `rg` first — agy hallucinates APIs otherwise; ledger lesson B-6), and the exact change sequence.
2. **Delegate (Hands).** Call `vox_agy_delegate` with `task` = your spec. It runs `agy -p ... --dangerously-skip-permissions` inside an isolated worktree (`agy/<slug>`), auto-accepting all prompts, hard-killed at `timeout_secs`, retrying quota/timeout. Do NOT write the implementation yourself.
3. **Verify (Auditor).** Review the returned `diff` against your spec. Run repo gates (build, tests, arch-check) before integrating. Prove the effect, not the shape (ledger B-9).
4. **Integrate or iterate.** Good: merge/cherry-pick `agy/<slug>`; then set the ledger entry's `verdict`. Not good: re-delegate with corrections (hand-fix only trivial typos).

## Credentials & budget
- Call `vox_credentials_status` to see every payable provider (inference) and the agy delegation status in one view.
- `agy` uses **Antigravity credits** (OAuth, no stored key, balance not queryable headlessly). It is NOT billed in USD — delegation results carry `"billing": "antigravity-credits"`.
- The Gemini-direct inference path (completions, not agentic editing) uses `GEMINI_API_KEY` in Clavis — a separate egress.
- For full limitations and update triggers (antigravity-cli#78/#36), see `docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md`.

## Safety invariants (do not weaken)
- Auto-accept defeats agy's own `--sandbox` (antigravity-cli#36). The ONLY sandbox is the worktree jail the tool creates — never run agy against the live tree yourself.
- Every delegation is auto-logged. Close the loop by filling the verdict after review.

## Parallel fan-out
For 2+ independent, file-DISJOINT tasks, use `vox_agy_delegate_batch` instead of multiple sequential single-delegate calls.

**Example — 3 file-disjoint tasks at `max_concurrency: 3`:**
```json
{
  "tasks": [
    "Add pub fn parse_config(path: &Path) -> Result<Config> to crates/vox-config/src/lib.rs — no other files",
    "Add #[test] fn roundtrip_config() to crates/vox-config/src/tests.rs — no other files",
    "Update README.md §Configuration to document the new parse_config API — no other files"
  ],
  "max_concurrency": 3,
  "timeout_secs": 600
}
```

**Integration rule:** Review each worker's `diff` independently. For file-disjoint branches, cherry-pick or merge in any order. For any overlap the workers shouldn't have had (e.g. both touched `lib.rs`), resolve sequentially. Apply the **two-strike rule** (see `dispatching-parallel-agents`): if a worker fails twice, STOP and re-delegate with a corrected spec.
