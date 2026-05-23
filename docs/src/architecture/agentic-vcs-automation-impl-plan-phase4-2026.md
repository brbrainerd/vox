---
title: "Agentic VCS Automation — Phase 4 Implementation Plan (2026-05-09)"
description: "Step-by-step TDD plan that adds the Vox-language @vcs.* decorator surface: parser support for @vcs.read_only / @vcs.requires(...) / @vcs.linear_working_tree / @vcs.audit_trail on fn declarations, HIR effect annotations, type-checker rules (read_only cannot call requires; linear cap consumed at most once), lowering to the Phase 1 Rust capability types, and standard-library .vox shapes for VcsCapability. Builds on Phases 1–3 and on the existing @durable / @endpoint precedent in the compiler."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 4 turns capability-typed VCS effects from a Rust-side soft contract into a language-level enforced contract. The compiler refuses to emit code that calls a write-side fn without the requisite capability. Concrete code, exact file paths, exact commands. Future agents executing this plan should not need to invent code."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-compiler: parser/HIR/typecheck for @vcs.* decorators"
  - "vox-codegen: lowering @vcs.requires(T) to Rust 'cap: T' parameter"
  - "vox-orchestrator-types: vcs_capability mint methods harden to pub(crate) + sealed trait"
  - "scripts/vcs/: existing .vox files type-check after this lands; remove their // vox:skip annotations"
---

# Agentic VCS Automation — Phase 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
>
> **Companion docs:** [Phase 1 plan](agentic-vcs-automation-impl-plan-phase1-2026.md), [Phase 2 plan](agentic-vcs-automation-impl-plan-phase2-2026.md), [Phase 3 plan](agentic-vcs-automation-impl-plan-phase3-2026.md), [research](agentic-version-control-automation-research-2026.md). Read research §"Layer 2 — Vox decorator surface" and the AGENTS.md grammar policy on bare keywords vs decorators before starting.

**Goal:** Make capability-typed VCS effects a *language-level* property. After Phase 4, a `.vox` fn annotated `@vcs.read_only` is statically forbidden from calling any fn annotated `@vcs.requires(T)`; a `@vcs.linear_working_tree` capability cannot be reused after consumption; a missing `@vcs.requires` annotation on a fn that internally calls a write-side primitive is a compile error. The Rust capability types from Phase 1 are the lowering target — Vox source becomes the unforgeable origin, Rust the implementation substrate.

**Architecture:** Three layers of compiler change. Parser adds the `@vcs.*` family to the existing decorator grammar (no new bare keyword). HIR gains a `VcsEffect` annotation per fn that records the effect set: `{ ReadOnly, Requires(CapabilityKind), Linear(CapabilityKind), AuditTrail }`. The type checker propagates effects up the call graph: any fn that internally calls `requires(T)` must itself declare `requires(T)` or be the call site that holds the cap. Linearity is checked separately: a cap argument annotated `@vcs.linear_working_tree` is consumed (moved) on first use; second use is a type error. Codegen lowers each `@vcs.requires(T)` to a Rust fn that takes `cap: T` as a parameter, using the existing capability types from `vox-orchestrator-types`.

**Tech stack:** Existing Vox compiler crates (`vox-compiler` for lex/parse/HIR/typecheck, `vox-codegen` for lowering). No new dependencies. Phase 4 specifically depends on the `@durable` / `@endpoint` infrastructure already merged — those decorators set the precedent for HIR-level effect annotation and codegen-side parameter injection. **If `@durable` does not yet land effect annotations in HIR, Phase 4 is blocked until it does.** Verify before starting.

**Out of scope for Phase 4:**
- Backend swap of `git_exec` to `gix` or `jj-lib` (Phase 5).
- Cross-crate import of `@vcs.*` decorated fns from outside the workspace (no module-path resolution rules added; intra-crate only for MVP).
- Deny-by-default: a fn without any `@vcs.*` decorator that internally calls a write-side primitive is a hard error in Phase 4. Inferring the necessary annotation is a possible Phase 4.5 but not in this plan.

---

## Verification setup

- `cargo test -p vox-compiler --lib` — parser/HIR/typecheck tests.
- `cargo test -p vox-codegen --lib` — lowering tests.
- `cargo test -p vox-compiler --test vcs_decorators` — integration test that compiles fixture `.vox` files.
- `vox check scripts/vcs/wip.vox` (and the other three from Phase 2) — these stop needing `// vox:skip` after this phase.
- `cargo run -p vox-arch-check` — must remain green.

