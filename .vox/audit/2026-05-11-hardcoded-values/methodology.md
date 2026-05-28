# Hardcoded Values Audit — Methodology

**Run ID**: `2026-05-11-hardcoded-values`  
**Generator**: `tools/hardcoded-values-audit.mjs` (invoke from repo root: `node tools/hardcoded-values-audit.mjs [OUT_DIR]`)

## Scope

- **Globs**: searches use `**/src/**/*.rs` under `crates/` (not `*/src/…`) for consistent **Windows + POSIX** ripgrep behavior.

### Included paths (each search pass adds path-specific globs)

- `crates/**/src/**/*.rs`, `crates/**/build.rs` — production Rust
- `apps/**/src/**/*.ts`, `apps/**/src/**/*.tsx` — app TS (excludes `__tests__`, `*.test.*`, `*.spec.*`)
- `examples/golden/**/*.vox` — canonical Vox corpus
- `contracts/**/*.{yaml,yml,json}` — SSOT drift candidates

### Excluded

- `**/tests/**`, `**/fixtures/**`, `**/benches/**`
- `**/*_test.rs`, `**/*_tests.rs`
- `**/node_modules/**`, `**/patches/**`, `**/*.generated.*`
- `docs/src/archive/**` (tombstoned)

### Line filters (post-pass)

The generator drops lines that are clearly non-actionable:

- Comments (`//`, `#`, `/*`, leading `*`)
- `use ` / `import ` lines
- `const ` / `static ` / `pub const` / `pub static` (values centralized in the binding)
- Whitespace-only

Rust `#[cfg(test)]` blocks are **not** fully elided (would require a parser); excluding `tests/` paths covers most test-only code.

## Category recipes (ripgrep patterns)

| # | `category` | Pattern summary |
|---|------------|-----------------|
| 1 | `hardcoded-urls` | `"https://...` / `"http://...` string literals |
| 2 | `hardcoded-ports` | `localhost:\d+`, `127.0.0.1:\d+`, `[::1]:\d+`, `0.0.0.0:\d+` in quotes |
| 3 | `hardcoded-ips` | Quoted IPv4 literals (127/10/192.168) |
| 4 | `hardcoded-filesystem-paths` | `C:\`, `/home/`, `/tmp/`, `/var/`, `/usr/`, `/etc/`, `~/` |
| 5 | `hardcoded-timeouts` | `Duration::from_*`, `sleep`/`delay` calls with numeric ms/s (heuristic) |
| 6 | `hardcoded-retry-counts` | `for _ in 0..N`, `max_retries`, `retry_n`, etc. |
| 7 | `hardcoded-buffer-sizes` | `with_capacity(N)`, `bounded(N)`, `channel(N)`, byte array sizes |
| 8 | `hardcoded-version-strings` | `/v1/`, `/v2/`, `"v1"`, semver-like `"0.x.y"` in strings |
| 9 | `hardcoded-model-names` | `gpt-`, `claude`, `whisper`, embedding model id patterns |
| 10 | `hardcoded-env-var-names` | `std::env::var("X")` / `env::var("X")` / `option_env!("X")` where `X` ∉ `contracts/config/env-vars.v1.yaml` |
| 11 | `brittle-string-needles` | `.contains("…")`, `.starts_with("…")`, `.ends_with("…")` |
| 12 | `brittle-regex-patterns` | `Regex::new("…")` / `Regex::new(r#"…"#)` (sample for manual review) |
| 13 | `hardcoded-extensions-globs` | `".rs"`, `".ts"`, `".md"`, `"*.toml"` in code strings |
| 14 | `hardcoded-date-literals` | `202[0-9]-`, `from_ymd_opt(20` |
| 15 | `hardcoded-ansi-codes` | `\x1b`, `\u{1b}`, `CSI` ANSI escapes |
| 16 | `hardcoded-magic-numbers` | `\b1024\b`, `\b4096\b`, … and `1_024` / `4_096` underscores (excluding const / diagnostic / shift-sizing lines) |
| 17 | `hardcoded-canonical-keys` | `"oratio"`, `"populi"`, `"vox-orchestrator"` as string literals |
| 18 | `hardcoded-provider-names` | `"openai"`, `"anthropic"`, `"openrouter"`, etc. |
| 19 | `hardcoded-test-data-in-prod` | `test@`, `@example.com`, `@example.org` |
| 20 | `hardcoded-retired-runtime-names` | `vox-dei`, `vox-ars`, `vox-ludus`, `vox-lexer`, `vox-parser`, `vox-hir` |

**Category 20 noise reduction:** the generator skips matches under `contracts/reports/`, plus paths containing `retired-symbols`, `retired-surfaces`, or `scaling-audit/` (SSOT lists and generated audit JSON mention retired names on purpose). CI sources that intentionally reference retired symbols for guards are excluded by basename: `docs_deprecated_command_guard.rs`, `nomenclature_guard.rs`, `retired_symbol_check.rs`.

## Caps and signal ranking

- Each category emits at most **100** itemized findings (`ITEM_CAP` in the script); `category_stats` records `raw_match_count` and whether the cap was applied.
- Raw matches are **sorted by signal score** (highest first) before the cap: findings in **network / IO / timeout** contexts and **long user-facing string needles** rank above log-only or schema-boilerplate hits.
- **`confidence`** on each finding reflects those heuristics (`high` / `medium` / `low`), not only the category default.

### Suppression and boosts (summary)

| Category | Suppress / downgrade | Boost to higher confidence |
|----------|----------------------|----------------------------|
| `brittle-string-needles` | Trivial needles (length ≤ 1, punctuation-only tokens) | Long needles, whitespace / phrase-like text |
| `hardcoded-magic-numbers` | `println!` / `debug!`-only lines; `<<` shift with power-of-two | `timeout`, `buffer`, `capacity`, `read_exact`, … |
| `hardcoded-urls` | Doc trust hosts (`docs.rs`, `semver.org`, …); JSON schema lines | `reqwest`, `Url::parse`, `connect_timeout`, … |
| `hardcoded-timeouts` | Zero `Duration`; log-only lines | `timeout`, `deadline`, `backoff`, `Duration::from_` |
| `hardcoded-ports` / `hardcoded-ips` | (none by default) | `bind`, `listen`, `TcpListener`, `connect` |
| `hardcoded-filesystem-paths` | (none by default) | `File::open`, `include_str!`, `read_to_string` |
| `hardcoded-canonical-keys` / `hardcoded-provider-names` | Any match under `contracts/` (SSOT definitions) | N/A (code duplicates only) |
| `hardcoded-version-strings` | `*.schema.json` paths | — |

## False positives

- **`hardcoded-magic-numbers`**: power-of-two outside shift patterns may still appear in legitimate byte math — use `confidence` and file context.
- **`brittle-string-needles`**: raw-string `.contains(r#"…"#)` forms are not parsed for inner needles; those lines may fall back to line-level evidence only.
- **Contracts YAML** (paths outside `contracts/`): URL-like strings may still be documentation examples.

## Regenerating

```bash
node tools/hardcoded-values-audit.mjs .vox/audit/2026-05-11-hardcoded-values
```

---

## Verification for a follow-up LLM

For each finding `hv-NNNN`:

1. **Shell** (repo root): copy `verification_command` from `findings.v1.json` and run unchanged.
2. If exit code ≠ 0 and rg prints nothing → finding may be stale (line moved); re-run the category recipe from this doc.
3. **Apply** `suggested_fix.replacement_snippet` only after confirming `why_it_matters` applies (especially `confidence: "low"`).
4. **`severity: "error"`** (`retired-runtime-names`, unregistered critical env reads): prioritize.
