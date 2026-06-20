# Vox-Native Frontend SSOT — Sub-project B (reactive `vox://` stream primitive) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `on stream(channel) as binding: { body }` reactive-component member that lets `.vox` subscribe to a named push event channel, lowering onto the existing effect/cleanup substrate and a generated transport-neutral `vox-channel.ts` runtime, with channels declared in a `contracts/channels.v1.yaml` SSOT.

**Architecture:** The primitive is a new arm in the `on`-family member enums (`ReactiveMemberDecl::OnStream` → `HirReactiveMember::OnStream` → `BehaviorNode::StreamSub`). It is parsed contextually (`on` + ident `stream` + `(name)` + ident `as` + ident + block) so no lexer/token change is needed. On `Target::TypeScript` it emits a `useEffect` that calls a generated `voxChannel.subscribe(name, cb)` runtime — never `@tauri-apps` directly — whose transport is resolved lazily and guarded (Tauri impl + dev-mock), with contract-declared polling fallback. A workspace test enforces contract↔`transport.ts` parity.

**Tech Stack:** Rust (`vox-ast`, `vox-compiler`, `vox-codegen`, `vox-codegen-ts`), the `parse(lex(src))` → `lower_module` test pattern, YAML contract (`serde_yaml`), generated TypeScript runtime.

**Spec:** `docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-subproject-b-design.md`.

**Execution model (Claude Sonnet 4.6 in Claude Code):** TDD mandatory — failing test first, observed-output verification before any "done". Tasks are mostly sequential (each compiler-layer variant must exist before the next layer compiles). Tasks 1→2→3 are a strict chain (type must exist downstream); Task 4 (contract+loader) is parallel-safe with 1–3; Tasks 5–7 depend on both chains.

**Parallelism map:**
- Task 1 `[SEQUENTIAL base]` — AST node + parser.
- Task 2 `[SEQUENTIAL after 1]` — HIR node + AST→HIR lower + compiler match sites.
- Task 3 `[SEQUENTIAL after 2]` — WebIR node + lower.
- Task 4 `[PARALLEL-SAFE]` — contract YAML + loader + parity test (touches only `contracts/` + `vox-codegen-ts` loader; independent of 1–3).
- Task 5 `[SEQUENTIAL after 4]` — channel runtime emitter (needs the loader/types).
- Task 6 `[SEQUENTIAL after 3 and 5]` — emit `on stream` → `useEffect` + unknown-channel diagnostic + passthrough match sites.
- Task 7 `[SEQUENTIAL after 6]` — end-to-end golden + one ledger surface flipped + DoD green.

---

## File Structure

| File | New/Modify | Responsibility |
|---|---|---|
| `crates/vox-ast/src/decl/ui.rs` | Modify | Add `ReactiveMemberDecl::OnStream(OnStreamDecl)` + `OnStreamDecl` struct. |
| `crates/vox-compiler/src/parser/descent/decl/head_component.rs` | Modify | Parse `on stream(name) as bind: <block>` (component + module scope). |
| `crates/vox-compiler/src/fmt/printer.rs` | Modify | Format the new member (exhaustive match). |
| `crates/vox-compiler/src/parser/with_registry.rs` | Modify | Exhaustive-match passthrough. |
| `crates/vox-compiler/src/hir/nodes/decl.rs` | Modify | Add `HirReactiveMember::OnStream(HirOnStream)` + `HirOnStream`. |
| `crates/vox-compiler/src/hir/lower/decl.rs` | Modify | AST→HIR lowering arm. |
| `crates/vox-compiler/src/hir/lower/mod.rs`, `hir/db_op_walk.rs`, `typeck/{mod,checker/mod,async_handler_lint,stale_capture_lint}.rs` | Modify | Exhaustive-match passthrough arms. |
| `crates/vox-codegen/src/web_ir/mod.rs` | Modify | Add `BehaviorNode::StreamSub`. |
| `crates/vox-codegen/src/web_ir/lower.rs` | Modify | `HirReactiveMember::OnStream` → `BehaviorNode::StreamSub`. |
| `contracts/channels.v1.yaml` | **New** | Channel SSOT: name → uri → payload → optional poll. |
| `crates/vox-codegen-ts/src/channels.rs` | **New** | Contract loader + types + `transport.ts` parity test. |
| `crates/vox-codegen-ts/src/channel_runtime_emit.rs` | **New** | Generate `vox-channel.ts` (transport interface + Tauri + mock + fallback + typed map). |
| `crates/vox-codegen-ts/src/reactive/effects.rs` | Modify | Emit `on stream` member → `useEffect` calling `voxChannel.subscribe`. |
| `crates/vox-codegen-ts/src/reactive/{bindings,hooks}.rs`, `reactive_module_emit.rs`, `crates/vox-rn-codegen/src/{component,mobile_utils}.rs` | Modify | Exhaustive-match passthrough arms. |
| `docs/superpowers/ledgers/frontend-coverage-ledger.md` | Modify | Flip one demonstrated surface's status note. |

---

## Task 1: AST node + parser `[SEQUENTIAL base]`

