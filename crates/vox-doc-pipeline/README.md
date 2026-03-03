# vox-doc-pipeline

**Workspace-level Rust API doc extractor** — parses all Vox crate source files
using `syn` and emits a unified mdBook with coverage gaps.

> **Not** the same as `vox doc` (which documents a single `.vox` source file).
> This tool scans the *Vox toolchain's own Rust source* to find undocumented APIs.

## Usage

```sh
cargo run -p vox-doc-pipeline -- --out docs/generated
```

## When to run

Run during CI to detect documentation regressions across the toolchain's public API.

Documentation generation tool for the Vox project. Scans `docs/src/` for Markdown files and generates a `SUMMARY.md` navigation index.

## Usage

```bash
cargo run -p vox-doc-pipeline
```

This scans `docs/src/` for all `.md` files (excluding `SUMMARY.md` itself), sorts them alphabetically, and generates a `SUMMARY.md` with title-cased links.

## How It Works

1. Reads all `.md` files in `docs/src/`
2. Converts filenames to title-case headings (e.g., `language-guide.md` → `Language guide`)
3. Writes a `# Summary` with links to each page

## Future Plans

- Copy crate `README.md` files into `docs/src/api/` as individual pages
- Generate cross-reference links between docs and rustdoc
- Integrate with mdBook or Pagefind for full-text search