The plan produces 8 commits.

---

## Pre-flight: confirm @durable's HIR plumbing exists

Phase 4 reuses the same effect-annotation pipeline as `@durable`. Before starting:

```
rg "DurableEffect" crates/vox-compiler/src/
rg "@durable" crates/vox-compiler/src/  # find the existing parser hook
```

If neither lands HIR-level annotations, **stop**. Open an issue noting that Phase 4 of agentic-VCS depends on Phase N of the GUI-Native Language Roadmap landing decorator-on-fn type checking; do not proceed.

---

## Task 1: Parser — recognise @vcs.read_only / @vcs.requires(T) / @vcs.linear_working_tree / @vcs.audit_trail

**Files:**
- Modify: `crates/vox-compiler/src/parser/decorators.rs` (or wherever `@durable` parsing lives)
- Modify: `crates/vox-compiler/src/parser/grammar.lalrpop` (if LALRPOP-based)
- Test: `crates/vox-compiler/src/parser/decorators_tests.rs`

- [ ] **Step 1: Write parser tests for each decorator form**

```rust
#[test]
fn parses_at_vcs_read_only() {
    let src = "@vcs.read_only fn list_recent_changes() -> Vec<Change> { todo!() }";
    let f = parse_fn_decl(src).unwrap();
    assert!(f.decorators.iter().any(|d| matches!(d, Decorator::VcsReadOnly)));
}

#[test]
fn parses_at_vcs_requires_single_cap() {
    let src = "@vcs.requires(WorkingTreeWrite) fn stage(cap: WorkingTreeWrite) -> CommitId { todo!() }";
    let f = parse_fn_decl(src).unwrap();
    assert!(f.decorators.iter().any(|d|
        matches!(d, Decorator::VcsRequires(t) if t.name() == "WorkingTreeWrite")
    ));
}

#[test]
fn parses_at_vcs_requires_multiple() {
    let src = r#"@vcs.requires(BranchCreate)
@vcs.requires(PushAllowed)
fn promote(b: BranchCreate, p: PushAllowed) -> Url { todo!() }"#;
    let f = parse_fn_decl(src).unwrap();
    let n = f.decorators.iter().filter(|d| matches!(d, Decorator::VcsRequires(_))).count();
    assert_eq!(n, 2);
}

#[test]
fn parses_at_vcs_linear_working_tree() {
    let src = "@vcs.linear_working_tree fn finish(wt: WorkingTreeWrite) -> () { todo!() }";
    let f = parse_fn_decl(src).unwrap();
    assert!(f.decorators.iter().any(|d| matches!(d, Decorator::VcsLinearWorkingTree)));
}

#[test]
fn parses_at_vcs_audit_trail() {
    let src = "@vcs.audit_trail fn push() -> () { todo!() }";
    let f = parse_fn_decl(src).unwrap();
    assert!(f.decorators.iter().any(|d| matches!(d, Decorator::VcsAuditTrail)));
}

#[test]
fn rejects_unknown_at_vcs_subkey() {
    let src = "@vcs.bogus fn x() {}";
    assert!(parse_fn_decl(src).is_err(), "unknown @vcs.* subkey must be a parse error, not silently ignored");
}
```

- [ ] **Step 2: Run tests — should fail**

Run: `cargo test -p vox-compiler --lib parser::decorators_tests`
Expected: FAIL — `Decorator::VcsReadOnly` etc. don't exist.

- [ ] **Step 3: Add the decorator variants to the AST**

In `crates/vox-compiler/src/ast/decorator.rs` (or wherever the `Decorator` enum lives):

```rust
pub enum Decorator {
    // … existing variants …
    VcsReadOnly,
    VcsRequires(TypePath),          // e.g. WorkingTreeWrite
    VcsLinearWorkingTree,
    VcsAuditTrail,
}
```

`TypePath` is the existing path AST node used by other decorators that take a type argument (e.g. `@durable(...)` if applicable).

- [ ] **Step 4: Extend the parser**

Match the structure of the existing `@durable` parser hook. The grammar admits `@vcs.<ident>` and optionally `(...)`:

