# Crate API: vox-toestub

## Overview

**T**odo, **O**mitted wiring, **E**mpty bodies, **S**tub functions, **T**oo-early victory, **U**nresolved references, **B**roken DRY — detector.

TOESTUB mechanically detects AI coding anti-patterns that are banned by `AGENTS.md` but otherwise only caught during manual review.

## Key Modules

| Module | Purpose |
|--------|---------|
| `scanner.rs` | File system scanner — discovers source files |
| `rules.rs` | `DetectionRule` definitions and `Finding` types |
| `detectors/` | Individual detection implementations |
| `engine.rs` | `ToestubEngine` — orchestrates scan + detection |
| `report.rs` | Output formatting (terminal, JSON, Markdown) |
| `ai_analyze.rs` | Optional AI-powered analysis for complex patterns |
| `task_queue.rs` | Parallel task processing |

## What It Detects

| Anti-Pattern | Example |
|-------------|---------|
| `todo!()` / `unimplemented!()` | Stub implementations left in place |
| Empty function bodies | `fn handle() {}` |
| Hardcoded values | Magic numbers, hardcoded URLs |
| DRY violations | Duplicated code blocks |
| Unwrap in production | `.unwrap()` outside tests |
| Stale comments | Comments that don't match code |

## CLI

```bash
vox stub-check                          # Scan current directory
vox stub-check --path src/              # Scan specific path
vox stub-check --format json            # JSON output
vox stub-check --format markdown        # Markdown report
vox stub-check --ai-provider ollama     # Enable AI analysis
```

## Severity Levels

| Level | Meaning |
|-------|---------|
| `Critical` | Must fix before merge |
| `Warning` | Should fix, may indicate incomplete work |
| `Info` | Informational, style improvement |

---

## Module: `vox-toestub\src\ai_analyze.rs`

Optional AI-powered analysis layer.

TOESTUB can optionally use an AI model to perform deeper semantic analysis
beyond what static regex/AST patterns can catch. This module supports:

1. **Ollama (local)** — Zero auth, fully redistributable, runs on user's machine
2. **Gemini Flash (free tier)** — Requires a free API key (no credit card)
3. **OpenRouter free models** — Aggregator with some free models

The AI layer is **entirely optional** — TOESTUB works fully offline with
just the static detectors. AI analysis enhances detection for subtle patterns
that regexes miss: semantic dead code, inconsistent naming, logic gaps, etc.


### `enum AiProvider`

Which AI backend to use for enhanced analysis.


### `struct AiAnalyzer`

Performs AI-enhanced analysis on source files.

This is intentionally synchronous and blocking for simplicity —
AI analysis is opt-in and expected to be slower than static detection.


### `struct DeprecatedUsageDetector`

Detects the presence of `@deprecated` annotations in Vox files.

Reminds the developer to remove obsolete code.


### `struct DryViolationDetector`

Detects near-duplicate code blocks across a single file.

Uses the `similar` crate to compute text similarity between function bodies.
Cross-file DRY detection is a Phase 2 feature (requires the engine to pass
multiple files to a single rule invocation).


### `struct EmptyBodyDetector`

Detects functions with empty or trivially-defaulted bodies.


### `struct GodObjectDetector`

Detects "God Objects" — files or entities that are too large or have too many responsibilities.


### `struct MagicValueDetector`

Detects hardcoded magic values: ports, IPs, filesystem paths, connection strings.

Enforces AGENTS.md line 138:
> "No magic values: Never hardcode ports, database paths, or file system paths."


## Module: `vox-toestub\src\detectors\mod.rs`

Registry of all built-in detection rules.


### `fn all_rules`

Returns all built-in detectors.


### `fn rule_count`

Returns the number of built-in rules.


### `struct SchemaComplianceDetector`

Verifies that files are in locations authorized by vox-schema.json.


### `struct SecretDetector`

Detects hardcoded secrets, API keys, and credentials.


### `struct SprawlDetector`

Detects "Sprawl" — unorganized directory structures, excessive file counts, or forbidden generic names.


### `struct StubDetector`

Detects `todo!()`, `unimplemented!()`, `panic!("not implemented")`,
Python `pass` / `raise NotImplementedError`, GDScript `pass`.


### `struct UnresolvedRefDetector`

Detects references to symbols (functions, types, modules) that appear to
be undefined within the file's scope.

