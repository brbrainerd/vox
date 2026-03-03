//! Shared training data utilities for the Vox CLI.
//!
//! Provides construct extraction, JSONL record emission, and instruction
//! template generation. Used by `vox check --emit-training-jsonl` and
//! `vox corpus` subcommands.
pub mod native;
pub mod datagen;

use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Schema version — must match `learn.rs` and `dogfood_train.py`.
pub const SCHEMA_VERSION: &str = "vox_dogfood_v1";

/// Walk a directory recursively and collect all `.vox` files.
pub fn walk_vox_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    walk_recursive(dir, &mut result);
    result.sort();
    result
}

fn walk_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_recursive(&path, out);
        } else if path.extension().map_or(false, |e| e == "vox") {
            out.push(path);
        }
    }
}

/// Generate a UTC timestamp string like "20260302T064530Z" without chrono.
pub fn timestamp_string() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Simple epoch-seconds based ID — good enough for run tags
    format!("ts_{}", secs)
}

/// Extract construct tags from AST declarations for training data categorization.
pub fn extract_constructs(module: &vox_ast::decl::Module) -> Vec<String> {
    use vox_ast::decl::Decl;
    let mut constructs = Vec::new();
    for decl in &module.declarations {
        let tag = match decl {
            Decl::Function(_) => "function",
            Decl::Component(_) => "component",
            Decl::TypeDef(_) => "type",
            Decl::Import(_) => "import",
            Decl::PyImport(_) => "py_import",
            Decl::Actor(_) => "actor",
            Decl::Const(_) => "const",
            Decl::Workflow(_) => "workflow",
            Decl::Activity(_) => "activity",
            Decl::HttpRoute(_) => "http_route",
            Decl::McpTool(_) => "mcp_tool",
            Decl::McpResource(_) => "mcp_resource",
            Decl::Test(_) => "test",
            Decl::ServerFn(_) => "server_fn",
            Decl::Table(_) => "table",
            Decl::Collection(_) => "collection",
            Decl::Index(_) => "index",
            Decl::VectorIndex(_) => "vector_index",
            Decl::SearchIndex(_) => "search_index",
            Decl::V0Component(_) => "v0_component",
            Decl::Routes(_) => "routes",
            Decl::Trait(_) => "trait",
            Decl::Impl(_) => "impl",
            Decl::Query(_) => "query",
            Decl::Mutation(_) => "mutation",
            Decl::Action(_) => "action",
            Decl::Skill(_) => "skill",
            Decl::AgentDef(_) => "agent_def",
            Decl::Agent(_) => "agent",
            Decl::Message(_) => "message",
            Decl::Scheduled(_) => "scheduled",
            Decl::Config(_) => "config",
            Decl::Context(_) => "context",
            Decl::Hook(_) => "hook",
            Decl::Provider(_) => "provider",
            Decl::Fixture(_) => "fixture",
            Decl::Layout(_) => "layout",
            Decl::Loading(_) => "loading",
            Decl::NotFound(_) => "not_found",
            Decl::ErrorBoundary(_) => "error_boundary",
            Decl::Keyframes(_) => "keyframes",
            Decl::Theme(_) => "theme",
            Decl::Mock(_) => "mock",
            Decl::Environment(_) => "environment",
            Decl::Page(_) => "page",
        };
        constructs.push(tag.to_string());
    }
    constructs.sort();
    constructs.dedup();
    constructs
}

/// Build a training JSONL record from a successful frontend result.
pub fn build_training_record(
    file: &Path,
    result: &crate::pipeline::FrontendResult,
) -> Result<serde_json::Value> {
    let content_hash = vox_runtime::builtins::vox_hash_fast(&result.source);

    let constructs = extract_constructs(&result.module);

    let record = serde_json::json!({
        "source": file.to_string_lossy(),
        "code": result.source,
        "constructs": constructs,
        "ast_hash": content_hash,
        "compiler_version": env!("CARGO_PKG_VERSION"),
    });

    Ok(record)
}

