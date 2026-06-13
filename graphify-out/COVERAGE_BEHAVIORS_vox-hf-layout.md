# Semantic Behavior Map — `vox-hf-layout`

Deterministically synthesized from 6 distinct proven-behavior claims (of 6 extracted) across 2 symbols. 0 symbols have an explicit error-path proof; **2 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `HfTransformerLayout`  (happy; EXTRACTED)
- [happy] namespace_prefix is set to 'model.language_model.layers' for Qwen35 architecture  (crates/vox-hf-layout/src/lib.rs)
- [happy] num_hidden_layers field is correctly parsed from text_config JSON  (crates/vox-hf-layout/src/lib.rs)
- [happy] layer_types array is extracted from text_config and has correct length  (crates/vox-hf-layout/src/lib.rs)
- [happy] layer_types array elements are populated correctly from config (e.g., linear_attention)  (crates/vox-hf-layout/src/lib.rs)

### `HfTransformerLayout::from_config_json_str`  (happy; EXTRACTED)
- [happy] parses Qwen3.5 model_type from JSON config and sets architecture to Qwen35  (crates/vox-hf-layout/src/lib.rs)
- [happy] when layer_types is omitted from config, defaults to full_attention for all num_hidden_layers  (crates/vox-hf-layout/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`HfTransformerLayout`** — only: _namespace_prefix is set to 'model.language_model.layers' for Qwen35 architecture_
- **`HfTransformerLayout::from_config_json_str`** — only: _parses Qwen3.5 model_type from JSON config and sets architecture to Qwen35_
