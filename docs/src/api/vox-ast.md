# Crate API: vox-ast

## Overview

Strongly-typed Abstract Syntax Tree for the Vox language.

## Purpose

Provides typed wrappers around the parser's untyped Concrete Syntax Tree (CST) nodes. This is the API that downstream stages (HIR lowering, type checking) consume.

## Key Files

| File | Purpose |
|------|---------|
| `decl.rs` | `Decl` — function, type, actor, workflow, activity, table, decorator declarations |
| `expr.rs` | `Expr` — literals, binary ops, calls, match, spawn, JSX, lambda |
| `stmt.rs` | `Stmt` — let bindings, return, assignment, expression statements |
| `pattern.rs` | `Pattern` — destructuring and match patterns for ADTs |
| `types.rs` | `TypeExpr` — type annotations, generics, and type parameters |
| `span.rs` | `Span` — source location tracking (start/end offsets) |

## Design

Each AST node type is an enum with variants for every language construct. All nodes carry `Span` information for error reporting and LSP integration.

```
Module
├── Vec<Decl>
│   ├── FnDecl { name, params, return_type, body, decorators }
│   ├── TypeDecl { name, variants }
│   ├── ActorDecl { name, state, handlers }
│   ├── WorkflowDecl { name, params, body }
│   └── ...
├── Vec<Stmt>
└── Vec<Route>
```

---

### `struct ConstDecl`

Constant declaration.


### `struct ConfigDecl`

A block of typed configuration / secrets.
`@config env: DATABASE_URL: str, API_KEY: str`


### `struct TableDecl`

Table declaration: a persistent record type.


### `struct TableField`

A field within a table declaration.


### `struct CollectionDecl`

Collection declaration: a schemaless document collection.


### `struct IndexDecl`

Index declaration for a table.


### `struct VectorIndexDecl`

Vector index declaration.


### `struct SearchIndexDecl`

Search index definition (e.g. FTS5 / Convex searchIndex).


### `struct FnDecl`

Function declaration.


### `struct ComponentDecl`

Component declaration (wraps a function with @component semantics).


### `struct StyleBlock`

A scoped style block within a component.


### `struct TestDecl`

Test declaration (wraps a function with @test semantics).


### `struct ServerFnDecl`

Server function declaration (wraps a function with @server semantics).


### `struct QueryDecl`

Query declaration: a read-only database function.


### `struct MutationDecl`

Mutation declaration: a write database function with transaction semantics.


### `struct ActionDecl`

Action declaration: server-side logic that can call queries and mutations.


### `struct SkillDecl`

Skill declaration: a modular AI capability.


### `struct AgentDefDecl`

Agent definition declaration: defines the core logic and interface for an AI agent.


### `struct ScheduledDecl`

Scheduled function declaration — runs at a fixed interval or cron schedule.


### `struct McpToolDecl`

MCP tool declaration.


### `struct MockDecl`

Mock declaration for testing.


### `struct HookDecl`

A frontend hook function declaration.


### `struct FixtureDecl`

Fixture declaration: setup code for tests.


### `struct ActorDecl`

Actor declaration.


### `struct ActorHandler`

Actor handler definition: `on receive(msg: str) to Unit:`


### `struct AgentDecl`

Native agent declaration


### `struct AgentHandler`

Agent handler definition: `on Event(msg) to Type:`


### `struct MigrationRule`

Agent migration rule: `migrate from "1.0":`


### `struct MessageDecl`

Native message declaration


### `struct WorkflowDecl`

Workflow declaration (durable execution).


### `struct ActivityDecl`

Activity declaration (durable execution side-effect).


### `struct HttpRouteDecl`

HTTP route declaration.


### `enum HttpMethod`

HTTP method for route declarations.


### `struct ImportPath`

An import path segment: `react.use_state`


### `struct ImportDecl`

Import declaration: `import react.use_state, network.HTTP`


### `enum Decl`

All top-level declaration types in Vox.


### `struct Module`

A complete Vox source module (one file).


### `struct Variant`

ADT variant in a type definition.


### `struct VariantField`

A field within an ADT variant.


### `struct TypeDefDecl`

Type / ADT / struct declaration.


### `struct TraitDecl`

Trait declaration.


### `struct TraitMethod`

A method signature within a trait.


### `struct ImplDecl`

Trait implementation for a specific type.


### `struct V0ComponentDecl`

v0.dev AI-generated component declaration.


### `struct RoutesDecl`

Client-side routing declaration.


### `struct ContextDecl`

Frontend React Context wrapper.


### `struct ProviderDecl`

A frontend provider component declaration.


### `struct LayoutDecl`

Layout component wrapper — wraps child routes with shared UI.


### `struct LoadingDecl`

Loading state component — shown during route suspense.


### `struct NotFoundDecl`

404 / not-found page component.


### `struct ErrorBoundaryDecl`

Error boundary component — catches render errors.


### `struct KeyframeDecl`

CSS keyframes declaration.


### `struct KeyframeStep`

A single keyframe step (e.g., `from:`, `to:`, `50%:`).


### `struct ThemeDecl`

Theme declaration with light/dark variants.


### `struct Arg`

A function/method argument, potentially named.


### `enum BinOp`

Binary operators


### `enum UnOp`

Unary operators


### `struct MatchArm`

A match arm: pattern [if guard] -> body


### `struct JsxElement`

A JSX element: <tag attrs...>children</tag>


### `struct JsxSelfClosingElement`

A self-closing JSX element: <tag attrs.../>


### `struct JsxAttribute`

A JSX attribute: name={expr} or name="string"


### `struct Param`

Parameter for functions and lambdas.


### `enum StringPart`

Parts of a string interpolation.


### `enum Expr`

All expression types in Vox.


## Module: `vox-ast\src\lib.rs`

# vox-ast

Strongly-typed Abstract Syntax Tree for the Vox language.

Provides typed wrappers (`Decl`, `Expr`, `Stmt`, `Pattern`) around the
parser's untyped CST nodes. All nodes carry [`Span`] information for
error reporting and LSP integration.


### `enum Pattern`

Pattern matching nodes for let bindings and match arms.


### `struct Span`

Source span for tracking positions in source code.


### `enum Stmt`

All statement types in Vox.


### `enum TypeExpr`

Type expressions in Vox source code.


