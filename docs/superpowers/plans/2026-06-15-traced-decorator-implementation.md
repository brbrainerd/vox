# `@traced` (TRACE-D) — Full Implementation Handoff

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make `@traced` a real decorator that emits a tracing span around the decorated function/handler/activity, seeded from the trace-id context that already propagates through the runtime.

**Status:** Spec + plan. Not started. The `is_traced` flag is pre-wired AST→HIR but **inert** (always `false`, read nowhere); no spans are emitted anywhere in Vox today.

**Architecture:** Add the missing front-end (`@traced` lexer token + parser flip of the existing `is_traced` flag), propagate the flag into `HirFn` (it currently lives only on `HirAgentHandler`), and emit a real `tracing` span in Rust codegen (and optionally the interpreter) when the flag is set — attaching the existing `TraceContext` trace-id as a span field so the propagated trace finally has spans hanging off it.

**Tech stack:** `logos` lexer, recursive-descent parser, HIR lowering, `vox-codegen` (Rust emit), `vox-telemetry` (`TraceContext`), the `tracing` crate.

---

## Why this is worth building (verdict: USEFUL GAP, not obviated)

Verified by read-only audit 2026-06-15:

- **Nothing in Vox emits a span per function/handler/activity/endpoint** — generated and interpreted code alike. Grep across `vox-codegen`, `vox-codegen-ts`, `vox-actor-runtime`, `vox-workflow-runtime` for `#[instrument]` / `info_span!` / `tracing::span!` / `opentelemetry` returns **nothing**. The only codegen `tracing::` use is log-line emission for the Vox `log`/`debug` builtin (`crates/vox-codegen/src/codegen_rust/emit/method_emit.rs:635`), not spans.
- **The trace *context* exists but has no spans to hang off.** `crates/vox-telemetry/src/span.rs:11-69` defines `TraceContext { trace_id, task_id, parent_task_id, span_depth }`, threaded task-locally via `TRACE_CTX.scope(...)`. It propagates a distributed trace-id (consumed only in `crates/vox-actor-runtime/src/llm/chat.rs` for LLM egress) — but never opens/closes a span. `vox-telemetry` is an event facade (`record_event!`), not a span API.
- **Durable workflows are journaled, not span-traced** (`vox-workflow-runtime/src/journal/`), so even the place spans are most expected has none.
- **`is_traced` is dead plumbing waiting for exactly this.** The field exists on `FnDecl` (`crates/vox-ast/src/decl/fundecl.rs:61`), several `logic.rs` decls, and `HirAgentHandler` (`crates/vox-compiler/src/hir/nodes/decl.rs:681`), and is copied AST→HIR for agent handlers (`crates/vox-compiler/src/hir/lower/decl.rs:459`) — but **every construction site sets it `false`** and **no codegen branch reads it**. `Decl::set_decorators(...)` (the only writer) has **zero parser call sites**.
- **`@traced` does not parse:** no `AtTraced` token in `crates/vox-compiler/src/lexer/token.rs`; not in `LSP_DECORATOR_DOCS`.

So `@traced` adds a capability nothing else provides, and the scaffolding (trace-context + `is_traced` field) was clearly designed to host it. **Proceed.**

---

## Design decisions (resolve before P3; defaults given)

1. **Span backend — DEFAULT: the `tracing` crate (`#[tracing::instrument]`).** It is already a workspace dependency and the codebase's logging substrate. A future OTel bridge can subscribe to `tracing` spans without changing emitted code. Do **not** invent a new span API in `vox-telemetry` for v1.
2. **Span name — DEFAULT: the Vox function name** (`fn greet` → span `greet`). Optionally allow `@traced("custom_name")` later (out of scope for v1; note that the arg form must be lexed differently — see Risks).
3. **Trace-id linkage — DEFAULT: attach `TRACE_CTX`'s `trace_id` as a span field** (`trace_id = %ctx.trace_id`) when a context is active, so the existing propagated id finally anchors spans. Read it via the `vox-telemetry` task-local; skip the field if no context.
4. **Scope — DEFAULT: plain functions first (P1–P4).** Endpoints/activities/workflows (which have their own `is_traced` sites) are P8, a follow-up, to keep each PR small.
5. **Interpreter — DEFAULT: emit a lightweight span too (P7)**, so `vox run --mode interp` behaves consistently; acceptable to defer if the interpreter has no span sink.

