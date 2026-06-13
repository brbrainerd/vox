# Semantic Behavior Map — `vox-container-types`

Deterministically synthesized from 43 distinct proven-behavior claims (of 43 extracted) across 12 symbols. 5 symbols have an explicit error-path proof; **4 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `parse_raw`  (edge, happy; EXTRACTED)
- [happy] parse_raw produces exactly one empty Arg when parsing empty double-quoted strings  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw produces exactly one empty Arg when parsing empty single-quoted strings  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw extracts the command name from a raw input string  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw populates the args vector with positional arguments  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw extracts flag names from long-form flags  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw correctly sets flag value to None when flag has no value  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw extracts flag values for short flags with following arguments  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw parses long flags with equals syntax and extracts the value  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw extracts the target of a stdout redirection  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw extracts the target command of a pipe redirect  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw preserves whitespace within quoted arguments  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [edge] parse_raw stops parsing flags after encountering -- separator  (crates/vox-container-types/src/exec_grammar/ast.rs)
- … +5 more claims

### `parse_pipeline_raw`  (error, happy, invariant; EXTRACTED)
- [happy] parse_pipeline_raw splits pipe-separated commands into exactly two segments with correct command names  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_pipeline_raw parses three-segment pipelines with correct command names in order  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [invariant] parse_pipeline_raw does not split pipeline segments on pipes that appear inside quoted strings  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_pipeline_raw treats single commands without separators as exactly one segment  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [error] parse_pipeline_raw returns ParseError::UnmatchedQuote when given input with unclosed quotes  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_pipeline_raw splits semicolon-separated commands into exactly two segments with correct command names  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_pipeline_raw splits mixed pipe and semicolon separators into correct number of segments with correct commands  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [invariant] parse_pipeline_raw does not split on semicolons that appear inside quoted strings  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_pipeline_raw splits && separator into exactly two segments with correct command names  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_pipeline_raw splits || separator into exactly two segments with correct command names  (crates/vox-container-types/src/exec_grammar/ast.rs)

### `ExecPolicy::evaluate()`  (happy; EXTRACTED)
- [happy] evaluate() returns exactly one violation when command is not in allowed list  (crates/vox-container-types/src/exec_grammar/policy.rs)
- [happy] evaluate() returns exactly one violation when blocked parameter is used  (crates/vox-container-types/src/exec_grammar/policy.rs)
- [happy] evaluate() returns empty violations list for permissive default policy  (crates/vox-container-types/src/exec_grammar/policy.rs)
- [happy] evaluate() returns no violations when command is in allowed_binaries  (crates/vox-container-types/src/exec_grammar/policy.rs)

### `ParseError`  (error; EXTRACTED)
- [error] parse_raw returns ParseError::UnmatchedQuote when an open quote is not closed  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [error] parse_raw returns ParseError::Empty when input contains only whitespace  (crates/vox-container-types/src/exec_grammar/ast.rs)

### `RedirectKind`  (happy; EXTRACTED)
- [happy] parse_raw correctly identifies stdout redirection as RedirectKind::Stdout  (crates/vox-container-types/src/exec_grammar/ast.rs)
- [happy] parse_raw correctly identifies pipe as RedirectKind::Pipe  (crates/vox-container-types/src/exec_grammar/ast.rs)

### `RiskLevel::Elevated`  (edge; EXTRACTED)
- [edge] classify() assigns RiskLevel::Elevated for commands with recursive flags like -Recurse  (crates/vox-container-types/src/exec_grammar/risk.rs)
- [edge] classify() assigns RiskLevel::Elevated for network commands like curl to external URLs  (crates/vox-container-types/src/exec_grammar/risk.rs)

### `ExecPolicy::default()`  (invariant; EXTRACTED)
- [invariant] Default ExecPolicy with no restrictions allows all commands  (crates/vox-container-types/src/exec_grammar/policy.rs)

### `RiskLevel::Blocked`  (error; EXTRACTED)
- [error] classify() assigns RiskLevel::Blocked when command is not in allowed list  (crates/vox-container-types/src/exec_grammar/risk.rs)

### `RiskLevel::Safe`  (happy; EXTRACTED)
- [happy] classify() assigns RiskLevel::Safe to allowed commands with safe flags  (crates/vox-container-types/src/exec_grammar/risk.rs)

### `ViolationKind::BlockedParameter`  (error; EXTRACTED)
- [error] ExecPolicy.evaluate() detects when parameters match blocked_parameters map  (crates/vox-container-types/src/exec_grammar/policy.rs)

### `ViolationKind::UnknownCommand`  (error; EXTRACTED)
- [error] ExecPolicy.evaluate() detects unknown commands not in allowed_binaries list  (crates/vox-container-types/src/exec_grammar/policy.rs)

### `classify()`  (happy; EXTRACTED)
- [happy] classify() modifies ast.risk field to Safe for permitted commands  (crates/vox-container-types/src/exec_grammar/risk.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ExecPolicy::evaluate()`** — only: _evaluate() returns exactly one violation when command is not in allowed list_
- **`RedirectKind`** — only: _parse_raw correctly identifies stdout redirection as RedirectKind::Stdout_
- **`RiskLevel::Safe`** — only: _classify() assigns RiskLevel::Safe to allowed commands with safe flags_
- **`classify()`** — only: _classify() modifies ast.risk field to Safe for permitted commands_
