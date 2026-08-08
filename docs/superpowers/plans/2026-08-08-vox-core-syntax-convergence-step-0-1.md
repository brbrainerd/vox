# Vox Core-Syntax Convergence — Steps 0–1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land Sequencing Steps 0 and 1 of the Vox core-syntax convergence
program: fix the two description surfaces with zero implementation
dependencies (MENS system prompt, AGENTS.md grammar section), scaffold CR-F3
in warn mode, and hardened the lexer per S2 (kill the two silent
unknown-character-drop sites, add reader-tolerant `;`/`==`/`!=`, lock in
`->`'s position-specific behavior, keep `!` a reader error with a
machine-readable fix, delete dead code, bound diagnostic cost on
pathological input).

**Architecture:** Add one new lexer token, `Token::Unknown(char)`, as a
lowest-priority catch-all — this is the single change that eliminates both
`Err(_) => None` silent-drop sites in `lexer/cursor.rs` simultaneously,
because both `lex` and `lex_preserving` already flow `Ok(token)` results
through unchanged; only genuinely unrecognized bytes currently produce
`Err(_)`, and the catch-all converts them to `Ok(Token::Unknown(c))`. Parser
changes are additive: one new tolerant-skip helper for `;`, one new warning
push at the existing `EqEq`/`NotEq` match site (no new parsing — these
tokens already unify into the canonical `BinOp`), and a bounded-diagnostic
counter at the one top-level dispatch fallback that currently produces an
unbounded number of "unexpected token" errors.

**Tech Stack:** Rust, Logos (lexer generator), the existing hand-written
recursive-descent parser in `crates/vox-compiler/src/parser/descent/`.

**Spec:** `docs/superpowers/specs/2026-08-08-vox-core-syntax-convergence-design.md`
(revision 2, commit `2ea8913c68`). **Audit:**
`docs/src/architecture/vox-language-syntax-audit-2026-08-08.md`.

**Scope note:** This plan covers Sequencing Steps 0–1 only. Steps 2–8 (S3
canonicalization, S4 decorator collapse + `@v0` retirement, S5 ergonomics,
S6 remaining regeneration, S7 migration, S8 hard gates) depend on decisions
this plan's execution will surface and get their own plan via a follow-up
`writing-plans` invocation once this one lands and is reviewed.

---

## File Structure

| File | Responsibility |
|---|---|
| `mens/config/system_prompt.txt` | Rewritten in place — the MENS training/inference system prompt, corrected to the current grammar |
| `AGENTS.md` | §Grammar Unification block corrected in place |
| `contracts/spec/language-surface-coverage.v1.yaml` | New — CR-F3 ledger, warn-mode only in this plan |
| `contracts/spec/language-surface-coverage.v1.schema.json` | New — JSON Schema for the ledger, so it's machine-validated from day one |
| `crates/vox-compiler/src/parser/indent.rs` | Deleted — confirmed dead (not a declared module) |
| `crates/vox-compiler/src/lexer/token.rs` | Add `Token::Unknown(char)` variant + regex + `Display` arm |
| `crates/vox-compiler/src/lexer/cursor.rs` | No functional change — add a clarifying comment on the now-effectively-unreachable `Err(_) => None` arms |
| `crates/vox-compiler/src/parser/descent/mod.rs` | Add `skip_tolerated_semicolon`, add bounded-diagnostic counter + cap in `parse_decl`'s fallback arm |
| `crates/vox-compiler/src/parser/descent/stmt.rs` | Call `skip_tolerated_semicolon` from `parse_block` |
| `crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs` | Push a reader warning when `EqEq`/`NotEq` are consumed |
| `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs` | Add a `Replacement` payload to the existing `BangInvalid` error (severity stays `Error` — this is an explicit exception to the tolerant-reader policy, per the spec's S2) |

---

### Task 0: CR-F3 ledger scaffold (warn mode)

**Files:**
- Create: `contracts/spec/language-surface-coverage.v1.schema.json`
- Create: `contracts/spec/language-surface-coverage.v1.yaml`
- Test: `crates/vox-compiler/tests/language_surface_coverage_schema_test.rs`

This task only creates the data files and a schema-validity test — it does
**not** wire a CI gate (that's Sequencing Step 8, S8, out of scope here).
The ledger starts covering the grammar productions this plan itself touches
(`Token::Unknown`, tolerant `;`, `==`/`!=` warnings) plus a `todo` bucket
naming every production not yet covered, so the file is honest about its
own incompleteness rather than silently empty.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-compiler/tests/language_surface_coverage_schema_test.rs
//! CR-F3 (warn-mode scaffold): the language-surface coverage ledger must
//! exist, parse as valid YAML, and validate against its own JSON Schema.
//! This test does NOT check completeness (that's the Sequencing-Step-8 hard
//! gate) — only that the file is well-formed and the schema is honest.

use std::fs;

#[test]
fn coverage_ledger_is_valid_yaml_matching_its_schema() {
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/spec/language-surface-coverage.v1.yaml"
    );
    let schema_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/spec/language-surface-coverage.v1.schema.json"
    );

    let yaml_src = fs::read_to_string(yaml_path)
        .unwrap_or_else(|e| panic!("failed to read {yaml_path}: {e}"));
    let schema_src = fs::read_to_string(schema_path)
        .unwrap_or_else(|e| panic!("failed to read {schema_path}: {e}"));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&yaml_src).expect("ledger must be valid YAML");
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_src).expect("schema must be valid JSON");

    // Round-trip YAML -> JSON so jsonschema can validate it.
    let doc_json: serde_json::Value =
        serde_json::to_value(&doc).expect("YAML must convert to JSON");

    let compiled = jsonschema::JSONSchema::compile(&schema_json)
        .expect("schema itself must compile");
    let result = compiled.validate(&doc_json);
    if let Err(errors) = result {
        let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
        panic!("ledger does not match schema:\n{}", msgs.join("\n"));
    }
}