**Files:**
- Modify: `crates/vox-ast/src/decl/ui.rs` (after `OnCleanupDecl`, ~line 276; and the `ReactiveMemberDecl` enum ~line 207)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_component.rs` (component scope ~line 107 `Token::On` arm; module scope ~line 253 `Token::On` arm)
- Modify: `crates/vox-compiler/src/fmt/printer.rs`, `crates/vox-compiler/src/parser/with_registry.rs` (exhaustive matches)
- Test: `crates/vox-compiler/src/parser/descent/tests.rs`

- [ ] **Step 1: Write the failing parser test**

Add to `crates/vox-compiler/src/parser/descent/tests.rs`:

```rust
#[test]
fn parses_on_stream_member() {
    let src = r#"
component Live() {
    state status: str = ""
    on stream(orch_status) as s: { status = s }
    view: text { status }
}
"#;
    let module = crate::parser::parse(crate::lexer::cursor::lex(src)).expect("parse");
    let decl = module.decls.iter().find_map(|d| match d {
        crate::ast::decl::Decl::ReactiveComponent(rc) => Some(rc),
        _ => None,
    }).expect("reactive component");
    let has_stream = decl.members.iter().any(|m| matches!(
        m, crate::ast::decl::ReactiveMemberDecl::OnStream(s)
            if s.channel == "orch_status" && s.binding == "s"
    ));
    assert!(has_stream, "expected an OnStream member with channel=orch_status binding=s");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler parses_on_stream_member`
Expected: FAIL — `no variant named OnStream` (enum lacks the variant).

- [ ] **Step 3: Add the AST node**

In `crates/vox-ast/src/decl/ui.rs`, add to the `ReactiveMemberDecl` enum (after the `OnCleanup` arm, ~line 217):

```rust
    /// Subscribes to a named push channel: `on stream(name) as bind: { body }`.
    OnStream(OnStreamDecl),
```

And add the struct after `OnCleanupDecl` (~line 276):

```rust
/// `on stream(channel) as binding: { body }` — subscribe to a named push channel.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OnStreamDecl {
    /// Channel registry name (validated against `contracts/channels.v1.yaml`).
    pub channel: String,
    /// Identifier bound to each received frame inside `body`.
    pub binding: String,
    /// Handler body; runs per frame with `binding` in scope.
    pub body: crate::expr::Expr,
    /// Source span.
    pub span: Span,
}
```

- [ ] **Step 4: Parse it (component scope)**

In `head_component.rs`, inside the `Token::On` arm of `finish_reactive_component_after_name` (the `match self.peek().clone()` at ~line 110), add a new arm BEFORE the catch-all `_`:

```rust
                        Token::Ident(n) if n == "stream" => {
                            self.advance(); // eat `stream`
                            self.expect(&Token::LParen)?;
                            let channel = self.parse_ident_name()?;
                            self.expect(&Token::RParen)?;
                            // `as binding`
                            match self.peek().clone() {
                                Token::Ident(a) if a == "as" => { self.advance(); }
                                other => {
                                    self.errors.push(ParseError::classified(
                                        self.span(),
                                        "Expected `as <binding>` after `on stream(<channel>)`.",
                                        vec!["as frame".into()],
                                        Some(other.to_string()),
                                        ParseErrorClass::Declaration,
                                    ));
                                    return Err(());
                                }
                            }
                            let binding = self.parse_ident_name()?;
                            let body = self.parse_reactive_block()?;
                            members.push(ReactiveMemberDecl::OnStream(OnStreamDecl {
                                channel,
                                binding,
                                body,
                                span: on_start.merge(self.span()),
                            }));
                        }
```

Update the `use` at the top of `head_component.rs` to include `OnStreamDecl`:

```rust
use crate::ast::decl::{
    Decl, EffectDecl, OnCleanupDecl, OnMountDecl, OnStreamDecl, ReactiveComponentDecl,
    ReactiveMemberDecl,
};
```

Also update the error message in the existing `_` arm (currently "Expected `mount` or `cleanup` after `on`") to read:
`"Expected `mount`, `cleanup`, or `stream` after `on` in reactive component block."` and add `"stream(orch_status) as s: { … }".into()` to its examples vec.

- [ ] **Step 5: Parse it (module scope)**

Apply the identical new `Token::Ident(n) if n == "stream"` arm inside the `Token::On` match in `parse_reactive_module_decl` (~line 253), and add `OnStreamDecl` to that function's local `use crate::ast::decl::{…}` import. Update its `_` error message the same way (append `stream`).

- [ ] **Step 6: Satisfy the other exhaustive matches**

`cargo build -p vox-compiler` will now fail in `fmt/printer.rs` and `parser/with_registry.rs` (non-exhaustive match on `ReactiveMemberDecl`). Add arms:

In `fmt/printer.rs`, find the `match member { … ReactiveMemberDecl::OnCleanup(c) => … }` and add:

```rust
            ReactiveMemberDecl::OnStream(s) => {
                self.push_line(&format!("on stream({}) as {}:", s.channel, s.binding));
                self.print_expr_block(&s.body);
            }
```

(Match the surrounding helper names actually used in `printer.rs`; if `print_expr_block` differs, use the same helper the `OnCleanup` arm uses on `c.body`.)

In `parser/with_registry.rs`, locate the `ReactiveMemberDecl` match and add a passthrough arm consistent with how `OnCleanup` is handled there (most arms in this file are structural recursion; add `ReactiveMemberDecl::OnStream(s) => { /* recurse into s.body like OnCleanup's c.body */ }`).

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test -p vox-compiler parses_on_stream_member`
Expected: PASS.

