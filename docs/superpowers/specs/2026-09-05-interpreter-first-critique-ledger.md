---
title: "Interpreter-first execution: critique ledger"
description: "Every finding from two rounds of parallel review against the source, with severity, verification status, and disposition — the audit trail behind revision 3 of the spec and plan."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Interpreter-first execution — critique ledger

Two rounds of parallel read-only review against the source, fifteen tracks in all.
Round 1 (seven tracks) produced revision 2. Round 2 (eight tracks) reviewed revision 2 and
produced this ledger and revision 3.

**Why this file exists.** A finding that is fixed silently teaches nobody, and a finding that
is dismissed silently gets rediscovered. Every item below carries a disposition. The ones
marked FALSE ALARM are as valuable as the bugs: they are the places a future reader will
suspect a problem, and they record that someone already checked.

**Verification legend.** **[V]** verified by running it on this machine · **[S]** verified by
reading the source · **[D]** documented measurement already in the repo · **[R]** reasoned,
not verified — treat as a hypothesis.

---

## 1. Defects that would have shipped broken

| # | Finding | Sev | Ver | Disposition |
|---|---|---|---|---|
| 1.1 | **`fs_resolve_allowed` denies every relative new-file path.** `Path::new("out.txt").parent()` is `Some("")`, not `None`, so `canonicalize("")` fails and the arm returns a *fatal denial* — under `developer_default()`, i.e. every ordinary local `vox run`, on the tier Task 7 makes the default. Breaks `fs.write("out.txt")`, `fs.mkdir("a/b/c")`, `fs.exists(dir)`, and at least four in-repo scripts. Task 5's tests only touch paths that already exist inside a tempdir, so nothing catches it. | CRITICAL | [S] | FIX — parent-walk to the nearest existing ancestor; add a `developer_default()` relative-write test; re-run Task 0's table *after* Task 5 |
| 1.2 | **`scripts/install-hooks.vox` and `scripts/setup.vox` fail under the interpreter today** with `AssertionFailed("called Option.unwrap() on a None value")`. `setup.vox` is what `setup-e2e.yml` runs. Both pass `vox check`, so a typecheck-only Task 0 would have green-lit the flip and broken CI and the pre-commit hook for every contributor. | CRITICAL | **[V]** | FIX — Task 0 must execute, not check; both scripts are Task 7 blockers |
| 1.3 | **`list.push` is O(n²) under the interpreter, O(n) natively.** `builtins.rs:292` does `v.to_vec()` — a full clone — and `VoxValue::Str` holds a `String`, so every element's string is deep-copied per push. `xs = xs.push(v)` is the idiomatic collection builder across `scripts/**`. The flip changes its *complexity class*, silently. The differential gate compares bytes and would pass it blind. | CRITICAL | **[V]** [S] | FIX — in-place path via `Scope::get_mut` + `Rc::make_mut` (the machinery exists; `env.rs:56` names `list.push` as its user) and `Rc<str>` for `VoxValue::Str`; prerequisite of Task 7, not a follow-up |
| 1.4 | **`fs.glob`'s prefix check is a sandbox escape.** `rsplit_once('/')` finds nothing in `C:\Windows\System32\config\*`, so `dir` falls back to `"."` — the job dir, an rw root — the check passes, and the **raw** pattern is globbed. `glob` treats `\` as a separator on Windows. Unix equivalent: `/tmp/job/*/../../etc/*`. Results are never filtered against the roots. | HIGH | [S] | FIX — split on both separators, reject `..`, and filter the *results*, not only the prefix |
| 1.5 | **Payload frame arithmetic rejected every payload** (round 1). postcard prefixes `Vec<u8>` with a varint *inside* the frame, so `read_frame(.., payload_bytes)` is short by 1–5 bytes for every payload including empty. | HIGH | [S] | FIXED in rev 2 (`+ 8`), plus a length-vs-claim check |
| 1.6 | **The allocator would have been a silent no-op.** `main.rs` has no `mod` declarations; `run()` is library code. `mod mem_limit;` in the bin plus `crate::mem_limit::arm()` in the lib compiles two copies of `USED`/`LIMIT` — the allocator reads one, `arm` writes the other. | HIGH | [S] | FIXED in rev 2 — module and `#[global_allocator]` both in `lib.rs` |
| 1.7 | **`std::process::exit` inside `alloc` can deadlock.** libc `exit` takes `__exit_funcs_lock` and runs atexit handlers; any handler that allocates re-enters over the ceiling. `abort()` is also wrong — it dies by signal, so the parent sees `None`, never 79. | HIGH | [S] | FIXED in rev 2 — `libc::_exit` / `TerminateProcess` |
| 1.8 | **`impl JobId` does not compile** — `JobId` is `pub type JobId = u64`, a type alias; an inherent impl on `u64` from another crate is E0390. | HIGH | [S] | FIXED in rev 2 — newtype |
| 1.9 | **The `tokio::select!` did not compile** — `child.wait()` holds `&mut child` while other arms call `child.kill()`. | HIGH | [S] | FIXED in rev 2 — `Done` enum, kill after the select |
| 1.10 | **Task 2 could not execute at all.** `vox-compiler` has no `vox-crypto` edge, `vox_crypto::hash` does not exist (only raw-byte facades), and the native `vox_hash_fast` is XXH3-128/32-hex while the plan asserted BLAKE3/64-hex — the "parity" test would have *introduced* an asymmetry. | HIGH | [S] | FIXED in rev 2 (deferred); rev 3 takes the edge under authorization and makes `vox-crypto` the SSOT for both tiers |
| 1.11 | **Two executor tests cannot pass.** `a_runaway_allocation…` needs ~7×10¹⁰ mallocs to reach 64 MiB live via the push loop (hours; `max_steps` is set high precisely so the step budget cannot rescue it). `one_peer_cannot_cancel…`'s 3 M-iteration loop is ~22 M steps, over the 10 M default — it exits 78 before printing `DONE`. | HIGH | [S] [R] | FIX — `s = s + s` doubling (26 iterations); thread `JobLimits::max_steps` into the child |

