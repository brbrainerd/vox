# Semantic Behavior Map — `vox-speech`

Deterministically synthesized from 76 distinct proven-behavior claims (of 80 extracted) across 24 symbols. 1 symbols have an explicit error-path proof; **20 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `route_transcript_with_options()`  (edge, error, happy, invariant; EXTRACTED)
- [happy] route_transcript_with_options with Tool mode and 0.9 confidence returns action 'oratio.status'  (crates/vox-speech/src/routing.rs)
- [error] route_transcript_with_options with Tool mode and 0.1 confidence returns status 'below_tool_confidence'  (crates/vox-speech/src/routing.rs)
- [happy] route_transcript_with_options with code-create intent phrase and 0.9 confidence returns action 'speech.intent.code_create'  (crates/vox-speech/src/routing.rs)
- [happy] route_transcript_with_options with code-create intent phrase returns status 'intent_matched'  (crates/vox-speech/src/routing.rs)
- [invariant] route_transcript_with_options response payload contains 'speech_escalation_recommended' field for code-create intents  (crates/vox-speech/src/routing.rs)
- [edge] route_transcript_with_options does not return action 'speech.intent.code_edit' for phrase 'exchange rate helper in the module'  (crates/vox-speech/src/routing.rs)
- [happy] route_transcript_with_options with explicit 'change' keyword phrase returns action 'speech.intent.code_edit'  (crates/vox-speech/src/routing.rs)
- [invariant] route_transcript_with_options enforces max_user_turns guard and returns status 'guard_max_user_turns' after exceeding configured turn limit  (crates/vox-speech/src/routing.rs)
- [happy] route_transcript_with_options with 'edit this function' phrase and active file context returns action 'speech.intent.code_edit'  (crates/vox-speech/src/routing.rs)
- [happy] route_transcript_with_options with contextual 'edit this' phrase returns status 'intent_matched'  (crates/vox-speech/src/routing.rs)
- [happy] route_transcript_with_options with phrase matching recent error returns action 'speech.intent.code_edit'  (crates/vox-speech/src/routing.rs)
- [invariant] route_transcript_with_options when matching error context returns intent_confidence >= 0.85  (crates/vox-speech/src/routing.rs)
- … +9 more claims

### `route_transcript_with_options`  (happy; EXTRACTED)
- [happy] returns action 'oratio.status' for high-confidence tool mode transcripts  (crates/vox-speech/src/routing.rs)
- [happy] returns status 'below_tool_confidence' when confidence is below routing threshold  (crates/vox-speech/src/routing.rs)
- [happy] sets action to 'speech.intent.code_create' when recognizing create function intents  (crates/vox-speech/src/routing.rs)
- [happy] sets status to 'intent_matched' for recognized speech intents  (crates/vox-speech/src/routing.rs)
- [happy] includes speech_escalation_recommended field in payload for code intents  (crates/vox-speech/src/routing.rs)
- [happy] does not route 'exchange rate helper' phrase as code_edit intent  (crates/vox-speech/src/routing.rs)
- [happy] routes explicit 'change' directive as code_edit intent  (crates/vox-speech/src/routing.rs)
- [happy] enforces max user turns guard and returns 'guard_max_user_turns' status after configured threshold  (crates/vox-speech/src/routing.rs)
- [happy] routes contextual 'edit this' phrase as code_edit intent with active file context  (crates/vox-speech/src/routing.rs)
- [happy] sets status to 'intent_matched' for contextual edit intents  (crates/vox-speech/src/routing.rs)
- [happy] routes to code_edit intent when error keywords from recent_errors are mentioned  (crates/vox-speech/src/routing.rs)
- [happy] increases intent_confidence in payload when error context keywords are present  (crates/vox-speech/src/routing.rs)
- … +2 more claims

### `OratioRuntimeConfig::merge_file`  (happy; EXTRACTED)
- [happy] parses TOML file and merges session_timing.capture_timeout_ms value  (crates/vox-speech/src/runtime_config.rs)
- [happy] parses TOML file and merges routing.tool_route_min_confidence value  (crates/vox-speech/src/runtime_config.rs)
- [happy] parses TOML file and merges refine.balanced_base value  (crates/vox-speech/src/runtime_config.rs)
- [happy] parses TOML file and merges hf.retry_attempts value  (crates/vox-speech/src/runtime_config.rs)

### `map_to_ast_target()`  (happy; EXTRACTED)
- [happy] maps natural language 'create a function called hello' to an AST target with node_kind='function' and symbol_name='hello'  (crates/vox-speech/src/ast_mapper.rs)
- [happy] maps function creation intent without cursor context (at_cursor=false)  (crates/vox-speech/src/ast_mapper.rs)
- [happy] maps 'edit this function' with cursor context to a function target with at_cursor=true and no symbol name  (crates/vox-speech/src/ast_mapper.rs)
- [happy] maps 'edit this function' without context to a function target with at_cursor=false  (crates/vox-speech/src/ast_mapper.rs)