---

## File map

| File | Change |
|---|---|
| `crates/vox-compiler/src/lexer/token.rs` | add `#[token("@traced")] AtTraced,` + `Display` arm |
| `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` | declare `is_traced` local, add `Token::AtTraced` arm, pass `is_traced` to the `FnDecl` construction (replace the hard-coded `is_traced: false`) |
| `crates/vox-compiler/src/hir/nodes/decl.rs` | add `pub is_traced: bool` to `HirFn` (model: `HirAgentHandler.is_traced`) |
| `crates/vox-compiler/src/hir/lower/decl.rs` | in `lower_fn`, set `is_traced: f.is_traced` |
| `crates/vox-codegen/src/codegen_rust/emit/...` (the fn-emit site) | emit `#[tracing::instrument(...)]` (or an `info_span!` guard) when `hir_fn.is_traced` |
| `crates/vox-compiler/src/lsp/language_surface.rs` (or wherever `LSP_DECORATOR_DOCS` lives) | register `@traced` |
| `docs/src/reference/ref-decorators.md` | document `@traced` as implemented |
| tests | parser, lowering, codegen-golden, behavioral (P1–P7 each add their own) |

---

## Phase 1 — Lexer token

**Files:** `crates/vox-compiler/src/lexer/token.rs`

- [ ] **Step 1 (test):** add a lexer test asserting `@traced` lexes to one token.

```rust
#[test]
fn lexes_at_traced() {
    let toks = lex("@traced fn f() to int { return 1 }");
    assert!(toks.iter().any(|t| matches!(t, Token::AtTraced)), "@traced must lex to AtTraced");
}
```

- [ ] **Step 2 (run, fail):** `cargo test -p vox-compiler lexes_at_traced` → FAIL (`AtTraced` undefined).
- [ ] **Step 3 (impl):** beside `AtPure` (token.rs:193) add:

```rust
    #[token("@traced")]
    AtTraced,
```
and in the `Display` impl beside `Token::AtPure => write!(f, "@pure")` (token.rs:603) add:
```rust
            Token::AtTraced => write!(f, "@traced"),
```

- [ ] **Step 4 (run, pass).** `cargo test -p vox-compiler lexes_at_traced` → PASS.
- [ ] **Step 5 (commit):** `feat(lexer): add @traced token (TRACE-D P1)`

---

## Phase 2 — Parser wiring (plain functions)

**Files:** `crates/vox-compiler/src/parser/descent/decl/head_fn.rs`

The decorator loop already handles `Token::AtPure => { self.advance(); is_pure = true; }` (head_fn.rs:99) and `Token::AtDeprecated` (115). The flags are declared near head_fn.rs:19-23 (`let mut is_pure = false;` etc.); the `FnDecl` is built around head_fn.rs:1087-1123 with a hard-coded `is_traced: false` (line ~1123).

- [ ] **Step 1 (test):** in the parser test module (or `semcov_struct_pipeline_tests` pattern), assert `@traced fn` sets the AST flag.

```rust
#[test]
fn at_traced_sets_fndecl_is_traced() {
    let m = parse(lex("@traced\nfn f() to int { return 1 }")).expect("parse");
    let f = m.declarations.iter().find_map(|d| match d {
        crate::ast::decl::Decl::Function(f) => Some(f),
        _ => None,
    }).expect("fn decl");
    assert!(f.is_traced, "@traced must set FnDecl.is_traced");
}
```

- [ ] **Step 2 (run, fail).**
- [ ] **Step 3 (impl):**
  - Near head_fn.rs:23 add `let mut is_traced = false;`.
  - In the decorator loop beside the `AtPure` arm add:
    ```rust
                    Token::AtTraced => {
                        self.advance();
                        is_traced = true;
                    }
    ```
  - At the `FnDecl { ... }` construction (head_fn.rs:~1123) replace `is_traced: false,` with `is_traced,`.

- [ ] **Step 4 (run, pass).**
- [ ] **Step 5 (commit):** `feat(parser): honor @traced on plain functions (TRACE-D P2)`