```rust
fn parse_vcs_decorator(p: &mut Parser) -> Result<Decorator, ParseError> {
    p.expect(Token::At)?;
    p.expect(Token::Ident("vcs"))?;
    p.expect(Token::Dot)?;
    let sub = p.expect_ident()?;
    match sub.as_str() {
        "read_only" => Ok(Decorator::VcsReadOnly),
        "linear_working_tree" => Ok(Decorator::VcsLinearWorkingTree),
        "audit_trail" => Ok(Decorator::VcsAuditTrail),
        "requires" => {
            p.expect(Token::LParen)?;
            let ty = p.parse_type_path()?;
            p.expect(Token::RParen)?;
            Ok(Decorator::VcsRequires(ty))
        }
        other => Err(ParseError::UnknownVcsDecorator(other.to_string())),
    }
}
```

- [ ] **Step 5: Run tests**

Expected: PASS — 6/6.

- [ ] **Step 6: Commit**

```
git add crates/vox-compiler/src/ast/decorator.rs crates/vox-compiler/src/parser/decorators.rs crates/vox-compiler/src/parser/decorators_tests.rs
git commit -m "feat(vox-compiler): parse @vcs.read_only / @vcs.requires(T) / @vcs.linear_working_tree / @vcs.audit_trail"
```

---

## Task 2: HIR — VcsEffect annotation per fn

**Files:**
- Modify: `crates/vox-compiler/src/hir/mod.rs` — extend `FnDef` with `vcs_effects: VcsEffectSet`
- Create: `crates/vox-compiler/src/hir/vcs_effect.rs`
- Test: same crate

- [ ] **Step 1: Define VcsEffect / VcsEffectSet**

```rust
// crates/vox-compiler/src/hir/vcs_effect.rs

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VcsEffect {
    ReadOnly,
    Requires(CapabilityName),
    Linear(CapabilityName),
    AuditTrail,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityName(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VcsEffectSet {
    inner: BTreeSet<VcsEffect>,
}

impl VcsEffectSet {
    pub fn add(&mut self, e: VcsEffect) { self.inner.insert(e); }
    pub fn contains(&self, e: &VcsEffect) -> bool { self.inner.contains(e) }
    pub fn is_read_only(&self) -> bool {
        self.inner.contains(&VcsEffect::ReadOnly) && self.required_caps().next().is_none()
    }
    pub fn required_caps(&self) -> impl Iterator<Item = &CapabilityName> {
        self.inner.iter().filter_map(|e| match e { VcsEffect::Requires(n) => Some(n), _ => None })
    }
    pub fn linear_caps(&self) -> impl Iterator<Item = &CapabilityName> {
        self.inner.iter().filter_map(|e| match e { VcsEffect::Linear(n) => Some(n), _ => None })
    }
    pub fn iter(&self) -> impl Iterator<Item = &VcsEffect> { self.inner.iter() }
}

impl FromIterator<VcsEffect> for VcsEffectSet {
    fn from_iter<I: IntoIterator<Item = VcsEffect>>(iter: I) -> Self {
        Self { inner: iter.into_iter().collect() }
    }
}
```

- [ ] **Step 2: Lower decorators to HIR effects**

In `crates/vox-compiler/src/hir/lower.rs` (or wherever decorator-to-HIR happens), extend the lowering:

```rust
fn lower_vcs_decorators(decorators: &[Decorator]) -> VcsEffectSet {
    let mut set = VcsEffectSet::default();
    for d in decorators {
        match d {
            Decorator::VcsReadOnly => set.add(VcsEffect::ReadOnly),
            Decorator::VcsRequires(ty) => set.add(VcsEffect::Requires(CapabilityName(ty.name().to_string()))),
            Decorator::VcsLinearWorkingTree => {
                // linear_working_tree is shorthand for Linear(WorkingTreeWrite).
                set.add(VcsEffect::Linear(CapabilityName("WorkingTreeWrite".into())));
            }
            Decorator::VcsAuditTrail => set.add(VcsEffect::AuditTrail),
            _ => {}
        }
    }
    set
}
```

Wire `vcs_effects: lower_vcs_decorators(&fn_def.decorators)` onto the HIR `FnDef`.

- [ ] **Step 3: Tests**