### `transcribe_path()`  (happy; EXTRACTED)
- [happy] transcribe_path() preserves raw_text including whitespace and newlines from file  (crates/vox-speech/src/traits.rs)
- [happy] transcribe_path() returns refined_text with trimmed whitespace when available  (crates/vox-speech/src/traits.rs)
- [happy] transcribe_path() display_text() method returns the cleaned refined text without excess whitespace  (crates/vox-speech/src/traits.rs)

### `transcribe_path_session`  (happy; EXTRACTED)
- [happy] normalizes fixture text from 'mends' to 'mens' via transcription  (crates/vox-speech/src/session.rs)
- [happy] returns non-zero confidence value in session result  (crates/vox-speech/src/session.rs)
- [happy] includes deadline diagnostics with OratioDeadlineTaxonomy::Ok in result  (crates/vox-speech/src/session.rs)

### `fill_slots_heuristic()`  (happy; EXTRACTED)
- [happy] fill_slots_heuristic extracts file paths enclosed in backticks from spoken commands  (crates/vox-speech/src/speech_intent.rs)
- [happy] fill_slots_heuristic resolves 'this file' to the active_file when context is provided  (crates/vox-speech/src/speech_intent.rs)

### `merge_bias_phrases()`  (happy; EXTRACTED)
- [happy] deduplicates bias phrases when merging lists (preserves single occurrence of 'a' across merged lists)  (crates/vox-speech/src/contextual_bias.rs)
- [happy] includes all phrases from both input lists in result, merging the bias vocabularies  (crates/vox-speech/src/contextual_bias.rs)

### `normalize_spoken_code_phrase()`  (happy; EXTRACTED)
- [happy] normalize_spoken_code_phrase() converts spoken camel-case phrases to camelCase code  (crates/vox-speech/src/speech_normalize.rs)
- [happy] normalize_spoken_code_phrase() converts the spoken phrase 'fat arrow' to the '=>' operator  (crates/vox-speech/src/speech_normalize.rs)

### `pick_best_transcript_index()`  (happy; EXTRACTED)
- [happy] pick_best_transcript_index() selects the parseable Vox hypothesis (index 1) when compiler-rerank feature is enabled  (crates/vox-speech/src/transcript_rerank.rs)
- [happy] pick_best_transcript_index() selects index 0 when compiler-rerank feature is disabled, regardless of parse validity  (crates/vox-speech/src/transcript_rerank.rs)

### `preprocess_audio_pcm_f32_reported`  (happy; EXTRACTED)
- [happy] when peak normalization is enabled, output peak amplitude is scaled to approximately 0.95 (±0.01) for quiet signals  (crates/vox-speech/src/acoustic_preprocess.rs)
- [happy] peak amplitude increases after preprocessing for quiet input signals  (crates/vox-speech/src/acoustic_preprocess.rs)

### `rerank_candidates_best_first_with_context()`  (happy; EXTRACTED)
- [happy] rerank_candidates_best_first_with_context() ranks hotword-matching hypothesis first when compiler-rerank feature is disabled  (crates/vox-speech/src/transcript_rerank.rs)
- [happy] rerank_candidates_best_first_with_context() retains at least one hotword hypothesis even with compiler-rerank feature enabled  (crates/vox-speech/src/transcript_rerank.rs)

### `should_commit_partial()`  (happy, invariant; EXTRACTED)
- [happy] should_commit_partial() returns true when audio is quiet below partial_quiet_ms and under max_wait_ms  (crates/vox-speech/src/streaming_partial.rs)
- [invariant] should_commit_partial() returns false when elapsed time exceeds max_wait_ms threshold  (crates/vox-speech/src/streaming_partial.rs)

### `speech_escalation_recommended()`  (happy; EXTRACTED)
- [happy] speech_escalation_recommended() returns true when either intent or action confidence is below threshold (0.2, 0.3)  (crates/vox-speech/src/tiering.rs)
- [happy] speech_escalation_recommended() returns false when both intent and action confidence are high (0.9, 0.9)  (crates/vox-speech/src/tiering.rs)

### `word_error_rate()`  (happy; EXTRACTED)
- [happy] returns 0.0 for identical reference and hypothesis strings  (crates/vox-speech/src/eval.rs)
- [happy] returns 0.0 when both reference and hypothesis are empty or whitespace-only  (crates/vox-speech/src/eval.rs)