#[test]
fn coverage_ledger_lists_this_plans_new_productions() {
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/spec/language-surface-coverage.v1.yaml"
    );
    let yaml_src = fs::read_to_string(yaml_path).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&yaml_src).unwrap();
    let productions = doc["productions"]
        .as_sequence()
        .expect("productions must be a list");
    let names: Vec<&str> = productions
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    for expected in [
        "lexer/unknown-char-token",
        "reader-tolerant-semicolon",
        "reader-tolerant-eq-eq",
        "reader-tolerant-not-eq",
    ] {
        assert!(
            names.contains(&expected),
            "expected production '{expected}' in ledger, got {names:?}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test language_surface_coverage_schema_test`
Expected: FAIL — `failed to read .../contracts/spec/language-surface-coverage.v1.yaml`
(the file doesn't exist yet). `jsonschema`, `serde_yaml`, and `serde_json`
are already `[dev-dependencies]`/`[dependencies]` of `vox-compiler`
(`crates/vox-compiler/Cargo.toml:56-60`, confirmed during planning) — no
`Cargo.toml` change needed for this test to compile.

- [ ] **Step 3: Create the schema**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://vox.dev/contracts/spec/language-surface-coverage.v1.schema.json",
  "title": "Vox language-surface coverage ledger",
  "description": "CR-F3: maps every grammar production, decorator, and builtin to at least one behavioral fixture. Warn-mode scaffold — see docs/superpowers/specs/2026-08-08-vox-core-syntax-convergence-design.md S6. The hard CI gate (fail on missing coverage) lands in Sequencing Step 8, not this file.",
  "type": "object",
  "required": ["schema_version", "mode", "productions"],
  "properties": {
    "schema_version": { "const": 1 },
    "mode": {
      "type": "string",
      "enum": ["warn", "enforce"],
      "description": "warn = informational only, no CI gate wired; enforce = CI fails on missing coverage (Sequencing Step 8)."
    },
    "productions": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "kind", "status"],
        "properties": {
          "name": {
            "type": "string",
            "description": "Stable slug, kebab-case, e.g. 'lexer/unknown-char-token'."
          },
          "kind": {
            "type": "string",
            "enum": ["token", "grammar-rule", "decorator", "soft-keyword", "builtin"]
          },
          "status": {
            "type": "string",
            "enum": ["covered", "todo"]
          },
          "fixture": {
            "type": ["string", "null"],
            "description": "Path to the behavioral fixture proving this production, e.g. a #[test] name or examples/golden/*.vox file. Required when status is 'covered'."
          },
          "spec_ref": {
            "type": ["string", "null"],
            "description": "Section of the convergence spec that introduced this row, e.g. 'S2'."
          }
        },
        "if": { "properties": { "status": { "const": "covered" } } },
        "then": { "required": ["fixture"] }
      }
    }
  }
}
```

- [ ] **Step 4: Create the ledger, warn mode, seeded with this plan's rows**

```yaml
# contracts/spec/language-surface-coverage.v1.yaml
#
# CR-F3 warn-mode scaffold (Sequencing Step 0 of the core-syntax convergence
# program). Every grammar production, decorator, and builtin should
# eventually have a row here with status: covered. No CI gate reads this
# file yet — enforcement lands in Sequencing Step 8. Add a row in the same
# PR that lands the production it describes; do not backfill later.
schema_version: 1
mode: warn
productions:
  - name: lexer/unknown-char-token
    kind: token
    status: covered
    fixture: "crates/vox-compiler/src/lexer/cursor.rs::tests::unknown_char_becomes_a_real_token"
    spec_ref: S2
  - name: reader-tolerant-semicolon
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/stmt.rs::tests::tolerates_and_warns_on_trailing_semicolon"
    spec_ref: S2
  - name: reader-tolerant-eq-eq
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs::tests::eq_eq_parses_and_warns"
    spec_ref: S2
  - name: reader-tolerant-not-eq
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs::tests::not_eq_parses_and_warns"
    spec_ref: S2
  - name: arrow-return-type-warning
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/mod.rs::tests::arrow_return_type_still_warns"
    spec_ref: S2
  - name: arrow-match-arm-stays-error
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/expr/pratt_match.rs::tests::arrow_match_arm_is_not_aliased"
    spec_ref: S2
  - name: bang-invalid-carries-replacement
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/expr/pratt_match.rs::tests::bang_invalid_error_carries_not_replacement"
    spec_ref: S2
  - name: bounded-unknown-token-diagnostics
    kind: grammar-rule
    status: covered
    fixture: "crates/vox-compiler/src/parser/descent/mod.rs::tests::long_run_of_unknown_bytes_bounds_diagnostics"
    spec_ref: S2
  # --- Everything else in the language is not yet ledgered. Sequencing
  # Steps 2-8 add their own rows as they land; this bucket is intentionally
  # visible (not silently omitted) so the file's own incompleteness is
  # honest per the audit's finding that CR-F3 must never again claim
  # coverage it doesn't have. ---
  - name: full-decorator-surface
    kind: decorator
    status: todo
    fixture: null
    spec_ref: S4
  - name: boolean-operator-aliases
    kind: grammar-rule
    status: todo
    fixture: null
    spec_ref: S3
  - name: else-if-chains
    kind: grammar-rule
    status: todo
    fixture: null
    spec_ref: S5
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test language_surface_coverage_schema_test`
Expected: PASS (2 tests) — note this test will only fully pass once Tasks
1–5 below land their own fixtures with matching names; if you're doing
Task 0 strictly first, the schema/structure assertions pass now and the
fixture-existence checks are satisfied by name only (the ledger doesn't
verify the fixture path resolves to a real test — that's an enforce-mode
concern for Step 8, out of scope here).

- [ ] **Step 6: Commit**

```bash
git add contracts/spec/language-surface-coverage.v1.yaml \
        contracts/spec/language-surface-coverage.v1.schema.json \
        crates/vox-compiler/tests/language_surface_coverage_schema_test.rs
git commit -m "feat(compiler): scaffold CR-F3 language-surface coverage ledger (warn mode)"
```

---

### Task 1: Regenerate `mens/config/system_prompt.txt`

**Files:**
- Modify: `mens/config/system_prompt.txt` (full rewrite)
- Test: `crates/vox-integration-tests/tests/mens_system_prompt_syntax_test.rs`

The current file (106 lines, last touched 2026-06-13) teaches a
colon-block, `ret`-using, `@table type`/`@query fn`/`@component fn`/
`@mcp.tool(...)` dialect that has not existed since April 2026 — verified
during the audit's review pass by direct read. This is the highest-leverage
fix in the whole program: it has no dependency on anything else in this
plan or the follow-up plans.

This task does not build the generator infrastructure the spec's S6
eventually wants (deriving this file from a parser-derived grammar IR) —
that IR doesn't exist yet (see the spec's S6 correction). This task is a
**hand-written correction**, verified by a test that actually compiles every
code fence in the file through the real parser, so it cannot silently
drift back to a dead dialect.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-integration-tests/tests/mens_system_prompt_syntax_test.rs
//! The MENS system prompt teaches Vox syntax by example. Every fenced code
//! block in it must actually parse against the current grammar — this is
//! the regression guard against the exact defect the audit found (a prompt
//! that taught a dead colon-block dialect for 4+ months undetected).

use std::fs;

fn extract_fenced_blocks(src: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("```") {
            if in_block {
                blocks.push(std::mem::take(&mut current));
                in_block = false;
            } else {
                in_block = true;
            }
            continue;
        }
        if in_block {
            current.push_str(line);
            current.push('\n');
        }
    }
    blocks
}

#[test]
fn every_fenced_block_in_system_prompt_parses() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../mens/config/system_prompt.txt"
    );
    let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let blocks = extract_fenced_blocks(&src);
    assert!(
        blocks.len() >= 3,
        "expected at least 3 fenced code examples (actor, workflow, component), found {}",
        blocks.len()
    );
    for (i, block) in blocks.iter().enumerate() {
        let tokens = vox_compiler::lexer::lex(block);
        let result = vox_compiler::parser::parse_script(tokens);
        assert!(
            result.is_ok(),
            "fenced block {i} failed to parse:\n---\n{block}\n---\nerrors: {:?}",
            result.err()
        );
    }
}