```rust
#[test]
fn lowers_read_only_decorator_to_effect_set() {
    let src = "@vcs.read_only fn x() {}";
    let hir = lower(src).unwrap();
    assert!(hir.fns[0].vcs_effects.is_read_only());
}

#[test]
fn lowers_requires_decorator_to_effect_set() {
    let src = "@vcs.requires(WorkingTreeWrite) fn x(cap: WorkingTreeWrite) {}";
    let hir = lower(src).unwrap();
    let names: Vec<_> = hir.fns[0].vcs_effects.required_caps().map(|n| n.0.clone()).collect();
    assert_eq!(names, vec!["WorkingTreeWrite".to_string()]);
}

#[test]
fn linear_working_tree_lowers_to_linear_workingtreewrite() {
    let src = "@vcs.linear_working_tree fn x(cap: WorkingTreeWrite) {}";
    let hir = lower(src).unwrap();
    let names: Vec<_> = hir.fns[0].vcs_effects.linear_caps().map(|n| n.0.clone()).collect();
    assert_eq!(names, vec!["WorkingTreeWrite".to_string()]);
}
```

- [ ] **Step 4: Commit**

```
git add crates/vox-compiler/src/hir/vcs_effect.rs crates/vox-compiler/src/hir/lower.rs crates/vox-compiler/src/hir/mod.rs
git commit -m "feat(vox-compiler): lower @vcs.* decorators to VcsEffectSet on HIR FnDef"
```

---

## Task 3: Type checker — read-only cannot call requires

**Files:**
- Modify: `crates/vox-compiler/src/typecheck/effects.rs` (create if absent)
- Test: same file

The rule: if fn `f` has `VcsEffect::ReadOnly` in its effect set, every fn `g` it calls must have an effect set that is a subset of `f`'s — specifically, `g` must not have any `VcsEffect::Requires(_)`.

- [ ] **Step 1: Tests**

```rust
#[test]
fn read_only_calling_requires_is_a_type_error() {
    let src = r#"
        @vcs.requires(WorkingTreeWrite)
        fn write(cap: WorkingTreeWrite) -> () {}

        @vcs.read_only
        fn read() -> () { write(/* cap */) }
    "#;
    let result = typecheck(src);
    assert!(matches!(result, Err(TypeError::ReadOnlyCallsWriteSide { .. })));
}

#[test]
fn read_only_calling_read_only_is_fine() {
    let src = r#"
        @vcs.read_only fn a() -> () {}
        @vcs.read_only fn b() -> () { a() }
    "#;
    assert!(typecheck(src).is_ok());
}

#[test]
fn requires_calling_requires_with_same_cap_is_fine() {
    let src = r#"
        @vcs.requires(WorkingTreeWrite)
        fn inner(cap: WorkingTreeWrite) -> () {}

        @vcs.requires(WorkingTreeWrite)
        fn outer(cap: WorkingTreeWrite) -> () { inner(cap) }
    "#;
    assert!(typecheck(src).is_ok());
}

#[test]
fn fn_without_decorator_calling_requires_is_a_type_error() {
    let src = r#"
        @vcs.requires(WorkingTreeWrite)
        fn write(cap: WorkingTreeWrite) -> () {}

        fn unannotated() -> () { write(/* cap */) }
    "#;
    let result = typecheck(src);
    assert!(matches!(result, Err(TypeError::UnannotatedCallsWriteSide { .. })));
}
```

- [ ] **Step 2: Implementation**

```rust
//! Effect propagation rules for @vcs.* annotations.

use crate::hir::{FnDef, HirProgram, VcsEffect};

#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    #[error("@vcs.read_only fn {caller} calls write-side fn {callee}")]
    ReadOnlyCallsWriteSide { caller: String, callee: String },
    #[error("unannotated fn {caller} calls @vcs.requires fn {callee}; annotate {caller} with @vcs.requires(...) or refactor")]
    UnannotatedCallsWriteSide { caller: String, callee: String },
    // … other variants for linearity (Task 4) …
}

pub fn check_vcs_effects(prog: &HirProgram) -> Result<(), TypeError> {
    for f in &prog.fns {
        check_fn(f, prog)?;
    }
    Ok(())
}

fn check_fn(f: &FnDef, prog: &HirProgram) -> Result<(), TypeError> {
    for call in f.body.calls() {
        let callee = prog.find_fn(&call.target_name)
            .ok_or_else(|| /* unresolved call — report or skip; up to existing typecheck */ unreachable!())?;

        if f.vcs_effects.is_read_only() && !callee.vcs_effects.required_caps().next().is_none() {
            return Err(TypeError::ReadOnlyCallsWriteSide {
                caller: f.name.clone(),
                callee: callee.name.clone(),
            });
        }

        if f.vcs_effects.iter().count() == 0 && callee.vcs_effects.required_caps().next().is_some() {
            return Err(TypeError::UnannotatedCallsWriteSide {
                caller: f.name.clone(),
                callee: callee.name.clone(),
            });
        }
    }
    Ok(())
}
```