/// Append a JSONL record to the output file (creating it if necessary).
pub fn append_jsonl(
    output_path: &Path,
    file: &Path,
    result: &crate::pipeline::FrontendResult,
) -> Result<()> {
    let record = build_training_record(file, result)?;
    let line = serde_json::to_string(&record)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_path)?;
    writeln!(f, "{}", line)?;

    Ok(())
}

// ── Instruction pair templates ───────────────────────────────────────────

/// Instruction templates keyed by construct type.
/// Each entry is a list of template strings where `{name}` is replaced
/// with the primary identifier extracted from the code.
pub fn instruction_templates(construct: &str) -> &[&str] {
    match construct {
        "function" => &[
            "Write a Vox function called {name}",
            "Implement the {name} function using Vox syntax",
        ],
        "component" => &[
            "Write a Vox UI component called {name}",
            "Create a {name} component in Vox using JSX syntax",
        ],
        "actor" => &[
            "Write a Vox actor called {name} with state management",
            "Create a {name} actor in Vox using the actor model",
        ],
        "workflow" => &[
            "Write a Vox durable workflow called {name}",
            "Create a {name} workflow in Vox with retry policies",
        ],
        "activity" => &[
            "Write a Vox activity called {name}",
            "Create a retryable {name} activity in Vox",
        ],
        "table" => &[
            "Define a Vox database table called {name}",
            "Create a {name} table using @table in Vox",
        ],
        "query" => &[
            "Write a Vox database query called {name}",
            "Create a read-only query {name} in Vox",
        ],
        "mutation" => &[
            "Write a Vox database mutation called {name}",
            "Create a {name} mutation in Vox for data modification",
        ],
        "action" => &[
            "Write a Vox server action called {name}",
        ],
        "type" => &[
            "Define a Vox tagged union type called {name}",
            "Create a {name} ADT in Vox with typed variants",
        ],
        "test" => &[
            "Write a Vox test for {name}",
            "Create unit tests in Vox using @test and assert",
        ],
        "mcp_tool" => &[
            "Write a Vox MCP tool called {name}",
            "Create an MCP-compatible tool in Vox for AI assistants",
        ],
        "mcp_resource" => &[
            "Write a Vox MCP resource for {name}",
        ],
        "http_route" => &[
            "Write an HTTP route in Vox",
            "Create an HTTP endpoint in Vox",
        ],
        "routes" => &[
            "Define client-side routes in Vox",
        ],
        "server_fn" => &[
            "Write a Vox server function called {name}",
        ],
        "skill" => &[
            "Write a Vox skill called {name}",
        ],
        "agent_def" => &[
            "Define a Vox AI agent called {name}",
        ],
        "trait" => &[
            "Define a Vox trait called {name}",
        ],
        _ => &[
            "Write Vox code demonstrating {name}",
        ],
    }
}

/// Extract the primary name from a Vox source string.
pub fn extract_name_from_source(code: &str) -> String {
    // Try keywords that precede a name: fn, actor, type, workflow, etc.
    let keywords = [
        "fn ", "actor ", "type ", "workflow ", "activity ",
        "trait ", "agent ", "skill ", "hook ", "layout ",
    ];
    for line in code.lines() {
        let trimmed = line.trim();
        for kw in &keywords {
            if let Some(rest) = trimmed.strip_prefix(kw) {
                // Also check after decorators like "@component fn Name"
                let name: String = rest.chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return name;
                }
            }
            // Check for decorator-prefixed: "@component fn Name"
            if trimmed.starts_with('@') {
                if let Some(idx) = trimmed.find(kw) {
                    let rest = &trimmed[idx + kw.len()..];
                    let name: String = rest.chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        return name;
                    }
                }
            }
        }
    }
    "example".to_string()
}

// ── System prompt generation ─────────────────────────────────────────────