---

## 2. Sandbox holes — the "interpreter is the sandbox" claim

The claim in revision 1 was *"every side effect passes through `call_builtin_method`"*. That was
**false about the source**, not merely incomplete.

| # | Bypass | Sev | Ver | Disposition |
|---|---|---|---|---|
| 2.1 | **`import` reads and executes arbitrary host `.vox`** — `eval/mod.rs:377/392`, before `main`, before any gate. Parser's only constraint is `.ends_with(".vox")`. | CRITICAL | [S] | FIXED in rev 2 — gated in `resolve_local_file_import`, mutation-verified |
| 2.2 | **`process.exec` replaces the process image** (`CommandExt::exec`) — every in-process bound ceases to exist. `spawn_background` outlives the interpreter. | HIGH | [S] | FIXED in rev 2 — documented as shell access; `process` denied for `Sandboxed`; `grant_native` is the only path |
| 2.3 | **`process.register_exit_command` writes to a process-global static** whose signal handler runs the commands and hard-exits the *host daemon*, surviving the interpreter that armed it. | HIGH | [S] | FIXED in rev 2 — gated at queue time, moved onto `Interpreter` |
| 2.4 | **The `@versioned` snapshot** calls `repo.snapshot` directly at `mod.rs:571`, bypassing method dispatch. Named by the spec as one of four bypass paths — **and dropped from the plan without a word**. | HIGH | [S] | **OPEN in rev 2** → rev 3 must gate it or record an explicit exception |
| 2.5 | **`path.resolve` calls `std::fs::canonicalize`** from a namespace marked pure — a whole-disk existence oracle under `--caps ''`. | HIGH | [S] | FIXED in rev 2 — gated as an fs read |
| 2.6 | **The fs method list was wrong in both directions.** Two named methods do not exist (`append`, `rename`); six real ones were missing, including **`remove_dir_all`** and `read_bytes`. `fs:ro=<input>` would have permitted `fs.remove_dir_all("/")`. | CRITICAL | [S] | FIXED in rev 2 — verified table, guard at the arm head, unknown methods denied by default |
| 2.7 | **`caps: None` remained a bypass**, and the test named for it never constructed `None`. | HIGH | [S] | FIXED in rev 2 — `caps` non-optional |
| 2.8 | **Six in-process embedders run `Interpreter` ungated**, including the MCP dispatch that executes `.vox` tools for an LLM. The spec claims five "each receive an explicit set"; **no task does this**, and Task 3's grep for `.caps = Some(` is true and irrelevant — they never set caps at all. Post-plan they are *more* permissive than today. | HIGH | [S] | **OPEN in rev 2** → rev 3 sets an explicit set at all six sites |
| 2.9 | **The denial marker is forgeable, but not weaponizable.** `log.error` is `PURE` and writes attacker-controlled text to stderr, so the sentinel string *is* producible. Reaching exit 77 needs `process.exit`, which is gated and denied for `Sandboxed`; `assert` faults to 1 and `log` changes no exit code. The two-factor check holds — only an already-`Native` peer could forge a verdict. | LOW | [S] | DESIGN SMELL — a sentinel should not be producible by a `PURE` builtin. Move it to a channel the script cannot write (fd 3 or a status file) when convenient; not a rev-3 blocker |
| 2.11 | **Forged `__namespace__` is contained.** A script can build `{__namespace__: "fs"}` and it does reach real namespace dispatch (dict methods are skipped when `ns` is `Some`) — but the gate keys on the same string and `allows_path` still scopes it. Forging a `PURE` name reaches only ops that do no privileged I/O. | — | [S] | **VERIFIED NEGATIVE** — closed. Recorded because it is the obvious first attack |
| 2.12 | **`_Denied` / `_Panic` cannot be observed as values.** Both are converted at the single `call_builtin_method` return site before any `match`, `Option`/`Result` wrapping, list store, or closure capture. | — | [S] | **VERIFIED NEGATIVE** — closed |
| 2.13 | **No existence or timing oracle.** An existing-but-outside path and a nonexistent path both yield the same `CapabilityDenied`; denial is fatal, so a script cannot probe then read a clock. The TOCTOU the spec admits needs a concurrent racer, i.e. `process:allow`. | — | [S] | **VERIFIED NEGATIVE** — closed |
| 2.10 | **`db`/`repo` were gated for nothing.** Both are pure in-memory stores with zero I/O. A mesh script using a `table` would die with a denial while `@versioned` mutated ungated. | MED | [S] | FIXED in rev 2 — moved to `PURE` |

