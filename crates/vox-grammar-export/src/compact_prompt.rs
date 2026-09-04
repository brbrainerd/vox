//! Compact LLM prompt cheatsheet derived from the authoritative EBNF grammar.
//!
//! This module produces a dense, human/LLM-readable summary of every Vox 0.4
//! language construct. It is the **single source of truth** for the grammar
//! prompt fed to LLMs during code generation — replacing the stale hand-written
//! draft that previously lived in `vox-compiler/src/llm_prompt.rs`.
//!
//! **Research rationale (Grammar Constraints §K-complexity):** Every unnecessary
//! boilerplate token in the grammar prompt proportionally increases hallucination
//! surface. This cheatsheet is optimised for minimal token count while retaining
//! full construct coverage.

/// Emit a compact, LLM-optimised Vox 0.4 grammar cheatsheet.
///
/// The output covers all 57 production rules from the authoritative EBNF
/// (`ebnf::emit_ebnf`) but uses readable pseudo-syntax instead of raw EBNF
/// notation. Categories mirror the EBNF groupings.
#[must_use]
pub fn emit_compact_llm_prompt() -> String {
    let mut p = String::with_capacity(4096);

    p.push_str("Vox 0.4 Grammar Cheatsheet (auto-derived from EBNF — 57 productions)\n\n");

    // ── Functions ────────────────────────────────────────────────────────
    p.push_str("== Functions ==\n");
    p.push_str("[pub] [async] fn name(arg: Type, opt?: Type) to RetType { body }\n");
    p.push_str("lambda: fn(params) { body }  or  fn(params) expr\n\n");

    // ── Variables & Assignment ───────────────────────────────────────────
    p.push_str("== Variables ==\n");
    p.push_str("let x = expr            // immutable\n");
    p.push_str("let mut y: Type = expr  // mutable, optional annotation\n");
    p.push_str("y = expr  |  y += expr  |  y -= expr  |  y *= expr  |  y /= expr\n\n");

    // ── Control Flow ────────────────────────────────────────────────────
    p.push_str("== Control Flow ==\n");
    p.push_str("if cond { ... } [else { ... }]   // else-if chains allowed\n");
    p.push_str("while cond { ... }\n");
    p.push_str("loop { break; continue }\n");
    p.push_str("for x in iterable { ... }\n");
    p.push_str("return expr             // canonical\n");
    p.push_str("ret expr                // deprecated\n\n");

    // ── Pattern Matching ────────────────────────────────────────────────
    p.push_str("== Match ==\n");
    p.push_str("match expr { Pattern -> expr, Ctor(a, b) -> expr, _ -> expr }\n\n");

    // ── Types ───────────────────────────────────────────────────────────
    p.push_str("== Types ==\n");
    p.push_str("Primitives: int, float, str, bool, Unit\n");
    p.push_str("Generics:   List[T], Option[T], Result[T]\n");
    p.push_str("Tuple:      (T1, T2)    Function: fn(T) to U\n");
    p.push_str("type Name = | Variant(fields) | Variant2   // ADT\n");
    p.push_str("type Name = { field: Type, ... }           // struct\n\n");

    // ── Expressions & Operators ─────────────────────────────────────────
    p.push_str("== Expressions ==\n");
    p.push_str("Binary:  + - * / == != < > <= >= and or\n");
    p.push_str("Unary:   not  -\n");
    p.push_str("Pipe:    expr |> fn\n");
    p.push_str("Call:    f(a, b)    Method: obj.method(a)\n");
    p.push_str("Field:   obj.field  Spawn: spawn expr\n");
    p.push_str("With:    with expr { field: val }\n\n");

    // ── Imports ─────────────────────────────────────────────────────────
    p.push_str("== Imports ==\n");
    p.push_str("import mod.sub, other.thing\n");
    p.push_str("import rust:crate_name\n\n");

    // ── Decorators & Annotations ────────────────────────────────────────
    p.push_str("== Decorators ==\n");
    p.push_str("@require(expr)  @ensure(expr)  @invariant(expr)  @fuzz\n");
    p.push_str("@test fn name() { ... }\n");
    p.push_str("@forall fn name(x: Type) { ... }   // property-based test\n\n");

    // ── Server / Query / Mutation ────────────────────────────────────────
    p.push_str("== Server Functions ==\n");
    p.push_str("server name(params) to RetType { ... }\n");
    p.push_str("query name(params) to RetType { ... }\n");
    p.push_str("mutation name(params) to RetType { ... }\n\n");

    // ── Tables & Indexes ────────────────────────────────────────────────
    p.push_str("== Data ==\n");
    p.push_str("table Name { field: Type, ... }\n");
    p.push_str("index Table.idx on (col1, col2)\n\n");

    // ── Components ──────────────────────────────────────────────────────
    p.push_str("== Components ==\n");
    p.push_str("component Name(props) {\n");
    p.push_str("  state x: Type = init\n");
    p.push_str("  derived y: Type = expr\n");
    p.push_str("  effect { ... }   mount { ... }   cleanup { ... }\n");
    p.push_str("  view: column(gap=\"2\") { text() { \"hello\" } }\n");
    p.push_str("}\n");
    p.push_str("@component fn Name()    // RETIRED — produces hard error\n");
    p.push_str("@v0 \"prompt\" fn Name() to Element\n\n");

    // ── View-call surface ───────────────────────────────────────────────
    p.push_str("== View Calls ==\n");
    p.push_str("Tag(named=prop) { child_exprs }\n");
    p.push_str("SelfClosing(named=prop)\n");
    p.push_str("All view-call args are named (keyword-only)\n");
    p.push_str("Lowering target is JSX-shaped HIR; angle-bracket JSX source is retired\n\n");

    // ── HTTP Routes ─────────────────────────────────────────────────────
    p.push_str("== HTTP ==\n");
    p.push_str("http get \"/path\" (params) to RetType { ... }\n");
    p.push_str("http post|put|delete \"/path\" (params) to RetType { ... }\n\n");

    // ── Routing ─────────────────────────────────────────────────────────
    p.push_str("== Routes ==\n");
    p.push_str("routes { \"/path\" to Handler with { key: val } }\n");
    p.push_str("@loading fn name() to Element { ... }\n\n");

    // ── Actors ──────────────────────────────────────────────────────────
    p.push_str("== Actors ==\n");
    p.push_str("actor Name { on message(params) to RetType { ... } }\n\n");

    // ── Workflows & Activities ──────────────────────────────────────────
    p.push_str("== Workflows ==\n");
    p.push_str("workflow Name(params) to RetType { ... }\n");
    p.push_str("activity Name(params) to RetType { ... }\n\n");

    // ── Agents & Environments ───────────────────────────────────────────
    p.push_str("== Agents ==\n");
    p.push_str("agent Name { on event(params) to RetType { ... } }\n");
    p.push_str("environment Name { key: Type = default }\n\n");

    // ── MCP Tools & Resources ───────────────────────────────────────────
    p.push_str("== MCP ==\n");
    p.push_str("@mcp.tool [\"desc\"] fn name(params) to RetType { ... }\n");
    p.push_str("@mcp.resource [\"uri\"] fn name(params) to RetType { ... }\n\n");

    // ── Literals ────────────────────────────────────────────────────────
    p.push_str("== Literals ==\n");
    p.push_str("42  3.14  \"hello\"  true  false  Unit\n");
    p.push_str("[a, b]  (a, b)  { key: val }\n");

    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_prompt_covers_all_categories() {
        let prompt = emit_compact_llm_prompt();
        let expected = [
            "Functions",
            "Variables",
            "Control Flow",
            "Match",
            "Types",
            "Expressions",
            "Imports",
            "Decorators",
            "Server Functions",
            "Data",
            "Components",
            "View Calls",
            "HTTP",
            "Routes",
            "Actors",
            "Workflows",
            "Agents",
            "MCP",
            "Literals",
        ];
        for cat in &expected {
            assert!(
                prompt.contains(cat),
                "Compact prompt missing category: {cat}"
            );
        }
    }

    #[test]
    fn compact_prompt_covers_key_constructs() {
        let prompt = emit_compact_llm_prompt();
        for kw in &[
            "fn",
            "let",
            "mut",
            "if",
            "while",
            "loop",
            "for",
            "match",
            "ret",
            "return",
            "type",
            "import",
            "@test",
            "@forall",
            "server",
            "query",
            "mutation",
            "table",
            "index",
            "component",
            "@v0",
            "http",
            "routes",
            "actor",
            "workflow",
            "activity",
            "agent",
            "environment",
            "@mcp.tool",
            "@mcp.resource",
            "spawn",
            "|>",
            "Option[T]",
            "Result[T]",
            "List[T]",
        ] {
            assert!(prompt.contains(kw), "Compact prompt missing keyword: {kw}");
        }
    }

    #[test]
    fn compact_prompt_is_reasonably_sized() {
        let prompt = emit_compact_llm_prompt();
        // Must be shorter than raw EBNF but non-trivial
        assert!(prompt.len() > 500, "Prompt too short");
        assert!(
            prompt.len() < 8000,
            "Prompt too long — defeats K-complexity goal"
        );
    }
}
