# Reference: CLI Command Index

The `vox` CLI is the primary entry point for the Vox toolchain. This index documents every subcommand and its most common flags.

## 📦 Project Lifecycle

### `vox init`
Scaffold a new project in the current directory.
- `--kind`: `application` (default), `skill`, `agent`, or `workflow`.
- `name`: Optional project name.

### `vox new <name>`
Create a new directory and scaffold a project inside it.
- `--kind`: project type.

### `vox build <file>`
Compile a `.vox` file.
- `-o`, `--out-dir`: Destination for generated code (default: `dist`).
- `--watch`: Auto-rebuild on save.

### `vox run <file>`
Start the backend for a Vox application.

### `vox dev <file>`
Build, run, and watch for hot-reloading. The recommended way to develop.
- `--port`: Dev server port.

## 🧪 Testing & Quality

### `vox test <path>`
Run automated tests in the specified directory.
- `--filter`: Run only tests matching a name pattern.

### `vox check <file>`
Type-check without generating code.
- `--strict`: Enable TOESTUB architectural enforcement.
- `--autofix`: Suggest fixes for type errors.

### `vox fmt <file>`
Format code according to Vox style standards.
- `--check`: Dry-run mode for CI.

## 🤖 AI & Agents

### `vox mcp run`
Launch the MCP server for agent integration.

### `vox review`
Perform an AI-powered code review.
- `--model`: Model override (OpenRouter/OpenAI/Gemini).
- `--diff`: Only review changed lines.
- `--ci`: Exit with error on findings.

### `vox generate "<prompt>"`
Generate Vox code from natural language using the fine-tuned QWEN model.
- `--output`: File path to save code.

## 🔧 Infrastructure

### `vox db`
Manage the local VoxDB (SQLite).
- `vox db migrate`: Run pending migrations.
- `vox db studio`: Launch the visual database explorer.

### `vox doctor`
Diagnose your development environment and check for missing dependencies.