---

## 3. Concurrency, resources, and the process boundary

| # | Finding | Sev | Ver | Disposition |
|---|---|---|---|---|
| 3.1 | **A saturated node cannot answer `Cancel` — and this is on `main` today.** `endpoint.rs:176-217` holds the accept permit through execution. On a ≥64-thread host, 64 legitimate slow jobs wedge the accept loop for the full wall clock: no `Probe`, no `Cancel`. And there is **no read deadline after the handshake**, so one trusted peer opens 64 connections, sends nothing, and the node accepts nothing from anyone, indefinitely, at zero cost. The peer-scoped cancel map is correct and unreachable exactly when it matters. | CRITICAL | [S] | FIX — `drop(permit)` after the trust check + `REQUEST_TIMEOUT` on every post-handshake frame. **Separate commit against merged Phase 3**, not buried in this plan |
| 3.2 | **A dropped `oneshot` sender silently kills a healthy job.** `_ = cancel_rx` cannot distinguish `Ok(())` from `Err(RecvError)`. `JobId` is sender-assigned and nothing rejects reuse; `HashMap::insert` over an occupied key drops the first job's sender. Round 1's payload-hash collision, re-entering through id reuse. | HIGH | [S] | FIX — refutable pattern `Ok(()) = cancel_rx`, and refuse duplicate `(peer, job_id)` |
| 3.3 | **Up to 2 detached tasks and 2 pipe fds leak per job** on the cancel path — the one an attacker controls. `tokio::spawn` detaches; dropping the `JoinHandle` does not abort. | HIGH | [S] | FIX — abort-on-drop `Drain` newtype |
| 3.4 | **`read_capped`'s truncation flag is computed after the extend**, double-counting `n`. Reports truncation for any single read where `max/2 < n ≤ max`, *including an exact fit* — and then appends a marker that pushes the response over the cap it is named after. | HIGH | [S] | FIX — compare the pre-extend length; hermetic unit test with `max=10`, reads `[10]`, `[5,5]`, `[6]`, `[11]` |
| 3.5 | **No Windows process-group equivalent.** `process_group(0)` is Unix-only; on Windows `child.kill()` reaches only the direct child, so a `process:allow` job's grandchildren survive the wall clock *and* `Cancel`. The spec states the guarantee unconditionally. | HIGH | [S] | FIX — `win32job` (already a workspace dep, third-party, **no crate edge**) with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; do **not** `mem::forget` the job as `sandbox.rs` does — it must drop per job |
| 3.6 | **No concurrent-job cap** in revision 1 — 64 in-flight × 512 MiB reachable. | MED | [S] | FIXED in rev 2 — semaphore; rev 3 adds the missing test and a `with_slots` ctor |
| 3.7 | **Stack overflow escapes every bound.** `fn f() { f() }` aborts by signal → `status.code()` is `None` → renders as `"exit None:"`. A 200 k-nested-paren payload SIGSEGVs at *parse* time, before any limit is armed. | HIGH | [S] | FIXED in rev 2 — depth bound + parser nesting limit; rev 3 moves the counter to `apply_closure` (a guard in `eval_expr` *increases* native stack per recursion level, shrinking the headroom it protects) |
| 3.8 | **1 GiB payload cap** on the receiver's unbounded heap, sized for the bundle lane being deleted. | HIGH | [S] | FIXED in rev 2 — 16 MiB general, 4 MiB for `VoxScript` (matches `MailboxLimits`, which already reasons this way) |
| 3.9 | An escaped grandchild (`setsid()`) leaves an orphaned job dir; `TempDir::drop` swallows the error by design. | MED | [S] | FIX — explicit `close()` with a `tracing::warn!` |
| 3.10 | `QueueStats.pending_count` reports *running*, bounded by the semaphore — never demand. Phase 4 placement would read a metric whose range is `0..=N` regardless of load. | MED | [S] | FIX — add `max_concurrent` while PROTO is already bumping; it is the free moment |

