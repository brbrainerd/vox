# vox-toestub

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