`f.body.calls()` is the existing HIR walker that yields `Call { target_name, args }` for every call expression. If it does not exist, add it; the existing typecheck must already do something similar for type inference.

- [ ] **Step 3: Run tests**

Expected: PASS — 4/4.

- [ ] **Step 4: Commit**

```
git add crates/vox-compiler/src/typecheck/effects.rs
git commit -m "feat(vox-compiler): typecheck rule — @vcs.read_only fn cannot call write-side fn; unannotated cannot call @vcs.requires"
```

---

## Task 4: Linearity — @vcs.linear_working_tree consumed at most once

**Files:**
- Modify: `crates/vox-compiler/src/typecheck/effects.rs`
- Test: same file

Linearity rule: when a fn is `@vcs.linear_working_tree`, the parameter typed `WorkingTreeWrite` is consumed (moved) on first use. A second use within the same scope is a type error.

This is essentially the existing affine-type machinery applied with the added trigger that `@vcs.linear_working_tree` *upgrades* the cap parameter from regular (clonable) to linear (move-only).

- [ ] **Step 1: Tests**

```rust
#[test]
fn linear_cap_used_twice_is_type_error() {
    let src = r#"
        @vcs.requires(WorkingTreeWrite) fn use1(c: WorkingTreeWrite) {}
        @vcs.requires(WorkingTreeWrite) fn use2(c: WorkingTreeWrite) {}
        @vcs.linear_working_tree
        @vcs.requires(WorkingTreeWrite)
        fn outer(c: WorkingTreeWrite) {
            use1(c);
            use2(c);   // ERROR: c already moved
        }
    "#;
    let result = typecheck(src);
    assert!(matches!(result, Err(TypeError::LinearCapReused { .. })));
}

#[test]
fn linear_cap_used_once_is_fine() {
    let src = r#"
        @vcs.requires(WorkingTreeWrite) fn use1(c: WorkingTreeWrite) {}
        @vcs.linear_working_tree
        @vcs.requires(WorkingTreeWrite)
        fn outer(c: WorkingTreeWrite) {
            use1(c);
        }
    "#;
    assert!(typecheck(src).is_ok());
}

#[test]
fn non_linear_cap_can_be_passed_twice_via_clone() {
    let src = r#"
        @vcs.requires(WorkingTreeWrite) fn use1(c: WorkingTreeWrite) {}
        @vcs.requires(WorkingTreeWrite) fn use2(c: WorkingTreeWrite) {}
        @vcs.requires(WorkingTreeWrite)
        fn outer(c: WorkingTreeWrite) {
            use1(c.clone());
            use2(c);
        }
    "#;
    // No @vcs.linear_working_tree → standard ownership rules apply.
    assert!(typecheck(src).is_ok());
}
```

- [ ] **Step 2: Implementation**

Reuse the existing affine/linear machinery (the language presumably has it for other purposes; if not, this is the place to add a minimal version). Mark each parameter listed under a `@vcs.linear_working_tree` fn with the linear flag in the symbol table; the existing borrow check then rejects second use.

```rust
fn mark_linear_caps(f: &mut FnDef) {
    for cap_name in f.vcs_effects.linear_caps() {
        for param in f.params.iter_mut() {
            if param.ty.name() == cap_name.0 {
                param.linearity = Linearity::Linear;
            }
        }
    }
}
```

- [ ] **Step 3: Run tests**

Expected: PASS — 3/3.

- [ ] **Step 4: Commit**

```
git add crates/vox-compiler/src/typecheck/effects.rs crates/vox-compiler/src/hir/
git commit -m "feat(vox-compiler): @vcs.linear_working_tree marks cap param linear; second use is type error"
```

---