### 2b. Availability — where the claim does *not* hold

The threat track's verdict: **the claim is true for confidentiality and integrity, false for
availability.** Every escape path (a) read outside the job dir, (b) network, (c) run a host
binary, (f) learn something it should not — is closed for a `Sandboxed` peer. Denial of
service is not.

| # | Finding | Sev | Ver | Disposition |
|---|---|---|---|---|
| 2.14 | **No disk quota on the job dir.** `fs:rw=<job dir>` plus no `max_disk_bytes` in `JobLimits`; the memory ceiling explicitly excludes disk. A 300 s job fills the partition backing `TMPDIR`. | HIGH | [S] | FIX — size-capped tmpfs for the job dir, or account bytes in the fs write/copy arms |
| 2.15 | **No inode cap** — cheaper than 2.14 and needs no bytes: `while true { fs.write(str(i), "") }` exhausts inodes on the backing filesystem, breaking **every other process on it** for the window. | HIGH | [S] | FIX — same mount, `nr_inodes=`; or a per-job file count |
| 2.16 | **The executor semaphore is global, not per-peer.** One trusted peer submits `n` never-ending jobs and every other peer gets "at ceiling, retry" for the wall clock. | MED-HIGH | [S] | FIX — per-`EndpointId` in-flight cap ahead of the global semaphore |
| 2.17 | **`regex` size limit left at the crate default** (10 MiB compiled program) and a compile is **one step**, so the step budget does not bound it. Unbounded locally, where there is no wall clock. | MED | [S] | FIX — a shared `RegexBuilder` with `size_limit(1<<20)` for the interpreter |
| 2.18 | **`JobId` reuse within one peer** cancels or orphans that peer's own jobs (self-inflicted; cross-peer is safe). | LOW | [S] | FIX — refuse a `Run` whose `(peer, JobId)` is already live (same fix as 3.2) |

