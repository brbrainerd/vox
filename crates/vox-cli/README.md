# vox-cli

The command-line interface for the Vox programming language. Entry point for all `vox` commands.

## Commands

Command discoverability is dynamic and clap-derived:

```bash
vox commands --recommended
vox commands --format json --include-nested
```

Why this matters:
- the list comes from the compiled command tree, so docs and editor integrations can avoid drift;
- first-time users get a curated starter set (`--recommended`) without hiding advanced subcommands.

Canonical reference: [`docs/src/reference/cli.md`](../../docs/src/reference/cli.md).

## Key Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | CLI argument parsing (clap) and command dispatch |
| `src/main.rs` | Binary entrypoint → `vox_cli::run_vox_cli()` |
| `commands/` | One module per subcommand |
| `templates.rs` | Project scaffolding templates for `vox init` |
| `v0.rs` | v0.dev AI component generation integration |

## Usage

```bash
cargo install --locked --path crates/vox-cli
```

Prebuilt binaries, Windows, prerequisites, and the optional subsystems:
[Installing Vox](../../docs/src/reference/installation.md) — the canonical page.