### `OratioRuntimeConfig`  (happy; EXTRACTED)
- [happy] environment variables override default configuration values  (crates/vox-speech/src/runtime_config.rs)

### `OratioRuntimeConfig::merge_env`  (happy; EXTRACTED)
- [happy] loads tool_route_min_confidence configuration from VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE environment variable  (crates/vox-speech/src/runtime_config.rs)

### `SpeechLexicon::apply()`  (happy; EXTRACTED)
- [happy] SpeechLexicon.apply() substitutes aliases with their canonical forms in text  (crates/vox-speech/src/speech_lexicon.rs)

### `SpeechLexicon::merge_from()`  (invariant; EXTRACTED)
- [invariant] SpeechLexicon.merge_from() prefers the first lexicon's mapping when aliases conflict  (crates/vox-speech/src/speech_lexicon.rs)

### `bias_hit_score()`  (happy; EXTRACTED)
- [happy] counts substring matches in input text against bias phrases (score of 2 for 'getUser' and 'MENS' in 'call getUser for mens')  (crates/vox-speech/src/contextual_bias.rs)

### `clarification_prompt_for_slots()`  (happy; EXTRACTED)
- [happy] clarification_prompt_for_slots returns Some when a code_create intent envelope lacks required slots  (crates/vox-speech/src/speech_intent.rs)

### `generate_srt_file()`  (invariant; EXTRACTED)
- [invariant] frame timing conversion should stay within 200ms budget in frame-to-ms accuracy (timing error budget)  (crates/vox-speech/src/eval_srt.rs)

### `mean_timing_offset_ms`  (happy; EXTRACTED)
- [happy] computes mean timing offset in milliseconds correctly between expected and actual segments  (crates/vox-speech/src/eval_srt.rs)

### `mean_timing_offset_ms()`  (happy; EXTRACTED)
- [happy] mean_timing_offset_ms returns exact mean absolute timing error of 50.0 ms when expected segment bounds differ from actual by 50ms and 50ms respectively  (crates/vox-speech/src/eval_srt.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`OratioRuntimeConfig`** — only: _environment variables override default configuration values_
- **`OratioRuntimeConfig::merge_env`** — only: _loads tool_route_min_confidence configuration from VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE environment variable_
- **`OratioRuntimeConfig::merge_file`** — only: _parses TOML file and merges session_timing.capture_timeout_ms value_
- **`SpeechLexicon::apply()`** — only: _SpeechLexicon.apply() substitutes aliases with their canonical forms in text_
- **`bias_hit_score()`** — only: _counts substring matches in input text against bias phrases (score of 2 for 'getUser' and 'MENS' in 'call getUser for mens')_
- **`clarification_prompt_for_slots()`** — only: _clarification_prompt_for_slots returns Some when a code_create intent envelope lacks required slots_
- **`fill_slots_heuristic()`** — only: _fill_slots_heuristic extracts file paths enclosed in backticks from spoken commands_
- **`map_to_ast_target()`** — only: _maps natural language 'create a function called hello' to an AST target with node_kind='function' and symbol_name='hello'_
- **`mean_timing_offset_ms`** — only: _computes mean timing offset in milliseconds correctly between expected and actual segments_
- **`mean_timing_offset_ms()`** — only: _mean_timing_offset_ms returns exact mean absolute timing error of 50.0 ms when expected segment bounds differ from actual by 50ms and 50ms respectively_
- **`merge_bias_phrases()`** — only: _deduplicates bias phrases when merging lists (preserves single occurrence of 'a' across merged lists)_
- **`normalize_spoken_code_phrase()`** — only: _normalize_spoken_code_phrase() converts spoken camel-case phrases to camelCase code_
- **`pick_best_transcript_index()`** — only: _pick_best_transcript_index() selects the parseable Vox hypothesis (index 1) when compiler-rerank feature is enabled_
- **`preprocess_audio_pcm_f32_reported`** — only: _when peak normalization is enabled, output peak amplitude is scaled to approximately 0.95 (±0.01) for quiet signals_
- **`rerank_candidates_best_first_with_context()`** — only: _rerank_candidates_best_first_with_context() ranks hotword-matching hypothesis first when compiler-rerank feature is disabled_
- **`route_transcript_with_options`** — only: _returns action 'oratio.status' for high-confidence tool mode transcripts_
- **`speech_escalation_recommended()`** — only: _speech_escalation_recommended() returns true when either intent or action confidence is below threshold (0.2, 0.3)_
- **`transcribe_path()`** — only: _transcribe_path() preserves raw_text including whitespace and newlines from file_
- **`transcribe_path_session`** — only: _normalizes fixture text from 'mends' to 'mens' via transcription_
- **`word_error_rate()`** — only: _returns 0.0 for identical reference and hypothesis strings_