**Spec consequence.** §3.2's threat-model paragraph must say plainly that the interpreter is
an *isolation* boundary, not a *resource-containment* one: it counts steps and heap, and does
not count disk, inodes, executor slots, or regex compile time. Availability against a
trusted-but-malicious peer depends on OS quotas this design does not build.

---

## 4. Cross-platform

| # | Finding | Sev | Ver | Disposition |
|---|---|---|---|---|
| 4.1 | **`Win32_System_IO` missing from the `windows-sys` features** — `WriteFile`'s signature references `System::IO::OVERLAPPED`, which is behind its own feature. Windows build break in `mem_limit.rs`. | HIGH | [S] | FIX — add the feature |
| 4.2 | **`unreachable_unchecked()` after `TerminateProcess` is UB** — the call is documented asynchronous and may return. | MED | [S] | FIX — `loop { spin_loop() }` |
| 4.3 | **A `,` in a Windows path breaks every mesh job.** Account names may contain commas, so `TEMP` can be `C:\Users\Smith, John\...`; `from_roots` then refuses the job dir and the plan's own test asserts that as correct behaviour. | MED | [S] | FIX — make `--caps` repeatable (`ArgAction::Append`); argv has no delimiter problem. Then drop the `,`/`=`/`\|` refusals entirely |
| 4.4 | **Windows paths in Vox string literals get escape-mangled.** The Vox lexer processes `\n`/`\t`/`\r`/`\0`; `C:\Users\tools\...` becomes TAB + `ools`. Machine-specific, latent, costs a day. | MED | [S] | FIX — a `vox_lit()` helper normalising `\` to `/`; keep one genuine `\` test for 1.4 |
| 4.5 | **NTSTATUS faults render as negative integers** — `STATUS_STACK_OVERFLOW` → `-1073741571`. Windows has no signals, so the `None` arm never fires there. | MED | [S] | FIX — an NTSTATUS arm with names |
| 4.6 | **`env_clear()` drops `LD_LIBRARY_PATH`** — a `vox` built in a Nix shell or against bundled `.so`s fails at exec. `remote_worker.rs::baseline_passthrough_env()` already has the curated list the spec says to reuse; the plan's code did not. | MED | [S] | FIX — reuse the curated list (defactored, ~13 names) |
| 4.7 | **The read-only `$HOME` trick is partial on Windows** — `dirs::home_dir()` uses `SHGetKnownFolderPath` and ignores `USERPROFILE`. | MED | [S] | FIX — state it honestly; the real boundary is that neither profile is under an `fs` root |
| 4.8 | `is_under` on verbatim/UNC/SUBST/8.3 paths | LOW | [S] | FALSE ALARM — canonicalize normalises both sides. Extend `component_eq` to `Prefix` for the UNC-casing gap |
| 4.9 | `split_once(':')` on `fs:rw=C:\...` | — | [S] | **FALSE ALARM** — splits at index 2; `&r[3..]` is offset-based, so drive letters and `=` in paths both survive |

---

## 5. Tests that cannot fail

| # | Test | Why it is a decoy | Ver | Disposition |
|---|---|---|---|---|
| 5.1 | **Golden 4 (`glob_and_listdir_order`)** | Cargo sets an integration test's CWD to the *package* root, so `fs.glob("examples/golden/*.vox")` matches nothing and `[] is [].sorted()` is `true` on both tiers regardless of `sort()`. Passes by hand from the repo root; inert in CI. **The exact failure this plan exists to fix, reintroduced.** | [S] | FIX — pin `current_dir(repo_root())` in the gate; assert non-emptiness in the golden |
| 5.2 | `glob_is_sorted_and_propagates_errors` | Same CWD bug, and never tests error propagation despite the name | [S] | FIX — hermetic tempdir, seeded out of order; assert both halves |
| 5.3 | `every_known_asymmetry_has_a_reason` | Asserts a string literal in the same file is non-empty. A lint wearing a test's clothes | [S] | REPLACE — a ratchet on list length and duplicate keys |
| 5.4 | `dispatch_runs_real_source_on_a_loopback_peer` | Body is three comments. Compiles, `#[ignore]`d, passes, counted by the tier report | [S] | REPLACE — narrower assertion needing no server |
| 5.5 | `a_forged_exit_77_is_not_reported_as_a_denial` | **Cannot run** — `process.exit` is under the `process` namespace, which `Sandboxed` does not grant, so the script is denied at the exit call and the marker *is* present | [S] | FIX — `grant_native`, which is also the only threat model where forgery matters |
| 5.6 | `a_new_trailing_field_is_not_readable_from_an_old_sender` | Dummy variant at index 0 makes it test a *discriminant* mismatch, not the trailing field | [S] | FIX — same variant index + a positive control |
| 5.7 | `every_fs_method_that_takes_a_path_is_scoped` | 19 deny-assertions, no allow-assertion: a resolver returning `None` unconditionally passes | [S] | FIX — positive control in the same loop |
| 5.8 | Task 7's `auto_mode_…` | A 5 s stopwatch is the only tier evidence; a warm script cache beats it | [S] | FIX — run with `PATH=""`; no cargo means only the interpreter can succeed |
| 5.9 | `one_peer_cannot_cancel_another_peers_job` | Races: if A finishes inside 300 ms the test is vacuous *and* the mutation does not reproduce | [S] | SPLIT — hermetic unit test for the ownership guard; keep the live one as a smoke with an elapsed assertion |
| 5.10 | `proto_is_two_and_limits_carry_every_bound` | `assert!(x > 0)` passes for a 1-byte ceiling | [S] | FIX — pin the actual defaults |
| 5.11 | Golden 5's native half | The script cache is keyed on the **source hash**, not the build profile, so a machine that ran it before `overflow-checks` flips keeps a wrapping binary forever | [S] | FIX — wipe `~/.vox/script-cache` per gate run |
| 5.12 | **The whole process boundary** | `env_clear`, read-only `HOME`, process group, wall-clock kill — the entire non-interpreter half of the claim — has **zero tests** | [S] | FIX — four tests supplied |
| 5.13 | Three of six prescribed mutations are too broad | Task 4 Step 5a disables the gate for every namespace, producing five files of red that mask the one signal | [S] | FIX — minimal mutations per guard |

