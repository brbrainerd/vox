# Semantic Behavior Map — `vox-eval`

One-paragraph summary: This map synthesizes 7 extracted Behavior claims (deduped to 4 distinct symbols: `scope_compliance_score()`, `eval_semantic_entropy()`, `SemanticEntropyReport`, and `summarize_placeholder()`). Of these, only `scope_compliance_score()` carries any error-path proof, and even that covers just 1 of its 7 dangerous patterns. The entropy reporter is proven on monoculture and diversity happy paths but never on its empty-input branch, the `SemanticEntropyReport` struct only ever has its `collapse_warning` flag observed, and `summarize_placeholder()` is a happy-path-only stub. The crate's only security-relevant surface (the scope validator) is the most under-proven contract.

## `scope_compliance_score()` (crates/vox-eval/src/lib.rs:226)
Proven behaviors:
- Returns `1.0` for snippets containing none of the dangerous patterns (`scope_compliance_clean_snippet`).
- Returns `0.0` when the snippet contains `std::process::Command` (`scope_compliance_flags_process_spawn`).

Error path: yes (1 rejection case). Edge/invariant: no.

Note: the function checks a `BAD` list of 7 patterns (`std::process::command`, `std::fs::remove_dir_all`, `../../../etc/passwd`, `child_process`, `rm -rf `, `eval(`, `base64 -d`). Only the first is exercised; case-insensitivity (`to_lowercase`) is incidentally exercised but not asserted as a distinct behavior.

## `eval_semantic_entropy()` (crates/vox-eval/src/lib.rs:498)
Proven behaviors:
- Monoculture: identical samples yield `ast_diversity < 0.4` and `collapse_warning = true` (`entropy_detects_monoculture`).
- Diversity: structurally distinct samples yield `ast_diversity > 0.9` and `collapse_warning = false` (`entropy_detects_diversity`).

Error path: no. Edge/invariant: no.

Note: an empty-`outputs` branch (lib.rs:498-503) short-circuits to `ast_diversity = 0.0, collapse_warning = true` and is never tested. The `collapse_warning` boundary (`ast_diversity < collapse_threshold`, lib.rs:549) is only probed at the extremes, never at the threshold.

## `SemanticEntropyReport` (crates/vox-eval/src/lib.rs:485)
Proven behaviors:
- `collapse_warning = true` under low structural diversity; `collapse_warning = false` under high diversity (both via the two `eval_semantic_entropy` tests).

Error path: n/a (data struct). Edge/invariant: no.

Note: only `collapse_warning` (and indirectly `ast_diversity`) is asserted; other fields of the report are never observed.

## `summarize_placeholder()` (crates/vox-eval/src/mens.rs:14)
Proven behaviors:
- Returns `CompileVerdict::Pass` (`placeholder_ok`).

Error path: no. Edge/invariant: no. (Placeholder stub — no failure contract exists to prove.)

## Semantic gaps
Symbols proven only on the happy path whose contract has an obvious failure/empty/conflict mode:

1. **`scope_compliance_score()` — security validator with 6 of 7 rejection paths unproven (most actionable).** This is the crate's only security surface. The single proven rejection (`std::process::Command`) leaves `remove_dir_all`, the `../../../etc/passwd` path-traversal pattern, `child_process`, `rm -rf `, `eval(`, and `base64 -d` entirely untested. A regression silently dropping any of those from `BAD` would pass CI. Add one rejection assertion per pattern.

2. **`eval_semantic_entropy()` — empty-input branch unproven.** The explicit `if outputs.is_empty()` early return (lib.rs:498-503) is dead from a coverage standpoint. Add a test for `eval_semantic_entropy(&[], _)` asserting `ast_diversity == 0.0` and `collapse_warning == true`. The threshold boundary (diversity exactly equal to `collapse_threshold`) is also worth one edge test.

3. **`SemanticEntropyReport` — fields beyond `collapse_warning` never asserted.** Only the warning flag and diversity bounds are observed; any other reported field is invariant-free.

4. **`summarize_placeholder()` — happy-path-only stub.** Acceptable while it remains a placeholder, but flag it: if it gains real logic it must grow a failure path before it can return anything but `Pass`.