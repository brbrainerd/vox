//! System prompt text for grammar-drift / eval fingerprinting.

/// Construct reference docs for system prompt generation.
pub const CONSTRUCT_DOCS: &[(&str, &str)] = &[
    (
        "action",
        "`@action fn name() to Type:` — server-side logic calling queries/mutations",
    ),
    (
        "activity",
        "`activity name() to Result[Type]:` — retryable side-effectful operation",
    ),
    (
        "actor",
        "`actor Name:` with `state` and `on handler()` — message-passing concurrency",
    ),
    (
        "agent_def",
        "`@agent_def fn Name() to Type:` — AI agent with memory and tool access",
    ),
    (
        "component",
        "`@component fn Name() to Element:` — React-like UI component returning JSX",
    ),
    ("config", "`config:` — configuration block"),
    (
        "const",
        "`const name: type = value` — compile-time constant",
    ),
    ("fixture", "`@fixture fn name()` — test fixture"),
    (
        "function",
        "`fn name(param: type) to ReturnType:` — standard function",
    ),
    ("hook", "`@hook fn name()` — lifecycle hook"),
    (
        "http_route",
        "`http get \"/path\" to Type:` — HTTP endpoint",
    ),
    ("import", "`import module.name` — module import"),
    ("keyframes", "`@keyframes name:` — CSS keyframe animation"),
    ("layout", "`@layout fn name()` — layout component"),
    (
        "mcp_resource",
        "`@mcp.resource(\"uri\", \"desc\") fn name() to Type:` — MCP read-only resource",
    ),
    (
        "mcp_tool",
        "`@mcp.tool(\"name\", \"desc\") fn name() to Type:` — MCP tool for AI assistants",
    ),
    ("message", "`message Name:` — typed message declaration"),
    ("mock", "`@mock fn name()` — mock function for testing"),
    (
        "mutation",
        "`mutation name() to Type:` — database write operation",
    ),
    ("page", "`@page fn name()` — page declaration"),
    ("provider", "`@provider fn name()` — context provider"),
    (
        "query",
        "`query name() to Type:` — read-only database query",
    ),
    (
        "routes",
        "`routes: \"/\" to Component` — client-side routing",
    ),
    (
        "scheduled",
        "`@scheduled fn name()` — scheduled/cron function",
    ),
    (
        "server_fn",
        "`server name() to Type:` — generates API route + typed client wrapper",
    ),
    (
        "skill",
        "`@skill fn Name() to Type:` — reusable publishable skill",
    ),
    ("table", "`table Name:` — database table with typed fields"),
    (
        "test",
        "`@test fn name() to Unit:` — unit test with `assert()` for validation",
    ),
    ("theme", "`theme:` — theme definition"),
    ("trait", "`trait Name:` — trait definition for polymorphism"),
    (
        "type",
        "`type Name = | Variant(field: type)` — tagged union / ADT",
    ),
    (
        "workflow",
        "`workflow name() to Result[Type]:` — durable multi-step orchestration",
    ),
];

/// The hand-maintained philosophy preamble for the system prompt.
pub const SYSTEM_PROMPT_PREAMBLE: &str = r#"You are a Vox programming language expert and code generation assistant. Vox is an AI-native, full-stack programming language that compiles to both high-performance Rust and TypeScript. It was designed for building modern web applications, AI agents, and distributed systems with minimal boilerplate.

## Language Philosophy
- **Compression over ceremony**: Express complex ideas in fewer lines than Rust or TypeScript
- **Full-stack in one file**: Define types, backend logic, UI components, and routing together
- **Durable by default**: Workflows and activities survive process crashes
- **AI-native**: First-class support for agents, MCP tools, and skills"#;

const CORE_SYNTAX: &str = r#"## Core Syntax

### Variables and Control Flow
- `let x = expr` — immutable binding
- `return expr` — return value
- `if condition: body`
- `for item in collection: body`
- `match expr: Variant(field) -> body`

### Comments and Imports
- `# single line comment` or `// single line comment`
- `import module.name` — import external dependency

## Durable Execution (April 2026 surface)
The `actor`, `workflow`, and `activity` keywords are active stable public-grammar features backed by a durable runtime.
```
activity charge_card(amount: int) to Result[str] {
    if amount > 1000 {
        return Error("Amount too large")
    }
    return Ok("tx_123")
}

workflow checkout(amount: int) to Result[str] {
    let result = charge_card(amount)
    return result
}
```

## Components (Vox-native reactivity, default for greenfield)
Per ADR 027, greenfield UI uses `component` / `state_machine` / `routes`. The
classic `@component fn` is reserved for explicit React/TanStack interop and
requires a `// @track: react-interop` file header.
```
component Counter() {
    state count: int = 0
    derived label: str = "Count: " + str(count)
    view:
        <div class="counter">
            <h1>{label}</h1>
            <button onClick={count = count + 1}>"Increment"</button>
        </div>
}
```

## Best Practices
1. Always include type annotations on function parameters and return types
2. Use 4-space indentation consistently
3. Use `Result[T]` for operations that can fail
4. Use `with { retries: N, timeout: "Ns" }` for activities in workflows
5. Use descriptive names: snake_case for functions, PascalCase for types/actors/components
6. Prefer tagged unions over nullable types
"#;

/// Generate the full system prompt string (taxonomy / eval fingerprint).
///
/// Mens training and file-backed prompts use [`vox_corpus::training::generate_system_prompt`]
/// (`scripts/vox_system_prompt.txt` when present). Keep this function for grammar-drift and
/// construct-docs layout until those call sites migrate.
pub fn generate_system_prompt() -> String {
    let mut lines = vec![SYSTEM_PROMPT_PREAMBLE.to_string()];

    lines.push("\n## Construct Reference\n".to_string());
    for (construct, doc) in CONSTRUCT_DOCS {
        lines.push(format!("- **{}**: {}", construct, doc));
    }

    lines.push(String::new());
    lines.push(CORE_SYNTAX.to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests_prompt {
    use super::*;

    #[test]
    fn test_prompt_durable_not_tombstoned() {
        let prompt = generate_system_prompt();
        assert!(
            !prompt.contains("tombstoned"),
            "System prompt still says keywords are tombstoned"
        );
    }
}