Phase 1: Simple heuristic — looks for `use` imports pointing at unknown
crate-internal modules and function calls that don't match any `fn` definition
in the same file. Full cross-crate resolution is a Phase 2 feature.


### `struct UnwiredModuleDetector`

Detects modules/files that are declared but never imported or referenced.

Catches the classic AI pattern: create a helper module, forget to wire it in.


### `struct VictoryClaimDetector`

Detects suspicious "victory claim" comments near stub or incomplete code.

AI agents love to say "Done!", "Complete!", "All set!" in comments right next
to code that is clearly not finished. This detector finds those patterns.


### `struct ToestubConfig`

Configuration for a TOESTUB analysis run.


### `struct ToestubEngine`

The main analysis engine.


### `struct AnalysisResult`

The output of a TOESTUB analysis run.


## Module: `vox-toestub\src\lib.rs`

# vox-toestub

**T**odo, **O**mitted wiring, **E**mpty bodies, **S**tub functions,
**T**oo-early victory, **U**nresolved references, **B**roken DRY — detector.

TOESTUB mechanically detects AI coding anti-patterns that are banned by
AGENTS.md but otherwise only caught during manual review.


### `enum OutputFormat`

Output format for the report.


### `struct Reporter`

Generates formatted output from findings.


## Module: `vox-toestub\src\review.rs`

AI-powered code review layer — `vox review`.

This module provides CodeRabbit-equivalent code review capabilities built
natively into Vox, using OpenRouter / OpenAI-compatible protocols with a
free-tier fallback chain that follows the same patterns established in
`vox-gamify`'s `FreeAiClient`.

## Provider cascade (highest to lowest preference)
1. **OpenRouter** (`OPENROUTER_API_KEY`) — access to Claude, GPT-4o, Gemini, and free models
2. **OpenAI-compatible** (`OPENAI_API_KEY` or custom `OPENAI_BASE_URL`) — gpt-4o-mini default
3. **Gemini Flash** (`GEMINI_API_KEY`) — free tier, no credit card
4. **Ollama** (local, auto-probed) — zero auth, zero cost
5. **Pollinations.ai** (always available) — no auth, limited quality

## Review categories
Logic errors · Security vulnerabilities · Error handling · Dead code ·
Performance · Naming/style · Vox-specific rules (null safety, scope discipline, etc.)


### `enum ReviewProvider`

AI provider for code review — superset of `AiProvider`, with OpenRouter
and OpenAI-compatible endpoints added.


### `fn auto_discover_providers`

Build the provider cascade from environment variables and local probing.
Returns providers in priority order.


### `struct ReviewConfig`

Configuration for a single review run.


### `enum ReviewOutputFormat`

Output format for review results.


### `struct ReviewResult`

The output of a review run.


### `struct ReviewFinding`

A single issue identified during review.


### `enum ReviewCategory`

Issue category, mirrors CodeRabbit's classification taxonomy.


### `struct ReviewClient`

Performs AI-powered code review using the configured provider cascade.


### `fn build_review_prompt`

Build the full review prompt, capped at `max_tokens` chars of source code.


### `fn build_diff_review_prompt`

Build a prompt focused on a git diff hunk — only the changed lines are reviewed.


### `fn parse_review_response`

Parse the structured `ISSUE|...` response format into findings.


### `fn format_terminal`

Format review findings for terminal output with icons per severity.


### `fn format_sarif`

Format as SARIF 2.1.0 JSON (compatible with GitHub Code Scanning).


### `fn format_markdown`

Format as Markdown for PR comment or file output.


### `enum Severity`

Severity of a finding.


### `enum Language`

Language that a file belongs to.


### `struct SourceFile`

A loaded source file ready for analysis.


### `struct Finding`

A single detected issue.


### `trait DetectionRule`

Every detector implements this trait.


### `struct Scanner`

File-system scanner that walks directories and loads source files.


### `enum Priority`

Priority for a fix suggestion.


### `struct FixSuggestion`

A suggested fix action with a prompt suitable for sending to an AI assistant.


### `struct TaskQueue`

Queue of remaining work items derived from TOESTUB findings.

Designed to integrate with task tracking systems:
- Generates markdown checklists for task.md
- Creates self-contained AI prompts for follow-up sessions
- Tracks progress across sessions via JSON state


