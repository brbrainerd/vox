# Semantic Behavior Map — `vox-grammar-export`

Deterministically synthesized from 34 distinct proven-behavior claims (of 34 extracted) across 8 symbols. 2 symbols have an explicit error-path proof; **3 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `emit_json_schema`  (happy; EXTRACTED)
- [happy] emit_json_schema() returns a non-empty string  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_json_schema() output is valid JSON that parses successfully  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_json_schema() output contains $schema key with value 'https://json-schema.org/draft/2020-12/schema'  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_json_schema() output contains $defs object for schema definitions  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_json_schema() $defs contains all core node definitions: Module, FnDecl, LetDecl, TypeDecl, Block, Expr, Literal, BinaryExpr, CallExpr, IfExpr, RecordLit, TupleLit, ArrayLit, FieldAccess  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_json_schema() output contains BinaryExpr.op as an enum array  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_json_schema() BinaryExpr.op enum includes + and == operators  (crates/vox-grammar-export/tests/export_test.rs)

### `emit_ebnf`  (edge, happy, invariant; EXTRACTED)
- [happy] emit_ebnf() returns a string containing the actor declaration type  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_ebnf() returns a string containing the @mcp.tool decorator  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_ebnf() returns a non-empty string  (crates/vox-grammar-export/tests/export_test.rs)
- [invariant] emit_ebnf() output contains at least 10 lines  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_ebnf() output exposes view_call_expr as a parser-facing surface  (crates/vox-grammar-export/tests/export_test.rs)
- [edge] emit_ebnf() output does not reference retired parser file names like pratt_jsx.rs  (crates/vox-grammar-export/tests/export_test.rs)

### `emit_lark`  (edge, happy; EXTRACTED)
- [happy] emit_lark() returns a non-empty string  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_lark() output defines 'start: module' rule  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_lark() output includes fn_decl and IDENT terminal definitions  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_lark() contains all expected grammar constructs and terminals  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_lark() output exposes view_call_expr as a parser-facing surface  (crates/vox-grammar-export/tests/export_test.rs)
- [edge] emit_lark() output does not reference retired parser file names like pratt_jsx.rs  (crates/vox-grammar-export/tests/export_test.rs)

### `export`  (error, happy; EXTRACTED)
- [happy] export() with GrammarFormat::Ebnf returns Ok with non-empty grammar_text  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] export() with GrammarFormat::Lark returns Ok with non-empty grammar_text  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] export() with GrammarFormat::JsonSchema returns Ok with non-empty grammar_text  (crates/vox-grammar-export/tests/export_test.rs)
- [error] export() with GrammarFormat::Gbnf returns Err with CVE-2026-2069 reference  (crates/vox-grammar-export/tests/export_test.rs)
- [error] export() with GrammarFormat::TreeSitterGrammar returns Err with 'not yet implemented' message  (crates/vox-grammar-export/tests/export_test.rs)

### `emit_compact_llm_prompt`  (happy; EXTRACTED)
- [happy] emit_compact_llm_prompt() returns a string containing all 19 expected category names including Functions, Variables, Control Flow, Match, Types, Expressions, Imports, Decorators, Server Functions, Data, Components, View Calls, HTTP, Routes, Actors, Workflows, Agents, MCP, and Literals  (crates/vox-grammar-export/src/compact_prompt.rs)
- [happy] emit_compact_llm_prompt() returns a non-empty string  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_compact_llm_prompt() contains version header 'Vox 0.4 Grammar Cheatsheet'  (crates/vox-grammar-export/tests/export_test.rs)

### `emit_gbnf`  (error, happy; EXTRACTED)
- [happy] emit_gbnf() returns a string containing GBNF rule definitions matching 'root ::= expr'  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] emit_gbnf() output contains expr rule with all expected alternatives  (crates/vox-grammar-export/tests/export_test.rs)
- [error] emit_gbnf() output does not contain direct left-recursion pattern where expr ::= expr starts a rule  (crates/vox-grammar-export/tests/export_test.rs)

### `get_version`  (invariant; EXTRACTED)
- [invariant] get_version() returns a value equal to get_compiler_version()  (crates/vox-grammar-export/tests/export_test.rs)
- [invariant] get_version() returns the same value as get_compiler_version()  (crates/vox-grammar-export/tests/export_test.rs)

### `verify_grammar_alignment`  (happy; EXTRACTED)
- [happy] verify_grammar_alignment() returns Ok(_) when called  (crates/vox-grammar-export/tests/export_test.rs)
- [happy] verify_grammar_alignment() returns Ok  (crates/vox-grammar-export/tests/export_test.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`emit_compact_llm_prompt`** — only: _emit_compact_llm_prompt() returns a string containing all 19 expected category names including Functions, Variables, Control Flow, Match, Types, Expressions, Imports, Decorators, Server Functions, Data, Components, View Calls, HTTP, Routes, Actors, Workflows, Agents, MCP, and Literals_
- **`emit_json_schema`** — only: _emit_json_schema() returns a non-empty string_
- **`verify_grammar_alignment`** — only: _verify_grammar_alignment() returns Ok(_) when called_