/// Construct reference docs for system prompt generation.
pub const CONSTRUCT_DOCS: &[(&str, &str)] = &[
    ("action", "`@action fn name() to Type:` — server-side logic calling queries/mutations"),
    ("activity", "`activity name() to Result[Type]:` — retryable side-effectful operation"),
    ("actor", "`actor Name:` with `state` and `on handler()` — message-passing concurrency"),
    ("agent_def", "`@agent_def fn Name() to Type:` — AI agent with memory and tool access"),
    ("component", "`@component fn Name() to Element:` — React-like UI component returning JSX"),
    ("config", "`config:` — configuration block"),
    ("const", "`const name: type = value` — compile-time constant"),
    ("fixture", "`@fixture fn name()` — test fixture"),
    ("function", "`fn name(param: type) to ReturnType:` — standard function"),
    ("hook", "`@hook fn name()` — lifecycle hook"),
    ("http_route", "`http get \"/path\" to Type:` — HTTP endpoint"),
    ("import", "`import module.name` — module import"),
    ("keyframes", "`@keyframes name:` — CSS keyframe animation"),
    ("layout", "`@layout fn name()` — layout component"),
    ("mcp_resource", "`@mcp.resource(\"uri\", \"desc\") fn name() to Type:` — MCP read-only resource"),
    ("mcp_tool", "`@mcp.tool(\"name\", \"desc\") fn name() to Type:` — MCP tool for AI assistants"),
    ("message", "`message Name:` — typed message declaration"),
    ("mock", "`@mock fn name()` — mock function for testing"),
    ("mutation", "`@mutation fn name() to Type:` — database write operation"),
    ("page", "`@page fn name()` — page declaration"),
    ("provider", "`@provider fn name()` — context provider"),
    ("query", "`@query fn name() to Type:` — read-only database query"),
    ("routes", "`routes: \"/\" to Component` — client-side routing"),
    ("scheduled", "`@scheduled fn name()` — scheduled/cron function"),
    ("server_fn", "`@server fn name() to Type:` — generates API route + typed client wrapper"),
    ("skill", "`@skill fn Name() to Type:` — reusable publishable skill"),
    ("table", "`@table type Name:` — database table with typed fields"),
    ("test", "`@test fn name() to Unit:` — unit test with `assert()` for validation"),
    ("theme", "`theme:` — theme definition"),
    ("trait", "`trait Name:` — trait definition for polymorphism"),
    ("type", "`type Name = | Variant(field: type)` — tagged union / ADT"),
    ("workflow", "`workflow name() to Result[Type]:` — durable multi-step orchestration"),
];

/// The hand-maintained philosophy preamble for the system prompt.
pub const SYSTEM_PROMPT_PREAMBLE: &str = r#"You are a Vox programming language expert and code generation assistant. Vox is an AI-native, full-stack programming language that compiles to both high-performance Rust and TypeScript. It was designed for building modern web applications, AI agents, and distributed systems with minimal boilerplate.

## Language Philosophy
- **Compression over ceremony**: Express complex ideas in fewer lines than Rust or TypeScript
- **Full-stack in one file**: Define types, backend logic, UI components, and routing together
- **Durable by default**: Workflows and activities survive process crashes
- **AI-native**: First-class support for agents, MCP tools, and skills"#;

/// Generate the full system prompt string.
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

const CORE_SYNTAX: &str = r#"## Core Syntax

### Variables and Control Flow
- `let x = expr` — immutable binding
- `ret expr` — return value
- `if condition: body`
- `for item in collection: body`
- `match expr: Variant(field) -> body`

### Comments and Imports
- `# single line comment` or `// single line comment`
- `import module.name` — import external dependency

## Actors (Message-Passing Concurrency)
```
actor Counter:
    state count: int = 0
    on increment() to int:
        count = count + 1
        count
    on reset() to Unit:
        count = 0
```
- `spawn(ActorName)` — creates a new actor instance
- `handle.send(method(args))` — sends a message to the actor