---

## 6. Would never have run, or would have blown a gate

| # | Finding | Sev | Ver | Disposition |
|---|---|---|---|---|
| 6.1 | **The differential gate runs on no CI job.** `ci.yml:1124-1131` carries a hand-maintained *duplicate* of `pre_push.rs`'s nextest filter, and CI's main step passes no `--run-ignored`. Registering only in `pre_push.rs` means local-only. | HIGH | [S] | FIX — amend both; note the duplication as debt |
| 6.2 | **The gate's cost model was wrong.** `native.rs:20-23` explicitly discards `shared_target` (`let _ = shared_target;`) and uses a per-cache-entry target dir, so **every golden rebuilds all 764 crates**: 19 × ~275 s ≈ **87 min serial**. It is killed by nextest's `slow-timeout` (540 s ci) after ~2 goldens, before ever reaching `tier-budget-check` — which is also `continue-on-error: true`. | CRITICAL | [S] [D] | FIX — one generated crate with 19 `[[bin]]` targets (~6–8 min); nightly lane, not `--include-slow` |
| 6.3 | **`ssot-drift` fails on four sub-gates** if `vox wasm` is deleted without regenerating the contract chain. `vox ci command-sync` alone is not sufficient — it writes the generated markdown from an input it does not update. | HIGH | [S] | FIXED in rev 2 — ordered chain in Global Constraints |
| 6.4 | **`secrets-parity` hard-fails** unless `vox ci secrets-contracts` regenerates three files *first*. | HIGH | [S] | FIXED in rev 2 |
| 6.5 | **`check-links` fails** on two markdown links to deleted files. | MED | [S] | FIXED in rev 2 |
| 6.6 | **Two HTTP dispatch handlers spawn deleted commands** (`vox wasm run`, `--isolation wasm`). Runtime breakage no compiler catches. | HIGH | [S] | FIXED in rev 2 |
| 6.7 | **`KNOWN_TIER_ASYMMETRIES` was never connected to the gate** — different crate, no exclusion mechanism. Task 16's acceptance was "PASS **or** a list that matches", i.e. a gate plus a human promise. | CRITICAL | [S] | FIX — either force both decisions before Task 1b, or add an `EXPECT-TIER-ASYMMETRY` directive the gate honours *and which fails when the tiers start agreeing* |
| 6.8 | `interp_executor.rs` and `caps_spec.rs` ship `pub fn`s with **no same-file test** — violates the plan's own Global Constraint and trips `skeleton/untested-pub-api`, a per-file detector | MED | [S] | FIX — in-file `mod tests` for the two pure functions |

