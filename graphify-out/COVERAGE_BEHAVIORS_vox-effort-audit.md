# Semantic Behavior Map — `vox-effort-audit`

Deterministically synthesized from 43 distinct proven-behavior claims (of 43 extracted) across 20 symbols. 2 symbols have an explicit error-path proof; **15 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `resolve_for_commit()`  (edge, happy; EXTRACTED)
- [happy] Returns MeasuredCost::Measured with correct input_tokens, output_tokens, and session_id when a matching session exists within the time window  (crates/vox-effort-audit/src/hybrid/transcripts.rs)
- [edge] Returns MeasuredCost::Unavailable when no session timestamp falls within the specified time window  (crates/vox-effort-audit/src/hybrid/transcripts.rs)
- [edge] Returns MeasuredCost::Unavailable when working directory in transcript does not match the provided repo_root  (crates/vox-effort-audit/src/hybrid/transcripts.rs)
- [edge] Returns MeasuredCost::Unavailable when the transcript directory does not exist  (crates/vox-effort-audit/src/hybrid/transcripts.rs)
- [edge] Successfully matches Windows native paths (C:\...) with Git-Bash style paths (/c/...) in transcripts and returns Measured  (crates/vox-effort-audit/src/hybrid/transcripts.rs)

### `EffortAuditConfig`  (happy; EXTRACTED)
- [happy] Default configuration has default_since="30 days ago", max_concurrent=4, max_diff_bytes=200KB, with_transcripts=true, report_top_n=20  (crates/vox-effort-audit/src/config.rs)
- [happy] Default configuration has limit field set to None  (crates/vox-effort-audit/src/config.rs)
- [happy] Configuration limit field deserializes from TOML string and equals the parsed value  (crates/vox-effort-audit/src/config.rs)
- [happy] Partial TOML configuration inherits unspecified fields from defaults (default_since and max_concurrent)  (crates/vox-effort-audit/src/config.rs)

### `parse_duration()`  (error, happy; EXTRACTED)
- [happy] parse_duration parses both "30d" and "30 days ago" to equivalent Duration::days(30)  (crates/vox-effort-audit/src/range.rs)
- [error] parse_duration rejects branch names (feature-2d) and short SHA-like refs (abc123d, abc123h, abc123w) that end in duration suffix letters  (crates/vox-effort-audit/src/range.rs)
- [happy] parse_duration() correctly parses both short ("30d") and long ("30 days ago") duration suffix forms to Duration::days(30)  (crates/vox-effort-audit/src/range.rs)
- [error] parse_duration() rejects branch names and SHA-like refs that end with duration suffix letters (d/h/w)  (crates/vox-effort-audit/src/range.rs)

### `CommitRange`  (happy; EXTRACTED)
- [happy] CommitRange::resolve with no explicit range arguments returns SinceDuration variant  (crates/vox-effort-audit/src/range.rs)
- [happy] CommitRange::resolve with explicit since ref and head returns Refs variant with since field set  (crates/vox-effort-audit/src/range.rs)
- [happy] CommitRange::resolve treats HEAD~30 syntax as a ref, returning Refs variant not SinceDuration  (crates/vox-effort-audit/src/range.rs)

### `MockJudge`  (happy; EXTRACTED)
- [happy] Routes high sweep scores (8) to ScriptAutomation remediation and returns JudgeStatus::Judged  (crates/vox-effort-audit/src/judge/mod.rs)
- [happy] Routes doc-only commits to NoneNeeded remediation with LegitDocs waste category  (crates/vox-effort-audit/src/judge/mod.rs)
- [happy] Routes default cases (mid-range score, low mechanical sweep, not doc-only) to Unknown remediation  (crates/vox-effort-audit/src/judge/mod.rs)

### `RunSummary`  (happy; EXTRACTED)
- [happy] RunSummary struct clones successfully and clone preserves all field values  (crates/vox-effort-audit/src/pipeline.rs)
- [happy] RunSummary Debug format contains the type name "RunSummary"  (crates/vox-effort-audit/src/pipeline.rs)
- [happy] run() returns summary with accurate commit counts matching range query (commits_judged=commits_in_range=5, commits_skipped=0)  (crates/vox-effort-audit/tests/e2e_smoke.rs)

### `parse()`  (error, happy; EXTRACTED)
- [happy] Successfully parses minimal JSON judge findings and extracts waste_score field  (crates/vox-effort-audit/src/judge/parse.rs)
- [happy] Successfully parses JSON judge findings wrapped in markdown code fences (```json ... ```)  (crates/vox-effort-audit/src/judge/parse.rs)
- [error] Rejects waste_score values above 10 with ParseError::Schema variant  (crates/vox-effort-audit/src/judge/parse.rs)

### `render()`  (happy; EXTRACTED)
- [happy] render() does not emit email symbols (@) in markdown output  (crates/vox-effort-audit/src/output/markdown.rs)
- [happy] render() does not emit the word 'author' in markdown output  (crates/vox-effort-audit/src/output/markdown.rs)
- [happy] render() does not leak author email hash prefixes in markdown output  (crates/vox-effort-audit/src/output/markdown.rs)