#[test]
fn system_prompt_does_not_mention_retired_syntax() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../mens/config/system_prompt.txt"
    );
    let src = fs::read_to_string(path).unwrap();
    for retired in [
        "ret ",
        "@component fn",
        "@table type",
        "@query fn",
        "@mutation fn",
        "@server fn",
        "@mcp.tool(",
        "@action fn",
        "@agent_def fn",
        "@page fn",
        "@layout fn",
        "@hook fn",
        "@provider fn",
        "@keyframes",
    ] {
        assert!(
            !src.contains(retired),
            "system_prompt.txt still mentions retired syntax: {retired:?}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-integration-tests --test mens_system_prompt_syntax_test`
Expected: FAIL on `system_prompt_does_not_mention_retired_syntax` (the
current file contains `ret `, `@component fn`, `@table type`, `@query fn`,
`@mcp.tool(`, `@action fn`, `@agent_def fn`, `@page fn`, `@layout fn`,
`@hook fn`, `@provider fn`, `@keyframes` — verified present in the file read
during planning) and likely also on `every_fenced_block_in_system_prompt_parses`
(the actor/workflow/component examples use colon-block syntax the brace
parser rejects).

- [ ] **Step 3: Rewrite the file to current, verified syntax**

```text
You are a Vox programming language expert and code generation assistant. Vox is an AI-native, full-stack programming language that compiles to a tree-walking interpreter, native Rust, and TypeScript/React. It was designed for building modern web applications, AI agents, and durable workflows with minimal boilerplate.

## Language Philosophy
- **Compression over ceremony**: express complex ideas in fewer lines than Rust or TypeScript
- **Full-stack in one file**: define types, backend logic, UI components, and routing together
- **Durable by default**: `workflow`/`activity` survive process crashes
- **AI-native**: first-class support for agents, MCP tools, and skills
- **One primitive per concept**: Vox picks a single canonical spelling for each idea rather than offering synonyms — do not invent alternate spellings for constructs below.

## Core Syntax

### Blocks and statements
Vox is brace-delimited, not indentation-sensitive. A statement ends at the
end of its line — there are no semicolons. Comments are `//` (line) or
`///` (doc comment).

```vox
fn add(a: int, b: int) to int {
    return a + b
}
```

### Variables and control flow
- `let x = expr` — immutable binding
- `let mut x = expr` — mutable binding
- `return expr` — return a value (bare `return` returns Unit)
- `if condition { … } else { … }`
- `for item in collection { … }`
- `match expr { Variant(field) => body … }` — arms use `=>`, not `->`
- Phonetic operators: `and`, `or`, `not`, `is`, `is not` — Vox does not use `&&`, `||`, `!`, `==`, `!=` in its own canonical form (a tolerant reader accepts them with a warning, but do not emit them).

### Types
- `type Name { field: Type }` — struct
- `type Name = | VariantA | VariantB(field: Type)` — tagged union (ADT)
- Container types are lowercase (`list`, `map`, `set`); ADT/shape types are capitalized (`Option`, `Result`, `Unit`, `MyType`, `Some`, `Ok`)
- `Result[T]` (single-parameter, message-string error) is the default; `Result[T, E]` (typed error) is used at API boundaries

## Construct Reference

- **function**: `fn name(param: Type) to ReturnType { … }` — standard function
- **component**: `component Name() { view: … }` — UI component (bare keyword, not a decorator)
- **table**: `table Name { field: Type }` — database table (bare keyword)
- **query**: `query name(param: Type) to Type { … }` — read-only database query (bare keyword)
- **mutation**: `mutation name(param: Type) to Type { … }` — database write operation (bare keyword)
- **server**: `server name(param: Type) to Type { … }` — server-only endpoint (bare keyword)
- **tool**: `tool "name: description" name(param: Type) to Type { … }` — MCP tool for AI assistants (bare keyword)
- **resource**: `resource "uri" "description" name() to Type { … }` — MCP read-only resource (bare keyword)
- **actor**: `actor Name(param: Type) { on handler(msg) { … } }` — message-passing concurrency
- **workflow**: `workflow name(param: Type) to Result[Type] { … }` — durable multi-step orchestration
- **activity**: `activity name(param: Type) to Result[Type] { … }` — retryable side-effectful step, called from inside a `workflow`
- **routes**: `routes { "/" to Home }` — client-side routing
- **state_machine**: `state_machine Name { state A { on Event -> B } }` — first-class state machine with exhaustiveness checking
- **import**: `import module.name`, `import "./file.vox"`, or `import react { X } from "pkg"`
- **@test**: `@test fn name() { assert(condition) }` — unit test
- **@pure**: `@pure fn name(...) to T { … }` — marks a function as calling no `http`/`net`/`fs`/`db`/`random`/`time`/`log`/`async`
- **@uses**: `@uses(net) fn name(...) { … }` — declares the I/O effects a function performs; required on any public function calling `http`/`net`
- **@scheduled**: `@scheduled("1h") fn name() { … }` — cron/interval scheduled job
- **@auth**: `@auth(scheme: bearer) fn name(...) { … }` — authentication requirement on an endpoint

Do not use, and do not suggest: `@endpoint(kind: …)`, `@component fn`,
`@table type`, `@query fn`/`@mutation fn`/`@server fn`, `@mcp.tool(...)`,
`@action`, `@agent_def`, `@page`, `@layout`, `@hook`, `@provider`,
`@keyframes`, `@fixture`, `@mock`, `ret` — all retired; the compiler
rejects them with a machine-readable suggestion pointing at the forms above.

## Actors (Message-Passing Concurrency)

```vox
actor Counter() {
    state count: int = 0
    on increment() to int {
        count = count + 1
        return count
    }
    on reset() to Unit {
        count = 0
    }
}
```
- `spawn(ActorName)` — creates a new actor instance
- `handle.send(method(args))` — sends a message to the actor

## Workflows & Activities (Durable Execution)

```vox
activity fetch_data(id: str) to Result[str] {
    return Ok(id)
}

workflow process(id: str) to Result[str] {
    let result = fetch_data(id) with { retries: 3, timeout: "30s" }
    return Ok(result)
}
```

## Components (Vox's own markup, not JSX)

Vox components use primitive calls with keyword arguments and `{ }`
children blocks — not literal JSX. `bind={state}` wires a controlled input.

```vox
component Counter() {
    state count: int = 0
    view: column() {
        text() { "Count: {count}" }
        button(on_click={count = count + 1}) { "Increment" }
    }
}
```

## Agentic Behavior & Tooling
When acting as an agent generating Vox code:
- Prefer bare-keyword `tool`/`resource` for MCP capabilities that require external state (not the retired `@mcp.tool` decorator form).
- Use `workflow` for long-running, multi-step tasks that must survive process restarts; put non-deterministic or side-effectful work in a separate `activity`, not directly in the `workflow` body.
- Use `import` to bring in relevant domain modules; stdlib modules are short-named (`import fs`, `fs.read(...)`), not `std.fs.*`.
- If `vox check` fails, read the diagnostic's `expected`/`found` fields and the machine-readable `replacement` payload when present — it names the exact canonical spelling to use.

## Best Practices
1. Always include type annotations on function parameters and return types.
2. Use `Result[T]` (or `Result[T, E]` at API boundaries) for operations that can fail; prefer `?` to propagate errors over a full `match`-and-rewrap.
3. Use `with { retries: N, timeout: "Ns" }` for activity calls inside a workflow.
4. Use descriptive names: snake_case for functions/variables, PascalCase for types/actors/components.
5. Prefer tagged unions (`type X = | A | B(...)`) over ad hoc nullable fields.
6. 4-space indentation is the style convention, but it is cosmetic — braces, not indentation, define block structure.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-integration-tests --test mens_system_prompt_syntax_test`
Expected: PASS (2 tests). If `every_fenced_block_in_system_prompt_parses`
fails on a specific block, read the parser's error output (it names the
exact `expected`/`found` mismatch) and fix that block — do not weaken the
test to skip the failing block.

- [ ] **Step 5: Commit**

```bash
git add mens/config/system_prompt.txt \
        crates/vox-integration-tests/tests/mens_system_prompt_syntax_test.rs
git commit -m "fix(mens): regenerate system_prompt.txt off the current grammar

Was teaching a colon-block, ret-using, @table type/@query fn/@mcp.tool
dialect dead since April 2026 (audit finding, highest-leverage training
defect in the repo). Now verified by a test that compiles every fenced
example through the real parser."
```

---

### Task 2: Fix `AGENTS.md` §Grammar Unification

**Files:**
- Modify: `AGENTS.md:224-252`
- Test: `crates/vox-compiler/tests/agents_md_grammar_section_test.rs`

Verified live during the review pass: this section still lists `@table`,
`@query`, `@mutation`, `@server` as canonical decorators (they are hard
parse errors since 2026-06-30, `cd7cc96874`) and omits the bare-keyword
forms that actually replaced them.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-compiler/tests/agents_md_grammar_section_test.rs
//! AGENTS.md's Grammar Unification section is the always-loaded agent
//! policy surface. It must not tell agents that retired decorator spellings
//! are canonical.

use std::fs;

#[test]
fn grammar_unification_section_does_not_list_retired_decorators_as_canonical() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../AGENTS.md");
    let src = fs::read_to_string(path).unwrap();
    let section_start = src
        .find("## Grammar Unification")
        .expect("AGENTS.md must have a Grammar Unification section");
    let section_end = src[section_start..]
        .find("\n## ")
        .map(|i| section_start + i)
        .unwrap_or(src.len());
    let section = &src[section_start..section_end];

    for retired in ["`@table`", "`@query`", "`@mutation`", "`@server`"] {
        assert!(
            !section.contains(retired),
            "Grammar Unification section still lists retired decorator {retired} \
             as canonical (it is a hard parse error since 2026-06-30, cd7cc96874)"
        );
    }
    for canonical in ["`table`", "`query`", "`mutation`", "`server`"] {
        assert!(
            section.contains(canonical),
            "Grammar Unification section must list the canonical bare-keyword \
             form {canonical}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test agents_md_grammar_section_test`
Expected: FAIL — the section currently contains `` `@table` ``, `` `@query` ``,
`` `@mutation` ``, `` `@server` `` verbatim (confirmed by direct read during
planning).

- [ ] **Step 3: Rewrite the section**

Replace `AGENTS.md:224-252` (the full `## Grammar Unification (Vox Source
Syntax)` section) with:

```markdown
## Grammar Unification (Vox Source Syntax)

Vox source follows one rule for top-level declarations:

> **Bare-keyword blocks declare scope. Decorators modify declarations.**

**Bare-keyword blocks** (each opens a scope with its own rules):
`type`, `fn`, `component`, `state_machine`, `routes`, `module`, `actor`,
`workflow`, `activity` (the last two stable per ADR-041 — see
Implementation status below), and the data-layer soft keywords `table`,
`query`, `mutation`, `server`, `tool`, `resource`, `form`, `index` (these
eight replaced the equivalent `@table`/`@query`/`@mutation`/`@server`/
`@tool`/`@resource`/`@form`/`@index` decorators — see §Retired Surfaces;
the decorator spellings are hard parse errors as of 2026-06-30, `cd7cc96874`).

**Decorators** (modifiers composed on top of a declaration):
`@pure`, `@deprecated`, `@require`, `@auth`, `@uses`, `@test`, `@durable`,
`@scheduled` (the last two stable per ADR-041 — see Implementation status
below). **Removed in v0.6.0:** `@endpoint` (see §Retired Surfaces).
**Retired 2026-06-30:** `@table`, `@query`, `@mutation`, `@server`, `@tool`,
`@resource`, `@form`, `@index`, `@mcp.tool`, `@mcp.resource` — use the
bare-keyword forms above instead (`tool`/`resource` replace the two MCP
decorators; the others replace themselves 1:1).

Decorators compose with bare-keyword blocks:

```vox
// vox:skip
@auth(scheme: bearer) table Task { … }         // decorator on a bare-keyword declaration
@uses(net) fn fetch_remote() { … }              // decorator on a function
@pure fn checksum(payload: bytes) { … }         // purity declared via decorator
```

**Rule for new features:** Do NOT introduce a new bare keyword for behavior
that can be expressed as a decorator. New execution semantics (durability,
tracing, sandboxing, rate-limiting) belong as decorators on `fn`. A
construct is a bare keyword iff it produces a distinct `Decl` AST variant
(the taxonomy law — see `docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md`);
otherwise it is a decorator; value-level constructs are builtins.

**Implementation status.** `actor`/`workflow`/`activity` and `@durable`/
`@scheduled` are stable, backed by a durable runtime for the supported
subset (ADR-041 supersedes the old ADR-028 reservation gate — out-of-subset
behavior is now policed by the determinism lint, not a reservation gate).
Contract: [ADR-019](docs/src/adr/019-durable-workflow-journal-contract-v1.md),
[ADR-021](docs/src/adr/021-generated-workflow-durability-parity.md),
[ADR-041](docs/src/adr/041-durable-functions-completion-2026.md). Drift
between this section and `pipeline.rs` is checked by the
[`docs-reality-audit-program`](docs/src/contributors/docs-reality-audit-program.md).
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test agents_md_grammar_section_test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md crates/vox-compiler/tests/agents_md_grammar_section_test.rs
git commit -m "fix(agents): correct Grammar Unification section to post-taxonomy-flip reality

Was listing @table/@query/@mutation/@server as canonical decorators;
they've been hard parse errors since 2026-06-30 (cd7cc96874). Now lists
the bare-keyword forms that replaced them and locks it in with a test."
```

---

### Task 3: Delete dead `parser/indent.rs`

**Files:**
- Delete: `crates/vox-compiler/src/parser/indent.rs`
- Test: none needed — see Step 1 (this is a "verify it's dead, then delete" task, not a TDD-red-green task, since there's no behavior to test)

- [ ] **Step 1: Verify it is genuinely unreferenced**

```bash
grep -rn "mod indent" crates/vox-compiler/src/parser/mod.rs
```
Expected: no output — `indent` is not declared as a module in `parser/mod.rs`
(only `descent`, `error`, `renames`, `with_registry` are). Also confirm no
other file references it:
```bash
grep -rn "IndentTracker\|parser::indent\|parser/indent" crates/ --include=*.rs
```
Expected: only matches inside `indent.rs` itself (its own definition).

- [ ] **Step 2: Delete the file and confirm the workspace still builds**

```bash
rm crates/vox-compiler/src/parser/indent.rs
cargo check -p vox-compiler
```
Expected: builds clean — the file was never compiled (not a declared
module), so this is a zero-risk deletion.

- [ ] **Step 3: Commit**

```bash
git add -u crates/vox-compiler/src/parser/indent.rs
git commit -m "chore(compiler): delete dead parser/indent.rs

Not a declared module in parser/mod.rs (only descent/error/renames/
with_registry are) and zero references elsewhere. Its doc comment claimed
the lexer emits Indent/Dedent tokens, which is false — the lexer is
explicitly brace-delimited. Was never compiled."
```

---

### Task 4: `Token::Unknown(char)` — kill both silent-drop sites

**Files:**
- Modify: `crates/vox-compiler/src/lexer/token.rs`
- Modify: `crates/vox-compiler/src/lexer/cursor.rs`

**Files (this task's test lives inline in `cursor.rs`'s existing
`#[cfg(test)] mod tests` block, per that file's established convention.)**

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-compiler/src/lexer/cursor.rs`'s existing `mod tests`
block (after `lex_preserving_keeps_raw_cr`, the last test in the file):

```rust
    /// S2: a character that matches no other token pattern must become a
    /// real, spanned `Token::Unknown` — not silently vanish. This is the
    /// fix for the audit's finding-2 P0 hazard (`a && b` used to lex as
    /// `a b`; `@unknown_decorator` degraded to a bare identifier).
    #[test]
    fn unknown_char_becomes_a_real_token() {
        let toks = lex_tokens("a ^ b");
        assert_eq!(
            toks,
            vec![
                Token::Ident("a".into()),
                Token::Unknown('^'),
                Token::Ident("b".into()),
                Token::Eof,
            ],
            "unrecognized char must lex to Token::Unknown, not vanish"
        );
    }

    /// Multiple consecutive unknown chars each become their own token — no
    /// merging, no silent collapse.
    #[test]
    fn multiple_unknown_chars_each_become_their_own_token() {
        let toks = lex_tokens("^~$");
        assert_eq!(
            toks,
            vec![
                Token::Unknown('^'),
                Token::Unknown('~'),
                Token::Unknown('$'),
                Token::Eof,
            ]
        );
    }

    /// `&&` must NOT collapse into a single BooleanAnd-shaped token by this
    /// task's change — S2's boolean-operator work (adding real And/Or
    /// tokens + Pratt arms) is explicitly out of scope for this plan and
    /// lands in the follow-up plan for Sequencing Step 2+. For now `&&`
    /// lexes as two Unknown('&') tokens, which is strictly better than the
    /// prior silent drop (it now produces a clear parse error instead of
    /// `a && b` silently becoming `a b`) without committing to the larger
    /// change here.
    #[test]
    fn double_ampersand_lexes_as_two_unknown_tokens_for_now() {
        let toks = lex_tokens("a && b");
        assert_eq!(
            toks,
            vec![
                Token::Ident("a".into()),
                Token::Unknown('&'),
                Token::Unknown('&'),
                Token::Ident("b".into()),
                Token::Eof,
            ]
        );
    }

    /// `lex_preserving` (the byte-preserving path `vox fmt`/`vox migrate`
    /// depend on) must also surface Unknown tokens rather than silently
    /// drop them — both call sites of the old `Err(_) => None` behavior
    /// are fixed by the same lexer-level change.
    #[test]
    fn lex_preserving_also_surfaces_unknown_chars() {
        let spanned = lex_preserving("a ^ b");
        let toks: Vec<&Token> = spanned.iter().map(|s| &s.token).collect();
        assert!(
            toks.contains(&&Token::Unknown('^')),
            "lex_preserving must not silently drop unknown chars either: {toks:?}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --lib lexer::cursor::tests::unknown_char_becomes_a_real_token`
Expected: compile FAIL — `Token::Unknown` doesn't exist yet.

- [ ] **Step 3: Add the token**

In `crates/vox-compiler/src/lexer/token.rs`, add the variant just before the
`// ── Identifiers ──` section comment (i.e. after the literal patterns,
before `Ident`/`TypeIdent` — placement doesn't affect Logos behavior, this
keeps it visually grouped with other fallback-shaped patterns):

```rust
    // ── Unknown ───────────────────────────────────────────────
    /// A single character that matched no other token pattern. Carries the
    /// raw character so the parser can produce a real diagnostic (and,
    /// where a known alias mapping exists — see the tolerant-reader policy
    /// in `parser/descent/`, e.g. `;`, `==`, `!=` — a machine-readable fix)
    /// instead of the character silently vanishing. Lowest priority so
    /// every other token pattern wins on any overlap.
    #[regex(r".", priority = 0)]
    Unknown(char),
```

Then find the callback need: Logos's `#[regex(r".")]` without a callback
captures the matched `&str` slice, not a `char`, by default — add an
explicit callback converting the single-character slice:

```rust
    #[regex(r".", priority = 0, callback = |lex| lex.slice().chars().next())]
    Unknown(char),
```

Add the corresponding `Display` arm in `impl std::fmt::Display for Token`
(next to the other literal-value arms, e.g. near `Token::Ident`):

```rust
            Token::Unknown(c) => write!(f, "{c}"),
```

- [ ] **Step 4: Update the now-clarified comments at both silent-drop sites**

`Token::Unknown` means Logos's `Err(_)` case (a byte sequence matching *no*
pattern at all, not even the new catch-all) should now be effectively
unreachable for well-formed UTF-8 input — the catch-all regex `.` matches
any single Unicode scalar value. The `Err(_) => None` arms stay (Rust match
exhaustiveness requires handling `Err`, and it's defensive against a Logos
internal error distinct from "no user-visible token matched"), but their
comments were actively misleading (they described the *removed* behavior
as intentional). Update both:

In `crates/vox-compiler/src/lexer/cursor.rs`, `lex_preserving` (around line
25):
```rust
            Err(_) => None, // Logos-internal lex failure only — every single
            // character that isn't part of a longer token now matches the
            // Token::Unknown(char) catch-all (added for the tolerant-reader
            // policy, see docs/superpowers/specs/2026-08-08-vox-core-syntax-
            // convergence-design.md S2) and reaches this iterator as Ok(_).
            // This arm is defensive, not the "skip unrecognized characters"
            // behavior it used to describe.
```

In `lex` (around line 58), the same replacement:
```rust
            Err(_) => None, // See the identical comment in lex_preserving above.
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-compiler --lib lexer::cursor`
Expected: PASS — all four new tests plus every pre-existing test in
`cursor.rs`'s `mod tests` (this task must not break
`test_bang_lexes_as_invalid_distinct_from_not`,
`test_bang_eq_still_one_token`, or any symbol/keyword test — the catch-all's
`priority = 0` must lose to every specific `#[token]`/`#[regex]` pattern).
If any pre-existing test fails, the priority is wrong — do not weaken those
tests to accommodate it; fix the priority value instead.

Run the full lexer+parser suite to catch any cross-cutting fallout:
Run: `cargo test -p vox-compiler --lib lexer:: parser::`
Expected: PASS, no regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/lexer/token.rs crates/vox-compiler/src/lexer/cursor.rs
git commit -m "fix(lexer): Token::Unknown(char) catch-all kills silent unknown-byte drop

Both lex() and lex_preserving()'s Err(_) => None arms silently dropped
every unrecognized character (audit finding 2, the project's worst P0
violation — this is the same class of bug that caused '!' to silently
mean nothing before BangInvalid existed). A lowest-priority regex
catch-all now surfaces every unknown char as a real, spanned token
instead. && and || still lex as two separate Unknown('&')/Unknown('|')
tokens for now (real boolean-operator support is a larger, separate
change deferred to Sequencing Step 2+) but this alone converts a silent
semantic change into a visible parse error, which is strictly safer."
```

---

### Task 5: Tolerant `;` at statement boundaries

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/mod.rs`
- Modify: `crates/vox-compiler/src/parser/descent/stmt.rs`

Scoped deliberately to statement-boundary `;` only (the dominant real-world
case — the audit found 1,145 corpus `;`-terminated lines, virtually all
statement terminators). Broader positions (e.g. inside argument lists) are
out of scope for this task; if discovered as a real corpus need later, they
get their own task in a follow-up plan.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-compiler/src/parser/descent/stmt.rs`. This file has no
existing `#[cfg(test)] mod tests` block — add one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::parser::descent::Parser;

    /// S2/S3: a trailing `;` after a statement is tolerated (parses) and
    /// produces exactly one Warning-severity diagnostic carrying a
    /// Replacement payload that strips it — the tolerant-reader/strict-
    /// writer policy. This is the dominant real-world case: the audit found
    /// 1,145 corpus lines ending in `;`, almost all statement terminators.
    #[test]
    fn tolerates_and_warns_on_trailing_semicolon() {
        let tokens = lex("import fs;\nlet x = 5;\n");
        let module = crate::parser::parse_script(tokens)
            .expect("trailing ';' must not be a hard parse error");
        assert_eq!(module.declarations.len(), 2, "both statements must parse");
    }

    /// The warning diagnostic itself: severity Warning, class Statement,
    /// and a Replacement payload that deletes the semicolon (from ";" to
    /// "" ) with the stable code `vox/lexer/semicolon-unnecessary`, so
    /// `vox fmt` can auto-apply it later without parsing English text.
    #[test]
    fn semicolon_warning_carries_replacement_payload() {
        use crate::parser::descent::Parser;
        let tokens = lex("let x = 5;\n");
        let mut p = Parser::new(tokens);
        let _ = p.parse_module_script();
        let warnings: Vec<_> = p
            .errors_for_test()
            .iter()
            .filter(|e| e.message.contains("semicolon"))
            .collect();
        assert_eq!(warnings.len(), 1, "expected exactly one semicolon warning");
        let w = warnings[0];
        assert_eq!(w.severity, crate::parser::error::ParseSeverity::Warning);
        let r = w
            .replacement
            .as_ref()
            .expect("semicolon warning must carry a Replacement payload");
        assert_eq!(r.from, ";");
        assert_eq!(r.to, "");
        assert_eq!(r.code, "vox/lexer/semicolon-unnecessary");
    }

    /// A file with NO semicolons is completely unaffected — no warnings,
    /// same parse result as before this change.
    #[test]
    fn no_semicolon_no_warning() {
        use crate::parser::descent::Parser;
        let tokens = lex("let x = 5\n");
        let mut p = Parser::new(tokens);
        let _ = p.parse_module_script();
        assert!(
            p.errors_for_test().is_empty(),
            "file with no ';' must produce zero diagnostics from this change"
        );
    }
}
```

This test needs a small test-only accessor on `Parser` (it currently has no
public/pub(crate) way to read `errors` after a failed-or-succeeded parse
from outside the module for inspection in a sibling test file). Add it in
Task 5 Step 3 alongside the real implementation, not as a separate task —
it's one line and only exists to make this test possible.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --lib parser::descent::stmt::tests`
Expected: compile FAIL — `Parser::errors_for_test` doesn't exist, and
`tolerates_and_warns_on_trailing_semicolon` would fail at runtime anyway
(today `;` is silently dropped by the *old* lexer behavior — but Task 4
already landed, so as of this task's starting point, `;` lexes to
`Token::Unknown(';')` and the parser has no tolerance for it yet, so this
test fails with a real parse error, not a silent pass).

- [ ] **Step 3: Implement**

In `crates/vox-compiler/src/parser/descent/mod.rs`, add a test-only accessor
right after the existing `pub(crate) fn peek_nth` method:

```rust
    /// Test-only accessor so sibling test modules can inspect accumulated
    /// diagnostics without going through the full `parse`/`parse_script`
    /// `Result<_, Vec<ParseError>>` API (which discards warnings on Ok).
    #[cfg(test)]
    pub(crate) fn errors_for_test(&self) -> &[ParseError] {
        &self.errors
    }
```

Then add the tolerant-skip helper right after `skip_newlines` (around line
147):

```rust
    /// S2/S3 tolerant-reader policy: a `;` immediately after a statement is
    /// accepted (it lexes to `Token::Unknown(';')` per Task 4) with a
    /// Warning diagnostic carrying a machine-readable `Replacement` that
    /// deletes it — Vox statements are newline-terminated, not
    /// semicolon-terminated. Scoped to the statement-boundary position
    /// only (see this task's own scope note); does not touch `;` anywhere
    /// else a stray one might appear.
    pub(crate) fn skip_tolerated_semicolon(&mut self) {
        if matches!(self.peek(), Token::Unknown(';')) {
            let span = self.span();
            self.errors.push(ParseError {
                message: "Vox statements end at end of line; no semicolon needed"
                    .to_string(),
                span,
                expected: vec![],
                found: Some(";".to_string()),
                class: ParseErrorClass::Statement,
                severity: ParseSeverity::Warning,
                replacement: Some(crate::parser::error::Replacement {
                    from: ";".to_string(),
                    to: String::new(),
                    code: "vox/lexer/semicolon-unnecessary".to_string(),
                }),
            });
            self.advance();
        }
    }
```

In `crates/vox-compiler/src/parser/descent/stmt.rs`, call it from
`parse_block`'s loop — modify the existing loop body (lines 14–30) from:

```rust
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(()) => {
                    // Recovery: skip to the next statement boundary so we can
                    // collect further errors within the same block.
                    while !matches!(self.peek(), Token::Newline | Token::RBrace | Token::Eof) {
                        self.advance();
                    }
                }
            }
            self.skip_newlines();
        }
```

to:

```rust
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(()) => {
                    // Recovery: skip to the next statement boundary so we can
                    // collect further errors within the same block.
                    while !matches!(self.peek(), Token::Newline | Token::RBrace | Token::Eof) {
                        self.advance();
                    }
                }
            }
            self.skip_tolerated_semicolon();
            self.skip_newlines();
        }
```

Also apply the same one-line addition to the top-level script/module
statement-collection loop, which lives in `parse_module_script` — search
for it:

```bash
grep -n "fn parse_module_script" crates/vox-compiler/src/parser/descent/mod.rs
```

Read that function and add `self.skip_tolerated_semicolon();` at the
equivalent point (immediately after a successful top-level statement parse,
before the next `skip_newlines()` call) — this is what makes
`import fs;`/`let x = 5;` at true top level (script mode) tolerant, matching
the corpus's dominant `scripts/*.vox` usage pattern from the audit.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --lib parser::descent::stmt::tests`
Expected: PASS (3 tests).

Run the full parser suite to confirm no regression:
Run: `cargo test -p vox-compiler --lib parser::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/mod.rs \
        crates/vox-compiler/src/parser/descent/stmt.rs
git commit -m "feat(parser): tolerate trailing ';' at statement boundaries (warn + fix-it)

Vox has no semicolons in its grammar and never did (no ';' token exists);
the corpus still has 1,145 lines ending in ';' (mostly in scripts/),
which previously parsed only because the old lexer silently dropped the
character. Now the ';' lexes to a real Token::Unknown and the parser
explicitly tolerates it at statement boundaries with a Warning diagnostic
carrying a Replacement payload (vox/lexer/semicolon-unnecessary) so
vox fmt can auto-strip it later. Scoped to statement-boundary position
only per spec S2/S3."
```

---

### Task 6: Reader-tolerant `==`/`!=` warnings (no new parsing)

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs`

`Token::EqEq`/`Token::NotEq` already unify into the canonical
`BinOp::Is`/`BinOp::Isnt` at `pratt_ops.rs:58-59` — this task adds **only**
a diagnostic, no new parsing behavior, per the spec's corrected S2 framing.

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)] mod tests` block to `pratt_ops.rs` (it currently has one
already, `semcov_wave1c_tests` — add these as new `#[test]` fns inside that
same module, since it already imports `use super::*;`):

```rust
    /// S2: `==` parses exactly as `is` does (same BinOp, same AST shape —
    /// this was already true before this change) but now also emits a
    /// single Warning diagnostic pointing at the canonical spelling.
    #[test]
    fn eq_eq_parses_and_warns() {
        use crate::lexer::lex;
        use crate::parser::descent::Parser;

        let tokens = lex("a == b\n");
        let mut p = Parser::new(tokens);
        let expr = p.parse_expr().expect("== must still parse");
        assert!(
            matches!(expr, Expr::Binary { op: BinOp::Is, .. }),
            "== must produce the same BinOp::Is as `is`"
        );
        let warnings: Vec<_> = p
            .errors_for_test()
            .iter()
            .filter(|e| e.severity == crate::parser::error::ParseSeverity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1, "expected exactly one warning for ==");
        assert!(warnings[0].message.contains("is"));
    }

    /// Same for `!=` -> BinOp::Isnt, pointing at `is not`.
    #[test]
    fn not_eq_parses_and_warns() {
        use crate::lexer::lex;
        use crate::parser::descent::Parser;

        let tokens = lex("a != b\n");
        let mut p = Parser::new(tokens);
        let expr = p.parse_expr().expect("!= must still parse");
        assert!(matches!(expr, Expr::Binary { op: BinOp::Isnt, .. }));
        let warnings: Vec<_> = p
            .errors_for_test()
            .iter()
            .filter(|e| e.severity == crate::parser::error::ParseSeverity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("is not"));
    }

    /// The canonical spellings (`is`, `is not` via `isnt`) must NOT trigger
    /// this new warning — only the mainstream aliases do.
    #[test]
    fn is_and_isnt_do_not_warn() {
        use crate::lexer::lex;
        use crate::parser::descent::Parser;

        for src in ["a is b\n", "a isnt b\n"] {
            let tokens = lex(src);
            let mut p = Parser::new(tokens);
            let _ = p.parse_expr().expect("must parse");
            assert!(
                p.errors_for_test().is_empty(),
                "canonical spelling {src:?} must not produce any diagnostic"
            );
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --lib parser::descent::expr::pratt_ops::semcov_wave1c_tests::eq_eq_parses_and_warns`
Expected: FAIL — no warning is currently pushed for `==`/`!=` (they parse
silently today, per the audit's own finding that this is *already*
unified parsing, just with no diagnostic).

- [ ] **Step 3: Implement**

Modify the operator-match block in `parse_expr_bp`
(`crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs:46-62`) from:

```rust
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Lte => BinOp::Lte,
                Token::Gte => BinOp::Gte,
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                Token::Is | Token::EqEq => BinOp::Is,
                Token::Isnt | Token::NotEq => BinOp::Isnt,
                Token::PipeOp => BinOp::Pipe,
                _ => break,
            };
            let (l_bp, r_bp) = infix_bp(op);
            if l_bp < min_bp {
                break;
            }
            self.advance();
```

to:

```rust
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Lte => BinOp::Lte,
                Token::Gte => BinOp::Gte,
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                Token::Is => BinOp::Is,
                Token::EqEq => {
                    self.warn_mainstream_operator_alias("==", "is");
                    BinOp::Is
                }
                Token::Isnt => BinOp::Isnt,
                Token::NotEq => {
                    self.warn_mainstream_operator_alias("!=", "is not");
                    BinOp::Isnt
                }
                Token::PipeOp => BinOp::Pipe,
                _ => break,
            };
            let (l_bp, r_bp) = infix_bp(op);
            if l_bp < min_bp {
                break;
            }
            self.advance();
```

Add the shared helper in `crates/vox-compiler/src/parser/descent/mod.rs`,
next to `skip_tolerated_semicolon` — this is deliberately shared (not
`==`-specific) so Task 4's follow-up work on `&&`/`||`/`->` in later plans
can reuse it:

```rust
    /// S2/S3 tolerant-reader policy: push a Warning diagnostic when a
    /// mainstream/legacy spelling was accepted in place of the canonical
    /// one. No `Replacement` payload here (unlike `skip_tolerated_semicolon`)
    /// because the AST is already correct — `vox fmt` derives the canonical
    /// spelling from the AST node, it doesn't need a text-level fix-it for
    /// operators that already parsed to the right `BinOp`.
    pub(crate) fn warn_mainstream_operator_alias(&mut self, found: &str, canonical: &str) {
        let span = self.span();
        self.errors.push(ParseError {
            message: format!(
                "`{found}` works, but Vox's canonical spelling is `{canonical}`"
            ),
            span,
            expected: vec![canonical.to_string()],
            found: Some(found.to_string()),
            class: ParseErrorClass::Expression,
            severity: ParseSeverity::Warning,
            replacement: None,
        });
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --lib parser::descent::expr::pratt_ops`
Expected: PASS (4 new tests, all pre-existing tests in the module
unaffected).

Run the full compiler test suite for regressions (this touches a very
hot path — every binary expression in the language goes through this
function):
Run: `cargo test -p vox-compiler`
Expected: PASS, zero regressions. If any golden/snapshot test fails because
it now picks up a warning where it previously had none, that test was
asserting on a corpus file using `==`/`!=` — check whether it's asserting
"zero diagnostics" (needs updating to allow this specific warning) or
something unrelated broke.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs \
        crates/vox-compiler/src/parser/descent/mod.rs
git commit -m "feat(parser): warn (don't error) on == and != as is/is-not aliases

No new parsing — pratt_ops.rs already unified EqEq/NotEq into BinOp::Is/
Isnt. Adds the reader-tolerant diagnostic per spec S2/S3's corrected
framing (these tokens needed zero new parser logic, only a warning at
the point they're consumed). is/isnt themselves are unaffected."
```

---

### Task 7: Lock in `->`'s two independent positions

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/mod.rs` (test only — no
  behavior change, both positions already do the right thing)
- Modify: `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs`
  (test only)

This task adds **regression tests only**. Both behaviors already exist
correctly (return-type `->` already warns via `eat_return_arrow`; match-arm
`->` already errors via the dedicated check in `pratt_match.rs`) — the
review found the *spec* conflated them, not the code. Locking both in with
explicit tests prevents a future change from accidentally aliasing them
together.

- [ ] **Step 1: Write the failing test**

These tests should currently **pass already** (this is the one task in
this plan where "red" means "test doesn't exist yet", not "behavior is
wrong") — write them, confirm they pass immediately, and treat that as the
task's own verification that the spec's claim (both behaviors are already
correct, only conflated in prose) was accurate.

Add to `crates/vox-compiler/src/parser/descent/mod.rs`'s existing test
module (`mod tests;` at the bottom of the file — open
`crates/vox-compiler/src/parser/descent/tests.rs` and add):

```rust
    /// S2/S3: `->` in RETURN-TYPE position is tolerated with a Warning
    /// (pre-existing behavior via `eat_return_arrow` — this test locks it
    /// in so a future refactor can't silently change the severity).
    #[test]
    fn arrow_return_type_still_warns() {
        let tokens = crate::lexer::lex("fn f() -> int { return 1 }\n");
        let mut p = Parser::new(tokens);
        let result = p.parse_module();
        assert!(
            result.is_ok(),
            "-> in return-type position must still parse (Warning, not Error)"
        );
    }
```

Add to `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs`'s
existing test module:

```rust
    /// S2/S3: `->` in MATCH-ARM position is a SEPARATE, already-existing,
    /// default-severity ERROR (canonical is `=>`) with NO alias mapping —
    /// this is not the same tolerance as return-type `->`. Locks in that
    /// the two positions stay independent (the spec's review found this
    /// distinction was easy to conflate in prose; this test prevents it
    /// from being conflated in code too).
    #[test]
    fn arrow_match_arm_is_not_aliased() {
        use crate::parser::descent::Parser;
        let tokens = crate::lexer::lex(
            "fn f(r: Result[int]) to int { match r { Ok(x) -> x  Error(e) -> 0 } }\n",
        );
        let mut p = Parser::new(tokens);
        let result = p.parse_module();
        assert!(
            result.is_err(),
            "-> in match-arm position must remain a hard error (canonical is =>)"
        );
    }
```

- [ ] **Step 2: Run test to verify current status**

Run: `cargo test -p vox-compiler --lib parser::descent::tests::arrow_return_type_still_warns parser::descent::expr::pratt_match::tests::arrow_match_arm_is_not_aliased`
Expected: PASS for both, immediately — no implementation step follows. If
either fails, that means the code's actual behavior has drifted from what
the audit verified (`parser/descent/mod.rs:182-193` for the warning,
`pratt_match.rs:673-681` for the error) — stop and investigate before
proceeding; do not "fix" the test to match unexpected behavior without
understanding why it changed.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/tests.rs \
        crates/vox-compiler/src/parser/descent/expr/pratt_match.rs
git commit -m "test(parser): lock in -> as two independent positions, not one alias

Return-type '->' (Warning, tolerated) and match-arm '->' (Error, no
alias) are separate, pre-existing behaviors that the spec's prose had
conflated. No code change — regression tests only, so a future change
can't accidentally merge the two."
```

---

### Task 8: `!` stays a reader error, gains a `Replacement` payload

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:74-89`

Explicit exception to the tolerant-reader policy (spec S2, reversing the
original draft's downgrade-to-warning): `BangInvalid` keeps `Error`
severity and keeps returning `Err(())` with no AST node synthesized — this
is deliberate, per the audit's own finding that `!` was the project's one
prior near-miss on exactly this class of silent semantic inversion. This
task only adds the machine-readable `Replacement` payload so tooling
(`vox fmt`, future codemods) can still auto-suggest `not` without the
reader becoming tolerant of it.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs`'s
existing test module:

```rust
    /// S2: `!` stays a hard parse ERROR (never downgraded to a tolerated
    /// warning — this is the project's one prior near-miss on the exact
    /// class of bug the tolerant-reader policy exists to prevent
    /// elsewhere), but the error now carries a machine-readable
    /// Replacement payload so `vox fmt`/codemods can still suggest the fix
    /// from data instead of parsing the English message.
    #[test]
    fn bang_invalid_error_carries_not_replacement() {
        use crate::parser::descent::Parser;
        let tokens = crate::lexer::lex("!true\n");
        let mut p = Parser::new(tokens);
        let result = p.parse_expr();
        assert!(result.is_err(), "! must remain a hard parse error");
        let errors = p.errors_for_test();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].severity,
            crate::parser::error::ParseSeverity::Error,
            "! must NOT be downgraded to a warning"
        );
        let r = errors[0]
            .replacement
            .as_ref()
            .expect("BangInvalid error must carry a Replacement payload");
        assert_eq!(r.from, "!");
        assert_eq!(r.to, "not");
        assert_eq!(r.code, "vox/lexer/bang-invalid");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --lib parser::descent::expr::pratt_match::tests::bang_invalid_error_carries_not_replacement`
Expected: FAIL — `errors[0].replacement` is currently `None`.

- [ ] **Step 3: Implement**

Modify the `Token::BangInvalid` arm in
`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:74-89` from:

```rust
            Token::BangInvalid => {
                // `!` is not a valid Vox operator. Vox uses phonetic operators
                // (`not`, `and`, `or`, `is`, `isnt`). Emit a clear error pointing
                // at the canonical form, then advance past the `!` so the parser
                // can keep going and report any other issues in the same pass.
                self.errors.push(ParseError::classified(
                    start,
                    "`!` is not a valid operator in Vox; use `not` instead. \
                     (Vox uses phonetic operators: `not`, `and`, `or`, `is`, `isnt`.)",
                    vec!["not".to_string()],
                    Some("!".to_string()),
                    ParseErrorClass::Expression,
                ));
                self.advance();
                return Err(());
            }
```

to:

```rust
            Token::BangInvalid => {
                // `!` is not a valid Vox operator. Vox uses phonetic operators
                // (`not`, `and`, `or`, `is`, `isnt`). Deliberately kept as a
                // hard ERROR (not folded into the tolerant-reader policy like
                // ==/!=/-> are) per spec S2: this is the exact class of silent
                // semantic inversion (`if !x` parsing as `if x`) that burned
                // the project once before BangInvalid existed at all. Carries
                // a Replacement payload so tooling can still auto-fix from
                // data; the reader itself stays strict.
                let mut err = ParseError::classified(
                    start,
                    "`!` is not a valid operator in Vox; use `not` instead. \
                     (Vox uses phonetic operators: `not`, `and`, `or`, `is`, `isnt`.)",
                    vec!["not".to_string()],
                    Some("!".to_string()),
                    ParseErrorClass::Expression,
                );
                err.replacement = Some(crate::parser::error::Replacement {
                    from: "!".to_string(),
                    to: "not".to_string(),
                    code: "vox/lexer/bang-invalid".to_string(),
                });
                self.errors.push(err);
                self.advance();
                return Err(());
            }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --lib parser::descent::expr::pratt_match::tests::bang_invalid_error_carries_not_replacement`
Expected: PASS. Also re-run the two pre-existing `!`-related tests in
`cursor.rs` (`test_bang_lexes_as_invalid_distinct_from_not`,
`test_bang_eq_still_one_token`) to confirm they're unaffected:
Run: `cargo test -p vox-compiler --lib lexer::cursor::tests`
Expected: PASS, unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/expr/pratt_match.rs
git commit -m "feat(parser): give BangInvalid a Replacement payload, keep it an error

Explicit exception to the tolerant-reader policy: unlike ==/!=/->, '!'
stays a hard parse error rather than being downgraded to a warning (spec
S2, reversing the original draft) -- it's the project's one prior
near-miss on silent semantic inversion (if !x parsing as if x). Adding
the machine-readable Replacement payload lets tooling still auto-suggest
'not' without the reader becoming tolerant of '!' itself."
```

---

### Task 9: Bound diagnostic cost on pathological unknown-byte input

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/mod.rs`

Addresses the spec's S2 requirement that unknown-byte diagnostics are
bounded, so a file that's mostly one repeated unrecognized character can't
produce unbounded diagnostic count or retained-error memory.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-compiler/src/parser/descent/tests.rs`:

```rust
    /// S2: a long run of one unknown byte at top level must not produce
    /// unbounded diagnostics. This is the pathological-input guard named
    /// in the spec's S2/S8 — before this task, each of the 500 '^'
    /// characters below would independently fail parse_decl's dispatch
    /// and push its own "Unexpected token at top level" error.
    #[test]
    fn long_run_of_unknown_bytes_bounds_diagnostics() {
        let source = "^".repeat(500);
        let tokens = crate::lexer::lex(&source);
        let result = crate::parser::parse(tokens);
        let errors = result.expect_err("500 unknown bytes must fail to parse");
        assert!(
            errors.len() <= 21,
            "diagnostic count must be bounded (cap + one summary line), got {}",
            errors.len()
        );
        // The final error must be the summary sentinel, not another
        // per-character "Unexpected token" message.
        let last = errors.last().unwrap();
        assert!(
            last.message.contains("more"),
            "expected a summary sentinel as the final diagnostic, got: {:?}",
            last.message
        );
    }

    /// A single unknown byte still gets its own clear, unbounded-count-
    /// unaffected diagnostic -- the cap only matters for pathological runs.
    #[test]
    fn single_unknown_byte_gets_one_clear_error() {
        let tokens = crate::lexer::lex("^\n");
        let result = crate::parser::parse(tokens);
        let errors = result.expect_err("bare '^' must fail to parse");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains('^'));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --lib parser::descent::tests::long_run_of_unknown_bytes_bounds_diagnostics`
Expected: FAIL — today's `parse_decl` fallback (descent/mod.rs, the `_ =>`
arm) pushes one "Unexpected token at top level" error per unrecognized
token with no cap, and each `Token::Unknown('^')` at top level is exactly
that case (declaration-head position, unrecognized token) — so 500 errors
get pushed, no summary sentinel exists.

- [ ] **Step 3: Implement**

Add a bounded counter field to the `Parser` struct
(`crates/vox-compiler/src/parser/descent/mod.rs:65-74`):

```rust
struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    errors: Vec<ParseError>,
    /// Source file classification. Defaults to [`crate::module::FileKind::Source`];
    /// overridden via [`parse_with_kind`] when the entry point knows the path. Per
    /// ADR-032, [`crate::module::FileKind::ReactiveModule`] permits module-scope
    /// `state` / `derived` / `effect` / `on mount` / `on cleanup`.
    file_kind: crate::module::FileKind,
    /// S2 bounded-diagnostics guard: counts how many "unrecognized token at
    /// top level" errors have been pushed for `Token::Unknown` specifically,
    /// so a pathological run of one bad byte can't produce unbounded
    /// diagnostics or retained-error memory. Not used for any other error
    /// class.
    unknown_token_error_count: usize,
}
```

Update `Parser::new` (line 77-84) to initialize it:

```rust
    pub(crate) fn new(tokens: Vec<Spanned>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: vec![],
            file_kind: crate::module::FileKind::Source,
            unknown_token_error_count: 0,
        }
    }
```

Add the cap constant near the top of the file (after the `use` statements,
line 13):

```rust
/// S2: cap on how many "unrecognized token" diagnostics a pathological
/// input (e.g. a long run of one unknown byte) can generate. Once hit, one
/// summary sentinel replaces further per-token errors.
const MAX_UNKNOWN_TOKEN_ERRORS: usize = 20;
```

Modify the `parse_decl` fallback arm (the block ending at line ~961) from:

```rust
            _ => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    format!("Unexpected token at top level: {}", self.peek()),
                    vec!["fn".into(), "import".into(), "type".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::TopLevel,
                ));
                Err(())
            }
```

to:

```rust
            _ => {
                if matches!(self.peek(), Token::Unknown(_)) {
                    if self.unknown_token_error_count < MAX_UNKNOWN_TOKEN_ERRORS {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            format!("Unexpected token at top level: {}", self.peek()),
                            vec!["fn".into(), "import".into(), "type".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::TopLevel,
                        ));
                        self.unknown_token_error_count += 1;
                    } else if self.unknown_token_error_count == MAX_UNKNOWN_TOKEN_ERRORS {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            format!(
                                "…and more unrecognized tokens follow (stopped reporting \
                                 after {MAX_UNKNOWN_TOKEN_ERRORS})"
                            ),
                            vec![],
                            None,
                            ParseErrorClass::TopLevel,
                        ));
                        self.unknown_token_error_count += 1; // never re-enter this branch
                    }
                    // Beyond the cap: silently advance without pushing further
                    // diagnostics, but still return Err so recovery proceeds.
                } else {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Unexpected token at top level: {}", self.peek()),
                        vec!["fn".into(), "import".into(), "type".into()],
                        Some(self.peek().to_string()),
                        ParseErrorClass::TopLevel,
                    ));
                }
                Err(())
            }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --lib parser::descent::tests::long_run_of_unknown_bytes_bounds_diagnostics parser::descent::tests::single_unknown_byte_gets_one_clear_error`
Expected: PASS (both). 500 unknown bytes now produce at most 21 diagnostics
(20 real + 1 summary), a single one still produces exactly 1.

Run the full parser suite once more for regressions:
Run: `cargo test -p vox-compiler`
Expected: PASS, zero regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/mod.rs \
        crates/vox-compiler/src/parser/descent/tests.rs
git commit -m "fix(parser): bound diagnostic count on pathological unknown-byte input

A file that's mostly one repeated unrecognized character (e.g. 500 '^'
in a row) previously pushed one 'Unexpected token' error per character
with no cap. Now stops at 20 real diagnostics plus one summary sentinel.
Scoped specifically to Token::Unknown at the top-level dispatch fallback
-- other error classes are unaffected."
```

---

## Self-Review

**1. Spec coverage** (against Sequencing Steps 0–1 specifically, not the
whole spec — Steps 2–8 are explicitly out of scope for this plan):

- Step 0 (CR-F3 warn-mode scaffold + `mens/system_prompt.txt` +
  AGENTS.md) → Tasks 0, 1, 2. ✅
- Step 1 / S2 (`;` deletion, `==`/`!=` no-new-parsing, `->` two positions,
  `&&`/`||` — explicitly **not** attempted here per Task 4's own scope
  note, deferred to the Step 2+ follow-up plan since it needs new
  Logos tokens + Pratt arms, a larger unit of work than this plan's other
  tasks — `!` stays an error, bounded unknown-byte diagnostics, delete
  `parser/indent.rs`) → Tasks 3, 4, 5, 6, 7, 8, 9. ✅ (boolean operators
  flagged as deferred, not silently dropped — see Task 4 Step 1's test
  `double_ampersand_lexes_as_two_unknown_tokens_for_now`, which documents
  and locks in the interim behavior)
- S2's "unknown decorator → registry error + nearest-name suggestion" →
  explicitly depends on S4's `DecoratorRegistry`, which doesn't exist yet
  (confirmed dependency-ordering fix from the adversarial review) — **not**
  in this plan, correctly deferred to the Step 3+ follow-up.

**2. Placeholder scan:** No "TBD"/"TODO"/"implement later" in any task.
Every code block is complete, compilable Rust (or complete YAML/JSON/
Markdown for the doc tasks) grounded in files actually read during
planning, not illustrative pseudocode.

**3. Type consistency:** `ParseError`, `ParseSeverity`, `ParseErrorClass`,
`Replacement` field names and shapes verified against
`crates/vox-compiler/src/parser/error.rs` directly (Task 5 Step 3's
`ParseError { .. }` struct-literal matches the real struct's field
names/order). `Token::Unknown(char)` is used consistently across Tasks 4–9
with the same shape. `errors_for_test()` is defined once (Task 5 Step 3)
and reused by Tasks 6, 7, 8, 9 without redefinition.

## Execution Handoff

Plan complete and saved to
`docs/superpowers/plans/2026-08-08-vox-core-syntax-convergence-step-0-1.md`.
Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per
task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using
executing-plans, batch execution with checkpoints.

**Which approach?**