## Workflows & Activities (Durable Execution)
```
activity name(param: type) to Result[Type]:
    ret Ok(value)
workflow name(param: type) to Result[Type]:
    let result = activity_call(args) with { retries: N, timeout: "Ns" }
    ret Ok(result)
```

## Components (JSX Syntax)
```
@component fn Name() to Element:
    let (state, set_state) = use_state(initial_value)
    <div class="container">
        <h1>"Title"</h1>
        <input bind={state} placeholder="..." />
        for item in items:
            <div class="item">{item.text}</div>
    </div>
```

## Best Practices
1. Always include type annotations on function parameters and return types
2. Use 4-space indentation consistently
3. Use `Result[T]` for operations that can fail
4. Use `with { retries: N, timeout: "Ns" }` for activities in workflows
5. Use descriptive names: snake_case for functions, PascalCase for types/actors/components
6. Prefer tagged unions over nullable types
"#;

// ── All known construct tags (for coverage reporting) ────────────────────

/// The full taxonomy of construct types for coverage reporting.
pub const TAXONOMY: &[&str] = &[
    "action", "activity", "actor", "agent", "agent_def", "collection",
    "component", "config", "const", "context", "environment",
    "error_boundary", "fixture", "function", "hook", "http_route",
    "import", "impl", "index", "keyframes", "layout", "loading",
    "mcp_resource", "mcp_tool", "message", "mock", "mutation",
    "not_found", "page", "provider", "py_import", "query", "routes",
    "scheduled", "search_index", "server_fn", "skill", "table", "test",
    "theme", "trait", "type", "v0_component", "vector_index", "workflow",
];

// ── Curriculum difficulty scoring ────────────────────────────────────────

/// Return a difficulty score (0-10) for a construct category.
/// Used for curriculum learning: sort training pairs simple→complex.
pub fn construct_difficulty(construct: &str) -> u8 {
    match construct {
        // Tier 0: basic building blocks
        "const" | "import" | "py_import" => 1,
        // Tier 1: simple declarations
        "type" | "function" | "test" | "fixture" | "mock" => 2,
        // Tier 2: UI and data
        "component" | "v0_component" | "table" | "index" | "page" => 3,
        "hook" | "provider" | "context" | "layout" | "loading" | "not_found" => 3,
        // Tier 3: medium complexity
        "query" | "mutation" | "action" | "server_fn" | "http_route" => 4,
        "routes" | "config" | "message" | "scheduled" | "keyframes" => 4,
        "theme" | "environment" | "error_boundary" => 4,
        // Tier 4: concurrency and data
        "actor" | "collection" | "vector_index" | "search_index" => 5,
        "trait" | "impl" => 5,
        // Tier 5: advanced patterns
        "activity" | "workflow" => 7,
        "agent" | "agent_def" | "skill" => 8,
        "mcp_tool" | "mcp_resource" => 8,
        // Documentation-sourced
        "documentation" => 3,
        _ => 5,
    }
}

// ── Negative example generation ──────────────────────────────────────────

/// Mutation strategies for generating negative (broken code) examples.
/// Each returns a (broken_code, error_description) pair.
pub fn generate_negative_examples(code: &str) -> Vec<(String, String)> {
    let mut negatives = Vec::new();

    // Strategy 1: Remove a closing bracket/paren
    if let Some(idx) = code.rfind('}') {
        let mut broken = code.to_string();
        broken.remove(idx);
        negatives.push((broken, "Missing closing brace".to_string()));
    } else if let Some(idx) = code.rfind(')') {
        let mut broken = code.to_string();
        broken.remove(idx);
        negatives.push((broken, "Missing closing parenthesis".to_string()));
    }

    // Strategy 2: Swap 'fn' with 'fun' (invalid keyword)
    if code.contains("fn ") {
        let broken = code.replacen("fn ", "fun ", 1);
        negatives.push((broken, "Invalid keyword 'fun' (should be 'fn')".to_string()));
    }

    // Strategy 3: Remove type annotation
    for line in code.lines() {
        let trimmed = line.trim();
        if let Some(colon_idx) = trimmed.find(") to ") {
            let broken = code.replacen(
                &trimmed[colon_idx..],
                "):",
                1,
            );
            // Make sure we actually changed something
            if broken != code {
                negatives.push((broken, "Missing return type annotation".to_string()));
                break;
            }
        }
    }

    // Strategy 4: Mangle an identifier
    if code.contains("let ") {
        let broken = code.replacen("let ", "lett ", 1);
        negatives.push((broken, "Misspelled keyword 'lett' (should be 'let')".to_string()));
    }

    negatives
}