- [ ] **Step 8: Add a malformed-form failing test**

Add to `tests.rs`:

```rust
#[test]
fn on_stream_requires_as_binding() {
    let src = r#"
component Bad() {
    on stream(orch_status): { }
    view: text { "x" }
}
"#;
    let res = crate::parser::parse(crate::lexer::cursor::lex(src));
    assert!(res.is_err(), "missing `as <binding>` must be a parse error");
}
```

Run: `cargo test -p vox-compiler on_stream_requires_as_binding` → PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-ast/src/decl/ui.rs crates/vox-compiler/src/parser/descent/decl/head_component.rs crates/vox-compiler/src/fmt/printer.rs crates/vox-compiler/src/parser/with_registry.rs crates/vox-compiler/src/parser/descent/tests.rs
git commit -m "feat(parser): on stream(channel) as binding reactive member"
```

---

## Task 2: HIR node + AST→HIR lowering `[SEQUENTIAL after 1]`

**Files:**
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` (enum ~line 718; structs ~line 765)
- Modify: `crates/vox-compiler/src/hir/lower/decl.rs` (~line 408)
- Modify (passthrough arms): `crates/vox-compiler/src/hir/lower/mod.rs`, `hir/db_op_walk.rs`, `typeck/mod.rs`, `typeck/checker/mod.rs`, `typeck/async_handler_lint.rs`, `typeck/stale_capture_lint.rs`
- Test: `crates/vox-compiler/src/hir/lower/decl.rs` (inline `#[cfg(test)]`) or `crates/vox-compiler/tests/`

- [ ] **Step 1: Write the failing lowering test**

Create `crates/vox-compiler/tests/on_stream_lower.rs`:

```rust
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const SRC: &str = r#"
component Live() {
    state status: str = ""
    on stream(orch_status) as s: { status = s }
    view: text { status }
}
"#;

#[test]
fn on_stream_lowers_to_hir_member() {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let rc = hir.components.iter().find(|c| c.name == "Live").expect("Live");
    let found = rc.members.iter().any(|m| matches!(
        m, vox_compiler::hir::HirReactiveMember::OnStream(s)
            if s.channel == "orch_status" && s.binding == "s"
    ));
    assert!(found, "expected lowered HirReactiveMember::OnStream");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-compiler --test on_stream_lower`
Expected: FAIL — `no variant named OnStream` on `HirReactiveMember`.

- [ ] **Step 3: Add the HIR node**

In `crates/vox-compiler/src/hir/nodes/decl.rs`, add to the `HirReactiveMember` enum (after `OnCleanup`, ~line 723):

```rust
    OnStream(HirOnStream),
```

And after `HirOnCleanup` (~line 765):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirOnStream {
    pub channel: String,
    pub binding: String,
    pub body: HirExpr,
    pub span: Span,
}
```

- [ ] **Step 4: Lower AST→HIR**

In `crates/vox-compiler/src/hir/lower/decl.rs`, add an arm before `ReactiveMemberDecl::Stmt` (~line 408):

```rust
                ReactiveMemberDecl::OnStream(s) => HirReactiveMember::OnStream(
                    crate::hir::nodes::decl::HirOnStream {
                        channel: s.channel.clone(),
                        binding: s.binding.clone(),
                        body: self.lower_expr(&s.body),
                        span: s.span,
                    },
                ),
```

- [ ] **Step 5: Add passthrough arms to the remaining matches**

`cargo build -p vox-compiler` will fail at each non-exhaustive `HirReactiveMember` match. For each of `hir/lower/mod.rs`, `hir/db_op_walk.rs`, `typeck/mod.rs`, `typeck/checker/mod.rs`, `typeck/async_handler_lint.rs`, `typeck/stale_capture_lint.rs`, add an arm that mirrors how `OnMount`/`OnCleanup` are treated there. For the lint/walk files that recurse into the body expression, recurse into `s.body` exactly as the `OnCleanup` arm recurses into its body; for files that ignore those members, add `HirReactiveMember::OnStream(_) => {}`. (Grep each file for `OnCleanup` to find the exact arm to copy.)

- [ ] **Step 6: Run to verify it passes**

Run: `cargo build -p vox-compiler && cargo test -p vox-compiler --test on_stream_lower`
Expected: build OK; test PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/hir crates/vox-compiler/src/typeck crates/vox-compiler/tests/on_stream_lower.rs
git commit -m "feat(hir): lower on stream member to HirReactiveMember::OnStream"
```

---

## Task 3: WebIR `BehaviorNode::StreamSub` + lowering `[SEQUENTIAL after 2]`

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/mod.rs` (`BehaviorNode` enum ~line 251)
- Modify: `crates/vox-codegen/src/web_ir/lower.rs` (member match ~line 859)
- Test: `crates/vox-codegen/tests/on_stream_webir.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-codegen/tests/on_stream_webir.rs`:

```rust
use vox_codegen::web_ir::{lower_hir_to_web_ir, BehaviorNode};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const SRC: &str = r#"
component Live() {
    state status: str = ""
    on stream(orch_status) as s: { status = s }
    view: text { status }
}
"#;