---

## 7. Operability and rollout

| # | Finding | Sev | Ver | Disposition |
|---|---|---|---|---|
| 7.1 | **The PROTO mismatch message is thrown away.** `protocol.rs:145-153` already carries good text, but `directory.rs:48-63`'s `_ => None` makes a v1 peer **indistinguishable from a switched-off machine**. Symptom: "the mesh does nothing", never "upgrade BLAPTOP04". | HIGH | [S] | FIX — keep the response and warn; add `vox mesh probe` (~20 lines over `directory()`) |
| 7.2 | **`VOX_WEB_RUN_MODE=script` already is the global revert** and Task 7 preserves it — but the plan mentions none of the three escape hatches. The name says "web"; nobody will guess it. | MED | [S] | FIX — document all three scopes; do not invent a new key |
| 7.3 | **`vox mesh join` silently becomes a code-execution server.** Task 10 swaps the executor and changes nothing else — same banner, no prompt, no log. | HIGH | [S] | FIX — banner text stating what the command now permits |
| 7.4 | **Zero observability.** `vox-mesh-transport` has no `tracing::info!` at all; `run.rs`'s two are on the native path the flip routes past. Phase 4's `PlacementRecord` wants data this executor computes and discards. | HIGH | [S] | FIX — one `tracing::info!` per job; two lines total |
| 7.5 | **`setup-e2e.yml` silently stops testing what it exists to test** — `--features script-execution` becomes decorative once `setup.vox` routes to the interpreter. | MED | [S] | FIX — pin `--mode script` in those three lines |
| 7.6 | Every new error message rewritten with the capability held, the flag to grant it, and a copy-pasteable command. `--max-depth` was advised but never defined. | MED | [S] | FIX — messages replaced; add the flag or drop the advice |
| 7.7 | No `vox doctor` rows for the execution tier; the plan's only change tombstones a deleted feature as permanently green | MED | [S] | FIX — delete that row; add five real ones incl. "executor binary accepts `--caps`" |
| 7.8 | The stale-PATH-binary concern is **moot at the hook**: `lefthook.yml:18` uses `cargo run`, not a PATH `vox` | — | [S] | **FALSE ALARM** — say so in the commit rather than building a version guard |

---

## 8. Coherence

Seven orphaned spec clauses, four partial, two unjustified plan steps, ten stale citations,
four convention violations. The full traceability tables are in the round-2 coherence report;
the load-bearing items are 2.4, 2.8, and:

| # | Finding | Disposition |
|---|---|---|
| 8.1 | §3.3 says the asymmetry list "must be empty"; §4 says "empty **or** carry a reason". The plan ships three entries, one (`log.*`) authorised by neither | FIX — one rule, two authorised entries |
| 8.2 | `where-things-live.md` rows listed in a Files block, written by no step; AGENTS.md requires them in the same PR | FIX — four rows |
| 8.3 | `isolation.md` and ADR-048 have no `category` specified; ADR template unpinned | FIX — `Language Reference` / `Architecture Decisions (ADRs)`; follow ADR-047's shape |
| 8.4 | Ten stale citations, two paths that do not exist, two symbol names that do not exist | FIX — all corrected and re-verified [V] |
| 8.5 | Spec claim "nine of eleven goldens expect a single `ok`" | **WRONG** [V] — 79 goldens, 11 with EXPECT, **4** literal `ok` |
| 8.6 | Close-code inventory inverted | **WRONG** [V] — 4001/4002/4004 taken; **4003 is free** |
| 8.7 | Plan says "17 tasks", has 18 sections | FIX |
| 8.8 | Terminology drift: script-shaped / interpreter-shaped / service-shaped; stderr marker / denial marker | FIX — one term each |

