# Semantic Behavior Map — `vox-lsp`

Deterministically synthesized from 32 distinct proven-behavior claims (of 32 extracted) across 6 symbols. 3 symbols have an explicit error-path proof; **2 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `validate_document()`  (edge, error, happy, invariant; EXTRACTED)
- [happy] validate_document() emits a WARNING diagnostic containing 'Mens activity call' when VOX_MESH_ENABLED=0 and mesh_snapshot() is in a workflow  (crates/vox-lsp/src/lib.rs)
- [edge] validate_document() does NOT emit a WARNING diagnostic for mesh activity when VOX_MESH_ENABLED=1  (crates/vox-lsp/src/lib.rs)
- [happy] validate_document() returns empty Vec for empty source string  (crates/vox-lsp/tests/diagnostic_tests.rs)
- [happy] validate_document() produces no ERROR diagnostics for valid let binding syntax  (crates/vox-lsp/tests/diagnostic_tests.rs)
- [happy] validate_document() emits a WARNING diagnostic with 'Mens activity call' when VOX_MESH_ENABLED=0 and mesh_snapshot() is in a workflow body  (crates/vox-lsp/tests/diagnostic_tests.rs)
- [edge] validate_document() does NOT emit a WARNING diagnostic containing 'Mens activity call' when VOX_MESH_ENABLED=1 and mesh_snapshot() is in a workflow body  (crates/vox-lsp/tests/diagnostic_tests.rs)
- [edge] validate_document() does NOT emit a WARNING diagnostic for mesh_snapshot() when it appears in a fn body, even when VOX_MESH_ENABLED=0  (crates/vox-lsp/tests/diagnostic_tests.rs)
- [invariant] validate_document() sets the source field to 'vox-lsp' for all emitted diagnostics  (crates/vox-lsp/tests/diagnostic_tests.rs)
- [error] validate_document() produces at least one ERROR severity diagnostic for malformed syntax (unclosed paren)  (crates/vox-lsp/tests/diagnostic_tests.rs)

### `line_has_speech_transcribe()`  (edge, error, happy; EXTRACTED)
- [happy] Detects plain Speech.transcribe method calls  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] Accepts Speech.transcribe calls with arbitrary whitespace around dot and parens  (crates/vox-lsp/tests/hover_tests.rs)
- [error] Rejects Speech.transcribe calls when receiver has prefix (not bare Speech)  (crates/vox-lsp/tests/hover_tests.rs)
- [error] Rejects standalone transcribe identifiers or calls without Speech receiver  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] Detects Speech.transcribe in indented code  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] line_has_speech_transcribe() detects 'Speech.transcribe' with optional whitespace and returns true  (crates/vox-lsp/src/lib.rs)
- [edge] line_has_speech_transcribe() rejects Speech.transcribe when prefixed by identifier characters (FooSpeech.transcribe)  (crates/vox-lsp/src/lib.rs)
- [edge] line_has_speech_transcribe() rejects standalone transcribe calls not preceded by Speech.  (crates/vox-lsp/src/lib.rs)

### `word_at_position()`  (edge, happy; EXTRACTED)
- [happy] Extracts identifier from start of line (position 0, 0)  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] Correctly extracts identifier from middle of a word  (crates/vox-lsp/tests/hover_tests.rs)
- [edge] Returns None when cursor is positioned on punctuation  (crates/vox-lsp/tests/hover_tests.rs)
- [edge] Returns None when position exceeds line length  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] Extracts identifiers from non-first lines in multiline text  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] Treats underscore as valid identifier character  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] word_at_position() extracts 'Speech' identifier at position (line=1, col=4) from 'Speech.transcribe(p)' on line 1  (crates/vox-lsp/src/lib.rs)

### `builtin_hover_markdown_in_line()`  (error, happy, invariant; EXTRACTED)
- [happy] Returns Some when word 'transcribe' is called with Speech receiver  (crates/vox-lsp/tests/hover_tests.rs)
- [error] Returns None when transcribe is called with non-Speech receiver (e.g., other.transcribe)  (crates/vox-lsp/tests/hover_tests.rs)
- [error] Returns None when transcribe is used as variable name, not method call  (crates/vox-lsp/tests/hover_tests.rs)
- [invariant] Ignores line context for non-transcribe words, returning Some for HTTP despite invalid context  (crates/vox-lsp/tests/hover_tests.rs)
- [happy] builtin_hover_markdown_in_line() returns Some for 'transcribe' only when the line contains Speech.transcribe  (crates/vox-lsp/src/lib.rs)
- [error] builtin_hover_markdown_in_line() returns None for 'transcribe' when the line does not contain Speech.transcribe  (crates/vox-lsp/src/lib.rs)

### `builtin_hover_markdown()`  (happy; EXTRACTED)
- [happy] Returns Some markdown for 'Speech' containing both 'Speech' and 'transcribe' text  (crates/vox-lsp/tests/hover_tests.rs)

### `validate_document_with_hir()`  (happy; EXTRACTED)
- [happy] Returns empty diagnostics vector for empty source code  (crates/vox-lsp/tests/validate_with_hir.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`builtin_hover_markdown()`** — only: _Returns Some markdown for 'Speech' containing both 'Speech' and 'transcribe' text_
- **`validate_document_with_hir()`** — only: _Returns empty diagnostics vector for empty source code_