#[test]
fn on_stream_lowers_to_streamsub_behavior() {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let (web, _summary) = lower_hir_to_web_ir(&hir);
    let found = web.behavior_nodes.iter().any(|b| matches!(
        b, BehaviorNode::StreamSub { channel, binding, .. }
            if channel == "orch_status" && binding == "s"
    ));
    assert!(found, "expected a BehaviorNode::StreamSub for orch_status");
}
```

> Confirm the public lowering fn name/signature: the seam used by Sub-project A is `lower_hir_to_web_ir(hir)` returning `(WebIrModule, summary)`. If the crate re-exports differ, adjust the `use` to the actual `vox_codegen::web_ir` path (grep `pub fn lower_hir_to_web_ir`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-codegen --test on_stream_webir`
Expected: FAIL — `no variant named StreamSub`.

- [ ] **Step 3: Add the WebIR node**

In `crates/vox-codegen/src/web_ir/mod.rs`, add to `BehaviorNode` (after `EffectDecl`, ~line 267):

```rust
    StreamSub {
        /// Channel registry name (validated against the channel contract at emit).
        channel: String,
        /// Identifier bound to each received frame in `body`.
        binding: String,
        /// Emitted handler body (TS source string), with `binding` in scope.
        body: String,
        span: Option<SourceSpanId>,
    },
```

- [ ] **Step 4: Lower it**

In `crates/vox-codegen/src/web_ir/lower.rs`, add an arm in the member match (after the `OnCleanup` arm, ~line 866):

```rust
                HirReactiveMember::OnStream(os) => {
                    let body = emit_hir_expr(&os.body, &mem_ctx);
                    m.behavior_nodes.push(BehaviorNode::StreamSub {
                        channel: os.channel.clone(),
                        binding: os.binding.clone(),
                        body,
                        span: None,
                    });
                }
```

- [ ] **Step 5: Satisfy any other `BehaviorNode` exhaustive matches**

`cargo build -p vox-codegen` may fail where `BehaviorNode` is matched elsewhere (grep `BehaviorNode::EffectDecl`). Add `BehaviorNode::StreamSub { .. } => {}` (or the analogous handling) at each site that is not the TS emitter (the TS emitter handling is Task 6).

- [ ] **Step 6: Run to verify it passes**

Run: `cargo build -p vox-codegen && cargo test -p vox-codegen --test on_stream_webir`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-codegen/src/web_ir crates/vox-codegen/tests/on_stream_webir.rs
git commit -m "feat(web-ir): lower on stream to BehaviorNode::StreamSub"
```

---

## Task 4: Channel contract + loader + parity test `[PARALLEL-SAFE]`

**Files:**
- Create: `contracts/channels.v1.yaml`
- Create: `crates/vox-codegen-ts/src/channels.rs`
- Modify: `crates/vox-codegen-ts/src/mod.rs` (register `pub mod channels;`)
- Test: inline `#[cfg(test)]` in `channels.rs`

- [ ] **Step 1: Write the contract**

Create `contracts/channels.v1.yaml`. Enumerate exactly the event-name constants currently in `crates/vox-gui/ui/src/transport.ts` (grep `= 'vox://`). As of this writing they are the nine below; **before committing, re-grep `transport.ts` and reconcile 1:1** (add/remove rows to match reality — do not assume a count):

```yaml
schema_version: 1
channels:
  - name: orch_status
    uri: "vox://orch-status"
    payload: OrchestratorStatus
    semantics: replace
    poll: { command: get_orchestrator_status, every_ms: 5000 }
  - name: agent_events
    uri: "vox://agent-events"
    payload: AgentEventFrame
    semantics: fold
  - name: scientia_queue
    uri: "vox://scientia-queue"
    payload: ScientiaQueuePing
    semantics: fold
  - name: scientia_discovery_surfaced
    uri: "vox://scientia-discovery-surfaced"
    payload: DiscoverySurfacedPayload
    semantics: fold
  - name: browser_frame
    uri: "vox://browser-frame"
    payload: BrowserFramePayload
    semantics: replace
  - name: preview_available
    uri: "vox://preview-available"
    payload: PreviewAvailablePayload
    semantics: replace
  - name: secretary_proposed
    uri: "vox://secretary-proposed-task"
    payload: SecretaryProposedPayload
    semantics: fold
  - name: pty_output
    uri: "vox://pty-output"
    payload: PtyOutputFrame
    semantics: fold
  - name: pty_exit
    uri: "vox://pty-exit"
    payload: PtyExitFrame
    semantics: fold
```

- [ ] **Step 2: Write the failing loader + parity test**

Create `crates/vox-codegen-ts/src/channels.rs`:

```rust
//! Channel-contract SSOT loader (`contracts/channels.v1.yaml`) for the frontend
//! stream-subscription primitive. The contract maps a `.vox`-facing channel name
//! to its wire URI, payload type, replace/fold semantics, and optional polling
//! fallback. A parity test enforces that the contract's URI set matches the event
//! constants hand-declared in `crates/vox-gui/ui/src/transport.ts` so the two
//! cannot drift.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ChannelPoll {
    pub command: String,
    pub every_ms: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ChannelDef {
    pub name: String,
    pub uri: String,
    pub payload: String,
    pub semantics: String,
    #[serde(default)]
    pub poll: Option<ChannelPoll>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelContract {
    pub schema_version: u32,
    pub channels: Vec<ChannelDef>,
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/vox-codegen-ts → workspace root is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Load and parse the channel contract from the workspace `contracts/` dir.
pub fn load_channel_contract() -> ChannelContract {
    let path = workspace_root().join("contracts/channels.v1.yaml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Look up a channel by its `.vox`-facing name.
pub fn channel_by_name<'a>(c: &'a ChannelContract, name: &str) -> Option<&'a ChannelDef> {
    c.channels.iter().find(|ch| ch.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn contract_parses_and_names_are_unique() {
        let c = load_channel_contract();
        assert_eq!(c.schema_version, 1);
        let names: BTreeSet<_> = c.channels.iter().map(|ch| ch.name.clone()).collect();
        assert_eq!(names.len(), c.channels.len(), "channel names must be unique");
        for ch in &c.channels {
            assert!(
                ch.semantics == "replace" || ch.semantics == "fold",
                "channel {} has invalid semantics {}", ch.name, ch.semantics
            );
        }
    }

    /// Drift guard: the contract's URI set must equal the `vox://…` event-name
    /// string literals declared in transport.ts.
    #[test]
    fn contract_uris_match_transport_ts() {
        let c = load_channel_contract();
        let contract_uris: BTreeSet<String> =
            c.channels.iter().map(|ch| ch.uri.clone()).collect();

        let ts = std::fs::read_to_string(
            workspace_root().join("crates/vox-gui/ui/src/transport.ts"),
        )
        .expect("read transport.ts");
        let mut ts_uris: BTreeSet<String> = BTreeSet::new();
        for (i, _) in ts.match_indices("'vox://") {
            let rest = &ts[i + 1..]; // skip opening quote
            if let Some(end) = rest.find('\'') {
                ts_uris.insert(rest[..end].to_string());
            }
        }

        let missing: Vec<_> = ts_uris.difference(&contract_uris).collect();
        let extra: Vec<_> = contract_uris.difference(&ts_uris).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "channel contract drifted from transport.ts.\n  in transport.ts but not contract: {missing:?}\n  in contract but not transport.ts: {extra:?}"
        );
    }
}
```

- [ ] **Step 3: Register the module + dependency**

In `crates/vox-codegen-ts/src/mod.rs`, add `pub mod channels;` alongside the other `pub mod` lines. Ensure `serde_yaml` is a dependency of `vox-codegen-ts` (`cargo add -p vox-codegen-ts serde_yaml` if `cargo tree -p vox-codegen-ts | grep serde_yaml` is empty).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-codegen-ts channels::`
Expected: `contract_parses_and_names_are_unique` PASS. `contract_uris_match_transport_ts` PASS **only after** Step 1's URI set matches `transport.ts` exactly — if it fails, the failure message lists the diff; fix `channels.v1.yaml` to match `transport.ts` and re-run.

- [ ] **Step 5: Commit**

```bash
git add contracts/channels.v1.yaml crates/vox-codegen-ts/src/channels.rs crates/vox-codegen-ts/src/mod.rs crates/vox-codegen-ts/Cargo.toml
git commit -m "feat(channels): channel contract SSOT + loader + transport.ts parity guard"
```

---

## Task 5: Channel runtime emitter (`vox-channel.ts`) `[SEQUENTIAL after 4]`

**Files:**
- Create: `crates/vox-codegen-ts/src/channel_runtime_emit.rs`
- Modify: `crates/vox-codegen-ts/src/mod.rs` (register module)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-codegen-ts/src/channel_runtime_emit.rs`:

```rust
//! Emits the transport-neutral channel runtime (`vox-channel.ts`) from the channel
//! contract. The runtime exposes `voxChannel.subscribe(name, cb)` which resolves a
//! transport lazily and guarded: the Tauri transport (wrapping `listen`) is loaded
//! ONLY when `__TAURI_INTERNALS__` exists; otherwise a dev-mock transport is used,
//! and any channel declaring `poll:` falls back to interval refetch. Nothing here
//! imports `@tauri-apps` at module load — that is the bare-browser crash fix.

use crate::channels::{load_channel_contract, ChannelContract};

/// Emit the full `vox-channel.ts` source from the channel contract.
pub fn emit_channel_runtime(contract: &ChannelContract) -> String {
    let mut out = String::new();
    out.push_str("// GENERATED by vox-codegen-ts channel_runtime_emit. Do not edit.\n");
    out.push_str("export type ChannelName =\n");
    for ch in &contract.channels {
        out.push_str(&format!("  | \"{}\"\n", ch.name));
    }
    out.push_str(";\n\n");

    // name → wire uri table
    out.push_str("const CHANNEL_URI: Record<ChannelName, string> = {\n");
    for ch in &contract.channels {
        out.push_str(&format!("  {}: \"{}\",\n", ch.name, ch.uri));
    }
    out.push_str("};\n\n");

    // optional poll table
    out.push_str("const CHANNEL_POLL: Partial<Record<ChannelName, { command: string; everyMs: number }>> = {\n");
    for ch in &contract.channels {
        if let Some(p) = &ch.poll {
            out.push_str(&format!(
                "  {}: {{ command: \"{}\", everyMs: {} }},\n",
                ch.name, p.command, p.every_ms
            ));
        }
    }
    out.push_str("};\n\n");

    out.push_str(RUNTIME_BODY);
    out
}