---

## 9. Verified false alarms

Recorded so they are not re-investigated.

| Suspicion | Reality | Ver |
|---|---|---|
| A script can forge a namespace and bypass the gate | Dispatch is reached, the gate is not bypassed — same key | [S] |
| A denial can be caught as a value | Converted at the one call site before any binding | [S] |
| Denied-vs-missing is an existence oracle | Same error, and denial is fatal | [S] |
| `Child::wait` is not cancel-safe | Documented cancel-safe; memoises into `FusedChild::Done` | [S] |
| `TempDir` dropped before the child dies | Correct by declaration order on every path | [S] |
| `SemaphorePermit` does not outlive the early returns | `let Ok(_slot)` is a **named** binding; a bare `_` would be the bug | [S] |
| `MutexGuard` held across an await | A **compile error** here — `execute` returns `Pin<Box<dyn Future + Send>>`. The `std::sync::Mutex` is load-bearing; do not "upgrade" it | [S] |
| Windows drive letters break the caps grammar | `split_once(':')` at index 2; `&r[3..]` is offset-based | [S] |
| `JobLimits` on the wire needs a PROTO bump | Receiver-local, no `Serialize` | [S] |
| `_Denied` swallowed by a `_ =>` arm | One production call site, converted immediately | [S] |
| stdout unflushed before the fault in `EXPECT-EXIT` goldens | `Stdout` is a `LineWriter` unconditionally | [S] |
| macOS needs elevation for the mesh | Application firewall is **disabled**; no `vox` entry | **[V]** |
| serde_json key-sorting is a workspace-wide problem | `preserve_order` is already on via `workspace-hack`; only the **standalone generated crate** diverges | **[V]** |

---

## 10. Decisions taken

| Decision | Basis | Call |
|---|---|---|
| **Object iteration order** | The generated crate is standalone (`[workspace]`, `serde_json = "1"`); the workspace already has `preserve_order` | **Insertion order.** One line in the generated manifest. Zero workspace blast radius |
| **`vox-compiler → vox-crypto` edge** | L2→L0, downward; `vox-actor-runtime` hashing directly already violates AGENTS.md §Cryptography | **Take it**, and make `vox-crypto` the SSOT for both tiers' hash/id builtins. Fixes an existing policy violation |
| **Where the accept-loop fix lands** | It is a defect on `main`, not introduced here | **Separate commit** against merged Phase 3 |
| **Whether to keep a permanently-red golden** | A red gate teaches contributors to ignore it | **No.** Decisions forced before Task 1b; `crypto_hash_parity.vox` ships with the PR that takes the edge |

---

## 11. What this changes about the shape of the work

The plan is 18 tasks on one branch — one CodeRabbit review for ~6 000 lines including
regenerated contracts. The recommended split, by revertability:

| PR | Tasks | Boundary rationale |
|---|---|---|
| 1 | 0, 1, 1b | No behaviour change; ships the gate and its corpus |
| 2 | 2 (+ `list.push`) | Both tiers' display, overflow, argv, glob order, and the O(n²) fix. PR 1's goldens are its acceptance test |
| 3 | 3, 4, 5, 6 + `isolation.md` | Additive: no `--caps` ⇒ `developer_default()` ⇒ today's behaviour |
| 4 | **7 alone** | One file, one predicate. **The git point of no return** — one `git revert` |
| 5 | 8, 9, 10, 11 | **The fleet point of no return**: `git revert` restores code, not the machines already upgraded |
| 6 | 12, 13, 14, 15 | Mechanical, contract-heavy, low semantic risk, huge diff |

Task 16 is a verification pass, not a PR. Docs move into the PR that creates each surface.
