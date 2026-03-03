# Crate API: vox-doc-pipeline

## Overview

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

---

## Module: `vox-doc-pipeline\src\main.rs`

`vox-doc-pipeline` - Deep Documentation Extraction Tool

This tool parses the Vox codebase to generate a unified, searchable mdBook.
It uses `syn` for robust Rust parsing and extracts doc comments from all public APIs.