/// Static runtime body: guarded lazy transport resolution + subscribe + poll fallback.
const RUNTIME_BODY: &str = r#"export interface VoxChannelTransport {
  subscribe(uri: string, onFrame: (raw: unknown) => void): Promise<() => void>;
}

function hasTauri(): boolean {
  return typeof (globalThis as any).__TAURI_INTERNALS__ !== "undefined";
}

async function tauriTransport(): Promise<VoxChannelTransport> {
  // Dynamic import so @tauri-apps is never pulled in at module load.
  const { listen } = await import("@tauri-apps/api/event");
  return {
    subscribe: (uri, onFrame) =>
      listen(uri, (e: { payload: unknown }) => onFrame(e.payload)),
  };
}

const mockTransport: VoxChannelTransport = {
  subscribe: async () => () => {},
};

async function resolveTransport(): Promise<VoxChannelTransport> {
  if (hasTauri()) {
    try { return await tauriTransport(); } catch { return mockTransport; }
  }
  return mockTransport;
}

export const voxChannel = {
  subscribe(name: ChannelName, onFrame: (payload: any) => void): Promise<() => void> {
    const uri = CHANNEL_URI[name];
    return resolveTransport().then((t) => {
      if (t === mockTransport) {
        const poll = CHANNEL_POLL[name];
        if (poll) {
          // Transparent fallback: the GUI host registers a refetch handler keyed by
          // command name; if absent this is a harmless no-op interval.
          const id = setInterval(() => {
            const fn = (globalThis as any).__voxPoll?.[poll.command];
            if (typeof fn === "function") void fn();
          }, poll.everyMs);
          return () => clearInterval(id);
        }
        return () => {};
      }
      return t.subscribe(uri, onFrame);
    });
  },
};
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_has_guarded_transport_and_no_toplevel_tauri_import() {
        let c = load_channel_contract();
        let src = emit_channel_runtime(&c);
        // Typed channel union present.
        assert!(src.contains("export type ChannelName"));
        // Guarded resolution present.
        assert!(src.contains("__TAURI_INTERNALS__"));
        assert!(src.contains("export const voxChannel"));
        // CRITICAL: no static top-level `import ... @tauri-apps`. The only reference
        // must be the dynamic `await import("@tauri-apps/api/event")`.
        for line in src.lines() {
            let l = line.trim_start();
            if l.starts_with("import ") {
                assert!(
                    !l.contains("@tauri-apps"),
                    "static @tauri-apps import would crash bare browser: {line}"
                );
            }
        }
        // A channel that declares poll must appear in the poll table.
        if c.channels.iter().any(|ch| ch.poll.is_some()) {
            assert!(src.contains("everyMs:"), "poll table missing");
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-codegen-ts channel_runtime_emit::`
Expected: FAIL — module not yet registered / not compiled.

- [ ] **Step 3: Register and pass**

Add `pub mod channel_runtime_emit;` to `crates/vox-codegen-ts/src/mod.rs`.
Run: `cargo test -p vox-codegen-ts channel_runtime_emit::`
Expected: PASS.

- [ ] **Step 4: Wire runtime emission into the frontend output**

The generated `vox-channel.ts` must be written into the emitted file set. In the TS emitter assembly (grep `vox-client.ts` in `crates/vox-codegen-ts/src/emitter.rs` to find where shared runtime files are pushed onto `output.files`), add — guarded so it only emits when at least one component contains a `StreamSub` behavior, to avoid emitting an unused file:

```rust
// Emit the channel runtime when any component subscribes to a stream.
let needs_channels = web_module
    .behavior_nodes
    .iter()
    .any(|b| matches!(b, crate::web_ir::BehaviorNode::StreamSub { .. }));
if needs_channels {
    let contract = crate::channels::load_channel_contract();
    files.push((
        "vox-channel.ts".to_string(),
        crate::channel_runtime_emit::emit_channel_runtime(&contract),
    ));
}
```

(Adapt the variable names `web_module` / `files` to the actual locals at the emit site.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen-ts/src/channel_runtime_emit.rs crates/vox-codegen-ts/src/mod.rs crates/vox-codegen-ts/src/emitter.rs
git commit -m "feat(channels): emit transport-neutral vox-channel.ts runtime"
```

---

## Task 6: Emit `on stream` → `useEffect` + unknown-channel diagnostic `[SEQUENTIAL after 3 and 5]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/reactive/effects.rs` (member match ~line 161)
- Modify (passthrough): `crates/vox-codegen-ts/src/reactive/{bindings,hooks}.rs`, `crates/vox-codegen-ts/src/reactive_module_emit.rs`, `crates/vox-rn-codegen/src/{component,mobile_utils}.rs`
- Test: `crates/vox-codegen/tests/on_stream_emit.rs`

- [ ] **Step 1: Write the failing golden test**

Create `crates/vox-codegen/tests/on_stream_emit.rs`:

```rust
use vox_codegen::codegen_ts::{generate_with_options, CodegenOptions};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const SRC: &str = r#"
component Live() {
    state status: str = ""
    on stream(orch_status) as s: { status = s }
    view: text { status }
}
"#;

fn live_tsx() -> String {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("emit");
    out.files
        .iter()
        .find(|(name, _)| name == "Live.tsx")
        .map(|(_, body)| body.clone())
        .expect("Live.tsx emitted")
}

#[test]
fn on_stream_emits_subscribe_useeffect_without_tauri_import() {
    let tsx = live_tsx();
    assert!(tsx.contains("voxChannel.subscribe(\"orch_status\""),
        "expected a voxChannel.subscribe call; got:\n{tsx}");
    assert!(tsx.contains("useEffect("), "subscription must be inside a useEffect");
    // Auto-cleanup: the effect returns an unsubscribe.
    assert!(tsx.contains("return () =>"), "effect must return a cleanup");
    // Transport-neutral: the component never imports @tauri-apps directly.
    assert!(!tsx.contains("@tauri-apps"),
        "emitted component must not import @tauri-apps; got:\n{tsx}");
    // The runtime is imported from the generated module.
    assert!(tsx.contains("vox-channel") || tsx.contains("./vox-channel"),
        "component must import the voxChannel runtime");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-codegen --test on_stream_emit`
Expected: FAIL — the member is currently dropped (no emit arm) or build fails on a non-exhaustive match.

- [ ] **Step 3: Emit the member**

In `crates/vox-codegen-ts/src/reactive/effects.rs`, add an arm to the `for member in &rc.members { match member { … } }` loop (after `OnCleanup`, ~line 213):

```rust
            HirReactiveMember::OnStream(os) => {
                // Validate the channel name against the contract; unknown → diagnostic + skip.
                let contract = crate::channels::load_channel_contract();
                if crate::channels::channel_by_name(&contract, &os.channel).is_none() {
                    let valid: Vec<&str> =
                        contract.channels.iter().map(|c| c.name.as_str()).collect();
                    out.push_str(&format!(
                        "  // vox/web/unknown-channel: `{}` is not in contracts/channels.v1.yaml \
                         (valid: {}). Subscription skipped.\n",
                        os.channel,
                        valid.join(", ")
                    ));
                } else {
                    let handler_body = emit_block_stmts(&os.body, &view_ctx, 4);
                    out.push_str("  useEffect(() => {\n");
                    out.push_str("    let unsub: (() => void) | undefined;\n");
                    out.push_str("    let cancelled = false;\n");
                    out.push_str(&format!(
                        "    voxChannel.subscribe(\"{}\", ({}) => {{\n{}    }})\n",
                        os.channel, os.binding, handler_body
                    ));
                    out.push_str("      .then((u) => { if (cancelled) u(); else unsub = u; });\n");
                    out.push_str("    return () => { cancelled = true; unsub?.(); };\n");
                    out.push_str("  }, []);\n");
                }
            }
```

Add the runtime import near the top of the component emit (where other `import` lines are pushed, e.g. after the `react_import_line`): when `rc.members` contains an `OnStream`, push:

```rust
    if rc.members.iter().any(|m| matches!(m, HirReactiveMember::OnStream(_))) {
        out.push_str("import { voxChannel } from \"./vox-channel\";\n");
    }
```

- [ ] **Step 4: Add passthrough arms for the remaining `HirReactiveMember` matches**

`cargo build` will fail in `reactive/bindings.rs`, `reactive/hooks.rs`, `reactive_module_emit.rs`, and the RN crate (`vox-rn-codegen/src/component.rs`, `mobile_utils.rs`). For each, grep `OnCleanup` and add an `OnStream` arm:
- In `hooks.rs`/`bindings.rs` (which collect reactive binding names / import refs): treat `os.body` exactly like `OnCleanup`'s body for name/import collection; the binding name `os.binding` is a local, not state, so it is NOT added to the state set.
- In `reactive_module_emit.rs`: emit the same `useEffect` block as Step 3, or if module-scope streams are out of scope, add `HirReactiveMember::OnStream(_) => {}` and a `// module-scope on stream not yet emitted` comment.
- In the RN crate: add `HirReactiveMember::OnStream(_) => {}` (RN target stays dormant per the program; no behavior).

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p vox-codegen --test on_stream_emit`
Expected: PASS.

- [ ] **Step 6: Add the unknown-channel diagnostic test**

Append to `on_stream_emit.rs`:

```rust
#[test]
fn unknown_channel_emits_diagnostic_comment() {
    let src = r#"
component Bad() {
    state x: str = ""
    on stream(not_a_real_channel) as s: { x = s }
    view: text { x }
}
"#;
    let hir = lower_module(&parse(lex(src)).expect("parse"));
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("emit");
    let tsx = out.files.iter().find(|(n, _)| n == "Bad.tsx").map(|(_, b)| b.clone()).expect("Bad.tsx");
    assert!(tsx.contains("vox/web/unknown-channel"),
        "unknown channel must produce a diagnostic comment; got:\n{tsx}");
    assert!(!tsx.contains("voxChannel.subscribe(\"not_a_real_channel\""),
        "unknown channel must not emit a subscribe call");
}
```

Run: `cargo test -p vox-codegen --test on_stream_emit` → both PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-codegen-ts/src/reactive crates/vox-codegen-ts/src/reactive_module_emit.rs crates/vox-rn-codegen/src crates/vox-codegen/tests/on_stream_emit.rs
git commit -m "feat(codegen-ts): emit on stream as guarded voxChannel useEffect"
```

---

## Task 7: End-to-end + ledger surface demonstration + DoD green `[SEQUENTIAL after 6]`

**Files:**
- Modify: `docs/superpowers/ledgers/frontend-coverage-ledger.md`
- Test: `crates/vox-codegen/tests/on_stream_e2e.rs`

- [ ] **Step 1: Write the end-to-end test**

Create `crates/vox-codegen/tests/on_stream_e2e.rs`:

```rust
use vox_codegen::codegen_ts::{generate_with_options, CodegenOptions};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

// A Dashboard-like surface: a live orchestrator status panel expressed entirely in
// .vox via `on stream`. Proves the blocked:reactive-streams surface is now expressible.
const SRC: &str = r#"
component LiveDashboard() {
    state agents: str = "0"
    on stream(orch_status) as s: { agents = s }
    view: column {
        heading(level=2) { "Live" }
        text { agents }
    }
}
"#;

#[test]
fn live_dashboard_emits_component_and_channel_runtime() {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("emit");

    let comp = out.files.iter().find(|(n, _)| n == "LiveDashboard.tsx").expect("component");
    assert!(comp.1.contains("voxChannel.subscribe(\"orch_status\""));
    assert!(!comp.1.contains("@tauri-apps"));

    // The channel runtime is emitted alongside (because a StreamSub exists).
    let runtime = out.files.iter().find(|(n, _)| n == "vox-channel.ts");
    assert!(runtime.is_some(), "vox-channel.ts must be emitted when a stream is used");
    let rt = &runtime.unwrap().1;
    assert!(rt.contains("__TAURI_INTERNALS__"), "runtime must guard the Tauri transport");
    for line in rt.lines() {
        let l = line.trim_start();
        if l.starts_with("import ") {
            assert!(!l.contains("@tauri-apps"), "runtime has a static tauri import: {line}");
        }
    }
}
```

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test -p vox-codegen --test on_stream_e2e`
Expected: PASS (all prior tasks make this green on first run; if `column`/`heading` view primitives differ in name, adjust the view to the minimal `view: text { agents }` form proven in Task 6).

- [ ] **Step 3: Update the coverage ledger note**

In `docs/superpowers/ledgers/frontend-coverage-ledger.md`, change the `Dashboard` row's Notes to record the new capability (keep `Status` as-is until full migration in Sub-project G; this is a capability note, not a surface flip):

```markdown
| Dashboard | blocked:reactive-streams | orch-status now `.vox`-expressible via `on stream` (Sub-project B); full surface migration pending (Sub-project G) |
```

Verify the drift guard still passes (the row name/status columns are unchanged, so the Sub-project A currency test stays green):

Run: `cargo test -p vox-codegen --test frontend_coverage_ledger`
Expected: PASS.

- [ ] **Step 4: Full DoD check**

Run, expecting all green:

```bash
cargo test -p vox-compiler --test on_stream_lower
cargo test -p vox-compiler parses_on_stream_member on_stream_requires_as_binding
cargo test -p vox-codegen --test on_stream_webir
cargo test -p vox-codegen-ts channels:: channel_runtime_emit::
cargo test -p vox-codegen --test on_stream_emit
cargo test -p vox-codegen --test on_stream_e2e
cargo test -p vox-codegen --test frontend_coverage_ledger
cargo clippy -p vox-ast -p vox-compiler -p vox-codegen -p vox-codegen-ts -- -D warnings
```

(If the build broker intercepts `cargo clippy` with a recursion-abort in this environment, run the clippy line outside the broker context or rely on CI; the per-crate test lines are the authoritative gate.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/tests/on_stream_e2e.rs docs/superpowers/ledgers/frontend-coverage-ledger.md
git commit -m "test(codegen): on stream end-to-end + ledger capability note"
```

---

## Definition of Done (Sub-project B)

- [ ] `on stream(channel) as s: { … }` parses at component + module scope; malformed forms produce classified errors (Task 1).
- [ ] It lowers AST→HIR→WebIR (`OnStream` → `HirOnStream` → `BehaviorNode::StreamSub`) (Tasks 2–3).
- [ ] `contracts/channels.v1.yaml` enumerates exactly the `transport.ts` channels; a parity test guards drift (Task 4).
- [ ] The generated `vox-channel.ts` resolves its transport lazily/guarded with a dev-mock + contract `poll:` fallback and **no** static `@tauri-apps` import (Task 5).
- [ ] Emitted components subscribe via `voxChannel.subscribe` inside a `useEffect` with auto-cleanup, never importing `@tauri-apps`; unknown channels emit a `vox/web/unknown-channel` diagnostic (Task 6).
- [ ] An end-to-end live-dashboard component + runtime emit proves a `blocked:reactive-streams` surface is now `.vox`-expressible; the Sub-project A drift guard stays green (Task 7).
- [ ] All new + existing `vox-compiler` / `vox-codegen` / `vox-codegen-ts` tests pass; clippy clean on touched crates.

## What this deliberately does NOT do (deferred)

- **Production browser WS/SSE gateway transport** — declared seam only (later sub-project); B ships interface + Tauri + dev-mock.
- **A `vox ci` subcommand parity gate** — the parity check ships as a workspace `cargo test` (runs in CI); promoting it to a first-class `vox ci channel-parity` gate belongs to Sub-project F (convergence gate).
- **Migrating the 173 `.tsx` surfaces** — Sub-project G.
- **Ecosystem-import (C), mobile-first/PWA (D), toolchain automation (E).**