## Task 5: Codegen — lower @vcs.requires(T) to Rust 'cap: T' parameter

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/fn_lower.rs` (or wherever fn lowering lives)
- Test: same crate

The lowering: a Vox fn `@vcs.requires(WorkingTreeWrite) fn f(cap: WorkingTreeWrite, …)` becomes a Rust fn `fn f(cap: WorkingTreeWrite, …)` where `WorkingTreeWrite` resolves to `vox_orchestrator_types::WorkingTreeWrite`. The decorator does not need to add anything at the Rust level — it has already done its job at type-check time.

The `@vcs.audit_trail` decorator inserts a `tracing::info!(target: "vox.vcs.<fn_name>", …)` event at the start of the fn body in the generated Rust.

- [ ] **Step 1: Tests**

```rust
#[test]
fn audit_trail_decorator_emits_tracing_event() {
    let src = "@vcs.audit_trail fn push() -> () { return; }";
    let rust = lower_to_rust(src).unwrap();
    assert!(rust.contains(r#"tracing::info!(target: "vox.vcs.push""#),
        "expected tracing::info! emission, got:\n{}", rust);
}

#[test]
fn requires_decorator_does_not_inject_extra_param() {
    // The cap param is already declared in the Vox source; codegen does not double it.
    let src = "@vcs.requires(WorkingTreeWrite) fn f(cap: WorkingTreeWrite) -> () { return; }";
    let rust = lower_to_rust(src).unwrap();
    assert_eq!(rust.matches("cap: WorkingTreeWrite").count(), 1);
}
```

- [ ] **Step 2: Implementation**

```rust
fn lower_fn_with_vcs_effects(f: &FnDef, out: &mut RustOutput) {
    out.write_decorators_as_attrs(&f.decorators);   // existing
    out.write_signature(&f);                        // existing

    out.open_body();

    if f.vcs_effects.iter().any(|e| matches!(e, VcsEffect::AuditTrail)) {
        out.writeln(format!(
            "tracing::info!(target: \"vox.vcs.{}\", \"audit\");",
            f.name
        ));
    }

    out.write_body_stmts(&f.body);
    out.close_body();
}
```

- [ ] **Step 3: Run tests**

Expected: PASS — 2/2.

- [ ] **Step 4: Commit**

```
git add crates/vox-codegen/src/codegen_rust/fn_lower.rs
git commit -m "feat(vox-codegen): emit tracing::info! for @vcs.audit_trail; @vcs.requires(T) lowers to existing param shape"
```

---

## Task 6: Harden Phase 1 mint methods to pub(crate) + sealed trait

**Files:**
- Modify: `crates/vox-orchestrator-types/src/vcs_capability.rs`
- Create: `crates/vox-orchestrator-types/src/vcs_capability_seal.rs`
- Modify: `crates/vox-orchestrator-types/src/lib.rs`

Phase 1 left the mint methods as `pub` + `#[doc(hidden)]` (soft-private) deliberately, with a note that Phase 4 hardens them. Now we do it: change visibility to `pub(crate)` and add a sealed trait that the orchestrator's `authorize_*` shims depend on so they can still mint capabilities through a controlled boundary.

- [ ] **Step 1: Add the sealed trait**

```rust
// crates/vox-orchestrator-types/src/vcs_capability_seal.rs
//! Sealed trait that allows authorised crates to mint capabilities.
//! "Sealed" means downstream crates cannot impl this trait — only this
//! crate impls it for the cap types. The trait is re-exported through
//! `vox-orchestrator-internal-mint`, a thin facade crate that the
//! orchestrator depends on; no other crate may depend on that facade.

use crate::{
    BranchCreate, BranchName, DestructiveKind, DestructiveOp, ForcePushAllowed,
    PushAllowed, RemoteId, WorkingTreeWrite, WorkspaceId,
};

mod private {
    pub trait Sealed {}
}

pub trait CapabilityMint: private::Sealed {
    type Args;
    fn mint_via(args: Self::Args) -> Self;
}

impl private::Sealed for WorkingTreeWrite {}
impl CapabilityMint for WorkingTreeWrite {
    type Args = (WorkspaceId, BranchName);
    fn mint_via((workspace, branch): Self::Args) -> Self {
        Self { workspace, branch }       // private fields, same crate, OK
    }
}

// … similar Sealed + CapabilityMint impls for the other 4 capability types …
```

In `vcs_capability.rs`, change every `mint` method from:

```rust
#[doc(hidden)]
pub fn mint(...) -> Self { ... }
```

to:

```rust
pub(crate) fn mint(...) -> Self { ... }
```

This breaks any current callers outside `vox-orchestrator-types`. The orchestrator's `authorize_*` shims (Phase 2 Task 9) called `mint` directly — they now need to use `CapabilityMint::mint_via` and depend on the sealed trait.

Add a tiny new crate `crates/vox-orchestrator-internal-mint/` that re-exports `CapabilityMint`:

```toml
# crates/vox-orchestrator-internal-mint/Cargo.toml
[package]
name = "vox-orchestrator-internal-mint"
version.workspace = true
edition.workspace = true

[dependencies]
vox-orchestrator-types.workspace = true
```

```rust
// crates/vox-orchestrator-internal-mint/src/lib.rs
pub use vox_orchestrator_types::vcs_capability_seal::CapabilityMint;
```

Add `vox-orchestrator-internal-mint` as a `[dependencies]` of `vox-orchestrator` (only). Update the `authorize_*` shims to use `CapabilityMint::mint_via`.

Add a `vox-arch-check` rule `no_internal_mint_dep_outside_orchestrator`: any crate other than `vox-orchestrator` that depends on `vox-orchestrator-internal-mint` is a layer violation.

- [ ] **Step 2: Tests**

Confirm the existing capability tests still pass:

Run: `cargo test -p vox-orchestrator-types --lib`

Add a test in `vox-orchestrator-internal-mint`:

```rust
#[test]
fn mint_via_round_trips() {
    use vox_orchestrator_types::{BranchName, WorkspaceId};
    use vox_orchestrator_internal_mint::CapabilityMint;
    use vox_orchestrator_types::WorkingTreeWrite;

    let cap = WorkingTreeWrite::mint_via((WorkspaceId(1), BranchName::parse("agent/x").unwrap()));
    assert_eq!(cap.workspace(), WorkspaceId(1));
}
```

- [ ] **Step 3: Run the full workspace tests + arch-check**

Run: `cargo test --workspace`
Run: `cargo run -p vox-arch-check`

Expected: PASS for both. Any broken callsite outside `vox-orchestrator` is the deliberate effect of the hardening — fix by routing through `authorize_*`.

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-types/src/vcs_capability.rs crates/vox-orchestrator-types/src/vcs_capability_seal.rs crates/vox-orchestrator-types/src/lib.rs crates/vox-orchestrator-internal-mint/ crates/vox-arch-check/src/main.rs
git commit -m "feat(vcs): harden capability mint to pub(crate) + sealed trait via vox-orchestrator-internal-mint facade"
```

---

## Task 7: Update scripts/vcs/*.vox to remove // vox:skip

**Files:**
- Modify: `scripts/vcs/wip.vox`, `sync.vox`, `finish.vox`, `recover.vox`

Phase 2 created these with `// vox:skip` annotations because the decorators didn't exist yet. After Tasks 1–5, they type-check.

- [ ] **Step 1: Remove the // vox:skip lines**

For each file: delete the `// vox:skip` line at the top.

- [ ] **Step 2: Run vox check on each**

```
vox check scripts/vcs/wip.vox
vox check scripts/vcs/sync.vox
vox check scripts/vcs/finish.vox
vox check scripts/vcs/recover.vox
```

Expected: PASS — all four. If any fail because of an undefined fn (e.g. `vox_git_fetch` from sync.vox), they were not in Phase 2 scope; the failure is correct and the file should keep the `// vox:skip` for that one until Phase 2.5 lands the missing tool.

- [ ] **Step 3: Commit**

```
git add scripts/vcs/
git commit -m "chore(vox-scripts): drop // vox:skip from VCS scripts now that @vcs.* decorators land"
```

---

## Task 8: Documentation

**Files:**
- Modify: `docs/src/architecture/git-concurrency-policy.md` — append "Language-level enforcement"
- Modify: `docs/src/architecture/where-things-live.md` — add row for `@vcs.*` decorators
- Modify: AGENTS.md? (only if existing decorator docs there need an update)

- [ ] **Step 1: git-concurrency-policy.md addition**

Append:

```markdown
## Language-level enforcement (Phase 4)

The Rust capability tokens are the runtime substrate. The Vox compiler
enforces correctness at the source level via `@vcs.*` decorators on `fn`:

| Decorator | Meaning |
|---|---|
| `@vcs.read_only` | This fn cannot transitively call any `@vcs.requires(_)` fn |
| `@vcs.requires(T)` | This fn requires the caller to hold a `T` capability |
| `@vcs.linear_working_tree` | The `WorkingTreeWrite` parameter is consumed (moved) on first use; no aliasing |
| `@vcs.audit_trail` | Codegen inserts a `tracing::info!(target: "vox.vcs.<fn_name>")` event |

These are decorators on `fn`, not bare keywords (per AGENTS.md grammar
policy). They lower to no extra Rust parameters — the cap is already a
parameter in the Vox source, and the decorator is the *static* contract
that the type checker enforces.

The Phase 1 mint methods are now `pub(crate)`; minting outside
`vox-orchestrator` requires the `CapabilityMint` sealed trait re-exported
through `vox-orchestrator-internal-mint` (a facade crate that only the
orchestrator depends on, enforced by an arch-check rule).
```

- [ ] **Step 2: where-things-live.md row**

```
| Add a Vox-level VCS effect annotation | `@vcs.<name>` on a `fn` declaration; supported names: read_only, requires(T), linear_working_tree, audit_trail. Add new variants in `crates/vox-compiler/src/parser/decorators.rs` and `crates/vox-compiler/src/hir/vcs_effect.rs`. |
```

- [ ] **Step 3: Regenerate**

```
cargo run -p vox-doc-pipeline
cargo run -p vox-doc-pipeline -- --check
```

- [ ] **Step 4: Commit**

```
git add docs/src/architecture/git-concurrency-policy.md docs/src/architecture/where-things-live.md
git commit -m "docs(vcs): document Phase 4 @vcs.* decorator surface and capability-mint hardening"
git add docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "chore(docs): regenerate SUMMARY.md / architecture-index.md / feed.xml"
```

---

## Phase 4 acceptance criteria

- [ ] `cargo test -p vox-compiler --lib` passes; new tests cover parsing, lowering, type-check rules, linearity.
- [ ] `cargo test -p vox-codegen --lib` passes; new tests cover audit_trail emission and requires-decorator codegen.
- [ ] `cargo test --workspace` passes (i.e. Task 6 hardening did not break any consumer).
- [ ] `cargo run -p vox-arch-check` passes (new rule `no_internal_mint_dep_outside_orchestrator` registered and green).
- [ ] `cargo run -p vox-doc-pipeline -- --check` passes.
- [ ] `vox check scripts/vcs/{wip,finish,recover}.vox` passes without `// vox:skip` (sync.vox may still need it for missing Phase 2.5 tools — document inline).
- [ ] All 8 commits land per the per-task templates.

---

## Notes for the implementing engineer

- **Task 6 is the riskiest in Phase 4.** It hardens a soft-private boundary that was deliberately permissive in Phase 1. Expect to fix several callsites in `vox-orchestrator` that called `mint` directly. Fix them by routing through `authorize_*`. If you find a callsite that *cannot* route through `authorize_*` (e.g. a test that needs to construct a cap by hand), add a `#[cfg(test)]` constructor to the cap type. Resist the urge to widen visibility back to `pub`.
- **The internal-mint facade crate is a concession to Rust's coarse visibility.** A more principled solution is a Rust `#[expose_to(crate)]` attribute that doesn't exist. The facade is the standard workaround; `vox-arch-check` enforces the layering.
- **Linearity is checked at the HIR level, not lowered to Rust's affine types.** The Vox compiler is the gate; the Rust output uses ordinary owned values (which Rust's borrow checker also ensures aren't reused after move). If both checks fail in different ways, the Vox-level check is canonical and the Rust output should be regenerated.
- **`@vcs.audit_trail` adds a `tracing::info!` at fn entry, NOT at every call site.** The dashboard in Phase 3 sees these events through the `vox.vcs.*` broadcast. Don't add an exit-side event in Phase 4 — operators don't need both, and exit-side events conflate with successful return vs panic. Keep it entry-only.
- **`@vcs.requires(T)` does NOT auto-inject a `cap: T` parameter.** The fn must already declare it. Phase 4 specifically rejects auto-injection so that Vox source remains literal — no hidden parameters. Compile error if `@vcs.requires(T)` decorates a fn that doesn't have a parameter of type `T`.
- **Phase 4 makes the Vox source the unforgeable origin, but the Rust output is still mechanical.** Resist any clever lowering that does more than the documented contract — codegen stays predictable and reviewable.