### `CommitRange::Refs`  (happy; EXTRACTED)
- [happy] resolve() returns CommitRange::Refs variant with correct since value when since is a SHA/branch  (crates/vox-effort-audit/src/range.rs)
- [happy] resolve() treats git-native refs like HEAD~30 as CommitRange::Refs, not as duration  (crates/vox-effort-audit/src/range.rs)

### `Judge`  (happy; EXTRACTED)
- [happy] Default Judge config has max_total_tokens=5_000_000 and max_dollar_cost≈5.00  (crates/vox-effort-audit/src/config.rs)
- [happy] Judge nested config field deserializes from partial TOML with model_preference specified  (crates/vox-effort-audit/src/config.rs)

### `iter_commits()`  (happy, invariant; EXTRACTED)
- [happy] iter_commits() returns correct commit count and ensures all commits have non-empty author_email_sha256  (crates/vox-effort-audit/src/walk.rs)
- [invariant] iter_commits() returns commits in newest-first chronological order  (crates/vox-effort-audit/src/walk.rs)

### `CommitRange::SinceDuration`  (happy; EXTRACTED)
- [happy] resolve() returns CommitRange::SinceDuration variant when both since and until arguments are None  (crates/vox-effort-audit/src/range.rs)

### `Manifest.judge_total_cost_usd`  (happy; EXTRACTED)
- [happy] run() calculates judge_total_cost_usd as Some(0.0) when using known rates with zero tokens  (crates/vox-effort-audit/tests/e2e_smoke.rs)

### `features()`  (happy; EXTRACTED)
- [happy] features() function detects commits with only documentation file changes as is_doc_only=true  (crates/vox-effort-audit/src/shape.rs)

### `features().commit_kind_from_message`  (happy; EXTRACTED)
- [happy] features() extracts commit_kind_from_message correctly from conventional commit prefixes (fix/refactor/other)  (crates/vox-effort-audit/src/shape.rs)

### `features().is_doc_only`  (happy; EXTRACTED)
- [happy] features() correctly sets is_doc_only flag to true for commits that only touch documentation files (*.md)  (crates/vox-effort-audit/src/shape.rs)

### `features().mechanical_sweep_score`  (happy; EXTRACTED)
- [happy] mechanical_sweep_score computes to mid-range (0.40-0.50) for diffs with ~50% repeated and ~50% distinct lines  (crates/vox-effort-audit/src/shape.rs)

### `iter_commits().diff_truncated`  (happy; EXTRACTED)
- [happy] iter_commits() sets diff_truncated flag to true when diff size exceeds the byte limit  (crates/vox-effort-audit/src/walk.rs)

### `normalize_path_str()`  (invariant; EXTRACTED)
- [invariant] Normalizes Windows backslash paths and Unix-style paths to identical canonical format, and strips trailing slashes  (crates/vox-effort-audit/src/hybrid/transcripts.rs)

### `vox_effort_audit::run()`  (happy; EXTRACTED)
- [happy] run() creates findings.jsonl, report.md, and manifest.json output files  (crates/vox-effort-audit/tests/e2e_smoke.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CommitRange`** — only: _CommitRange::resolve with no explicit range arguments returns SinceDuration variant_
- **`CommitRange::Refs`** — only: _resolve() returns CommitRange::Refs variant with correct since value when since is a SHA/branch_
- **`CommitRange::SinceDuration`** — only: _resolve() returns CommitRange::SinceDuration variant when both since and until arguments are None_
- **`EffortAuditConfig`** — only: _Default configuration has default_since="30 days ago", max_concurrent=4, max_diff_bytes=200KB, with_transcripts=true, report_top_n=20_
- **`Judge`** — only: _Default Judge config has max_total_tokens=5_000_000 and max_dollar_cost≈5.00_
- **`Manifest.judge_total_cost_usd`** — only: _run() calculates judge_total_cost_usd as Some(0.0) when using known rates with zero tokens_
- **`MockJudge`** — only: _Routes high sweep scores (8) to ScriptAutomation remediation and returns JudgeStatus::Judged_
- **`RunSummary`** — only: _RunSummary struct clones successfully and clone preserves all field values_
- **`features()`** — only: _features() function detects commits with only documentation file changes as is_doc_only=true_
- **`features().commit_kind_from_message`** — only: _features() extracts commit_kind_from_message correctly from conventional commit prefixes (fix/refactor/other)_
- **`features().is_doc_only`** — only: _features() correctly sets is_doc_only flag to true for commits that only touch documentation files (*.md)_
- **`features().mechanical_sweep_score`** — only: _mechanical_sweep_score computes to mid-range (0.40-0.50) for diffs with ~50% repeated and ~50% distinct lines_
- **`iter_commits().diff_truncated`** — only: _iter_commits() sets diff_truncated flag to true when diff size exceeds the byte limit_
- **`render()`** — only: _render() does not emit email symbols (@) in markdown output_
- **`vox_effort_audit::run()`** — only: _run() creates findings.jsonl, report.md, and manifest.json output files_
