# Stale artifact notice (2026-06-10)

The `rollup/` and `by-crate/` JSON/Markdown under this directory may reference **retired crate names** (e.g. `vox-ars-runtime`, `vox-clavis`, `vox-mcp-meta`) from pre-2026-05 workspace reorg snapshots.

Do **not** use these reports for contraction or orphan decisions. Regenerate with `vox ci scaling-audit` (or the current SSOT command in `docs/src/reference/cli.md`) before trusting crate-level counts.