> NOTE: `head.rs:328`, `mid.rs:955/988/1037`, `descent/mod.rs:353` also build decls with `is_traced: false` (endpoints/activities/workflows/etc.). Leave those `false` for now; they're P8. Do NOT route through `Decl::set_decorators` (it has zero callers and is orphaned plumbing — wire directly like `is_pure`).

---

## Phase 3 — HIR field + lowering

**Files:** `crates/vox-compiler/src/hir/nodes/decl.rs`, `crates/vox-compiler/src/hir/lower/decl.rs`

`HirFn` (decl.rs:328) has `is_pure` (349) and `is_deprecated` (386) but **no** `is_traced`. `HirAgentHandler.is_traced` (decl.rs:681) is the model.

- [ ] **Step 1 (test):** in `crates/vox-compiler/src/semcov_struct_pipeline_tests.rs` (the structural module) add:

```rust
#[test]
fn traced_marker_survives_lowering() {
    // Catches: @traced parsed but dropped during lowering (no HirFn.is_traced).
    let hir = lower("@traced\nfn f() to int { return 1 }");
    let f = hir.functions.iter().find(|f| f.name == "f").expect("fn f");
    assert!(f.is_traced, "@traced must survive lowering into HirFn.is_traced");
}
```

- [ ] **Step 2 (run, fail):** FAIL — `HirFn` has no `is_traced` field (won't compile).
- [ ] **Step 3 (impl):**
  - In `HirFn` (decl.rs:328) add `pub is_traced: bool,` beside `pub is_deprecated: bool,` (386). Add `#[serde(default)]` if the struct's other bools use it (match the neighbours).
  - In `lower_fn` (hir/lower/decl.rs — the `HirFn { ... }` builder) add `is_traced: f.is_traced,`.
  - **Compiler will flag every other `HirFn { ... }` literal** that now lacks the field — add `is_traced: false` to each (test fixtures included; the structural-test `lower_when_view` etc. construct HIR indirectly so are unaffected). This is the same mechanical fan-out the `auth` field caused earlier; expect a handful.

- [ ] **Step 4 (run, pass).**
- [ ] **Step 5 (commit):** `feat(hir): propagate is_traced into HirFn (TRACE-D P3)`

---

## Phase 4 — Rust codegen emits a span

**Files:** the function-emit site in `crates/vox-codegen/src/codegen_rust/emit/` (find the `fn`-emitting function; `method_emit.rs` already references `tracing::` at :635, confirming `tracing` is in scope for generated code).

- [ ] **Step 1 (test):** a codegen test asserting the emitted Rust for a `@traced fn` contains an instrument attribute.

```rust
#[test]
fn traced_fn_emits_instrument_attr() {
    let rust = emit_rust("@traced\nfn greet() to int { return 1 }"); // use the crate's existing emit helper
    assert!(
        rust.contains("tracing::instrument") || rust.contains("info_span!"),
        "a @traced fn must emit a span; got:\n{rust}"
    );
    assert!(rust.contains("greet"), "span must reference the fn name");
}
```

- [ ] **Step 2 (run, fail).**
- [ ] **Step 3 (impl):** at the fn-emit site, when `hir_fn.is_traced`, prepend the attribute to the generated fn:

```rust
    if hir_fn.is_traced {
        // Seed the span with the propagated trace-id when a context is active.
        out.push_str(&format!(
            "#[tracing::instrument(skip_all, name = \"{}\", fields(trace_id = tracing::field::Empty))]\n",
            hir_fn.name
        ));
    }
```

and emit, at the top of the generated fn body, a best-effort record of the trace-id from the task-local context (guarded so it compiles even when no context):

```rust
    if hir_fn.is_traced {
        body.push_str(
            "if let Some(__tc) = vox_telemetry::current_trace_context() { \
             tracing::Span::current().record(\"trace_id\", tracing::field::display(&__tc.trace_id)); }\n",
        );
    }
```

> Confirm the accessor name: the task-local is `TRACE_CTX` in `crates/vox-telemetry/src/span.rs`. If there is no public `current_trace_context()` getter, add a thin one to `vox-telemetry` (`pub fn current_trace_context() -> Option<TraceContext>`) that reads the task-local — a 3-line addition — and use it. This is the only `vox-telemetry` change.

- [ ] **Step 4 (run, pass).** Also add a golden: compile a `@traced` fn end-to-end and confirm it builds (the generated crate must still `cargo build`).
- [ ] **Step 5 (commit):** `feat(codegen): emit tracing span for @traced fns (TRACE-D P4)`

---

## Phase 5 — LSP + docs

**Files:** `LSP_DECORATOR_DOCS` (in `language_surface.rs`), `docs/src/reference/ref-decorators.md`

- [ ] **Step 1:** register `@traced` in `LSP_DECORATOR_DOCS` with hover text ("Emits a tracing span around the function, seeded with the active trace-id.").
- [ ] **Step 2:** add a `### \`@traced\`` section to `ref-decorators.md` under an Observability heading, with a real `@traced fn` usage example and the note that it emits a `tracing` span (OTel via a `tracing` subscriber). Keep YAML frontmatter intact.
- [ ] **Step 3 (commit):** `docs(decorators): document @traced (TRACE-D P5)`

---

## Phase 6 — Behavioral test (the observable proof)

**Files:** `crates/vox-cli/tests/behavioral_stdout_interp.rs` (or a sibling tracing-capture test)

- [ ] **Step 1:** assert that running a `@traced` program actually produces a span. The cleanest observable check: set `RUST_LOG`/a test subscriber and confirm a span with the fn name is entered. If the interpreter path (P7) is deferred, gate this on the compiled path or assert at the codegen level (P4) instead, and mark this test `#[ignore]` with a reason until P7 lands.
- [ ] **Step 2 (commit):** `test(traced): behavioral span assertion (TRACE-D P6)`

---

## Phase 7 — Interpreter (optional, consistency)

**Files:** the interpreter's call site (`crates/vox-compiler/src/eval/...` — `Interpreter::call`).

- [ ] When invoking a function whose `HirFn.is_traced`, open an `info_span!(name, trace_id = ...)` for the duration of the call. Add a unit/behavioral test capturing the span. Commit `feat(interp): span for @traced fns (TRACE-D P7)`.

---

## Phase 8 — Extend to endpoints / activities / workflows (follow-up)

- [ ] Repeat P2–P4 for the other `is_traced` construction sites (`head.rs:328`, `mid.rs:955/988/1037`, `descent/mod.rs:353`) and their HIR nodes (endpoint fns, activities, workflows — workflows especially benefit, pairing the span with the journal). One PR per decl kind. Activities/workflows should attach the span to the journal entry's trace-id for replay correlation.

---

## Risks & open questions

- **`@traced("name")` arg form:** the `logos` `#[token("@traced")]` matches only the bare keyword. An argument form needs `@traced` + `(` + string handled in the parser loop (like other arg-bearing decorators), not the lexer. Out of scope for v1; if added later, mirror how `@auth(...)`-style decorators parse args. (Note the parallel `@deprecated("reason")` gap — same shape; consider solving both together.)
- **HIR field fan-out (P3):** adding `is_traced` to `HirFn` breaks every `HirFn { .. }` literal lacking it (compile errors). Mechanical but touches test fixtures — budget for it.
- **`set_decorators` is orphaned** — do not resurrect it; wire flags directly in the parser as the existing `is_pure`/`is_deprecated` do.
- **OTel vs `tracing`:** v1 emits `tracing` spans only. A real OpenTelemetry exporter is a separate, larger initiative (a `tracing-opentelemetry` subscriber wired into `vox-telemetry`); `@traced` is forward-compatible with it (no emitted-code change needed).
- **`current_trace_context()` accessor:** if absent in `vox-telemetry`, add the 3-line getter (the only telemetry change). Verify the task-local symbol name (`TRACE_CTX`) before referencing it in generated code.

## Acceptance criteria

- `@traced fn f() ...` parses, lowers (`HirFn.is_traced == true`), and the generated Rust contains a `tracing` span named `f` that records the active `trace_id`.
- A `@traced` program still compiles and runs; a span is observable (P6).
- `@traced` appears in LSP hover + `ref-decorators.md`.
- No regression: the existing structural/semcov suites stay green; the `is_traced` HIR fan-out is fully resolved.
