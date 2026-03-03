# vox-cli

The command-line interface for the Vox programming language. Entry point for all `vox` commands.

## Commands

| Command | Description |
|---------|-------------|
| `vox build <file>` | Compile a `.vox` file to Rust + TypeScript |
| `vox run <file>` | Build and run a Vox application |
| `vox bundle <file>` | Bundle into a standalone web application |
| `vox test <file>` | Run `@test` decorated functions |
| `vox fmt <file>` | Format Vox source files |
| `vox check <file>` | Type-check without producing output |
| `vox compact <file>` | Compact source for LLM context |
| `vox lsp` | Launch the Language Server |
| `vox doc` | Generate documentation via vox-doc-pipeline |
| `vox init [name]` | Scaffold a new Vox project |
| `vox install <pkg>` | Install a package |
| `vox add/remove` | Add or remove dependencies |
| `vox update` | Update dependencies |
| `vox vendor` | Bundle dependencies for offline builds |
| `vox publish` | Publish a package to the registry |
| `vox search <query>` | Search the package registry |
| `vox audit` | Check dependencies for security advisories |
| `vox clean` | Remove build artifacts |
| `vox stub-check` | Run TOESTUB anti-pattern detection |
| `vox review` | AI-assisted code review |
| `vox share <action>` | Publish/browse shared artifacts |
| `vox snippet <action>` | Save/search code snippets |
| `vox agent <action>` | Register/manage AI agents |
| `vox gamify <action>` | Code companions, quests, battles |
| `vox orchestrator <action>` | Multi-agent task queues |
| `vox dashboard` | Orchestrator HUD web UI |
| `vox learn <action>` | Learning and skill progress |
| `vox agent <action>` | Agent registry operations |

## Key Files

| File | Purpose |
|------|---------|
| `main.rs` | CLI argument parsing (clap) and command dispatch |
| `commands/` | One module per subcommand |
| `templates.rs` | Project scaffolding templates for `vox init` |
| `v0.rs` | v0.dev AI component generation integration |

## Usage

```bash
# Install from source
cargo install --path crates/vox-cli

# Or build for development
cargo build -p vox-cli
```