// ── Multi-turn conversation templates ────────────────────────────────────

/// Return follow-up instruction templates for a given construct.
/// These are used after a base instruction pair to simulate multi-turn refinement.
/// `{name}` is replaced with the extracted identifier.
pub fn followup_templates(construct: &str) -> &[&str] {
    match construct {
        "function" => &[
            "Now add error handling that returns a Result type",
            "Add a docstring explaining what the {name} function does",
            "Refactor {name} to accept its parameters as a struct",
        ],
        "component" => &[
            "Add a loading state and error boundary to {name}",
            "Extract the {name} component props into a typed struct",
            "Add responsive CSS classes to the {name} component",
        ],
        "actor" => &[
            "Add history tracking to the {name} actor so it records all state changes",
            "Add a reset() handler to {name} that restores the initial state",
            "Add a subscription mechanism to notify observers of {name} state changes",
        ],
        "workflow" => &[
            "Add compensating transactions to the {name} workflow for rollback on failure",
            "Add exponential backoff to the retry policy in {name}",
            "Extract the activities in {name} into separate named functions",
        ],
        "activity" => &[
            "Add idempotency checking to {name} so it can safely be retried",
            "Add structured logging to {name} using Vox's tracing primitives",
        ],
        "table" => &[
            "Add an index on the most frequently queried field in {name}",
            "Add created_at and updated_at timestamp fields to {name}",
        ],
        "query" => &[
            "Add pagination (limit and offset) parameters to {name}",
            "Add filtering by status field to {name}",
        ],
        "test" => &[
            "Add edge case tests for null/empty inputs to {name}",
            "Add property-based tests to verify {name} handles all input ranges",
        ],
        "mcp_tool" => &[
            "Add input validation and descriptive error messages to {name}",
            "Add rate limiting to {name} to prevent abuse",
        ],
        _ => &[
            "Add error handling to the previous code",
            "Add a comment explaining the purpose of the previous code",
        ],
    }
}

/// Build multi-turn conversation pairs from a base (instruction, code) pair.
/// Returns a Vec of (follow_up_prompt, refined_code) where refined_code is the same
/// base code since we don't have a compiler that can apply the follow-up automatically —
/// the model should learn the *pattern* of refinement instructions.
pub fn generate_multiturn_pairs(
    construct: &str,
    name: &str,
    base_instruction: &str,
    code: &str,
    schema_version: &str,
    source: &str,
) -> Vec<serde_json::Value> {
    let mut pairs = Vec::new();
    let templates = followup_templates(construct);
    for template in templates.iter().take(2) {
        let follow_up = template.replace("{name}", name);
        // Multi-turn format: include the previous exchange as context in the prompt
        let prompt = format!(
            "Previous instruction: {base_instruction}\nPrevious code:\n```vox\n{code}\n```\n\nFollow-up: {follow_up}"
        );
        pairs.push(serde_json::json!({
            "prompt": prompt,
            "response": code,
            "instruction": follow_up,
            "output": code,
            "category": construct,
            "source": source,
            "rating": 4,
            "turn": 2,
            "schema_version": schema_version,
        }));
    }
    pairs
}
