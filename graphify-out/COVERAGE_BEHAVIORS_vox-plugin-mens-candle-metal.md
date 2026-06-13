# Semantic Behavior Map — `vox-plugin-mens-candle-metal`

Deterministically synthesized from 19 distinct proven-behavior claims (of 19 extracted) across 10 symbols. 2 symbols have an explicit error-path proof; **8 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `preflight_native_qlora()`  (error, happy; EXTRACTED)
- [error] Rejects wte.weight tensor with incorrect rank (3D instead of 2D) and produces error message mentioning both the tensor key and dimension constraint  (crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs)
- [happy] Returns Ok with embed_key set to model.embed_tokens.weight for qwen2 models  (crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs)
- [happy] Correctly extracts vocab size (5) from config when model.embed_tokens.weight is present  (crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs)
- [happy] Correctly extracts d_model (7) from safetensors tensor dimensions  (crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs)
- [happy] Parses and returns layout.hidden_size matching config file value (7)  (crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs)
- [happy] Parses and returns layout.model_type from config (qwen2)  (crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs)

### `CheckpointState::load()`  (error, happy; EXTRACTED)
- [happy] CheckpointState::load() returns None when the checkpoint file does not exist  (crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs)
- [error] CheckpointState::load() returns None when the checkpoint file contains invalid JSON  (crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs)
- [error] CheckpointState::load() returns None when the checkpoint has an outdated schema version  (crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs)

### `ordered_full_block_weight_keys()`  (happy; EXTRACTED)
- [happy] ordered_full_block_weight_keys() includes all MLP projection keys (gate_proj, up_proj, down_proj) and attention keys (q_proj) in full block weights  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)
- [happy] ordered_full_block_weight_keys() includes both attention keys (attn.c_attn) and MLP keys (mlp.c_fc) for GPT-2  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)

### `ordered_middle_projection_keys()`  (happy; EXTRACTED)
- [happy] ordered_middle_projection_keys() generates correct middle projection keys in order for GPT-2 architectures  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)
- [happy] ordered_middle_projection_keys() tracks all layers in a multi-layer Qwen2 model correctly  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)

### `CheckpointState::delete()`  (happy; EXTRACTED)
- [happy] CheckpointState::delete() removes the checkpoint file such that subsequent load() returns None  (crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs)

### `CheckpointState::save() / CheckpointState::load()`  (happy; EXTRACTED)
- [happy] CheckpointState can be serialized to disk with save() and deserialized with load(), preserving all fields (epoch, global_step, pair_offset, shuffled_indices, rng_seed) correctly  (crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs)

### `HfTransformerLayout::from_config_json_str()`  (happy; EXTRACTED)
- [happy] HfTransformerLayout can parse a GPT-2 config JSON with n_embd, n_head, n_layer, and vocab_size fields  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)

### `middle_block_projection_key()`  (happy; EXTRACTED)
- [happy] middle_block_projection_key() produces correct HuggingFace naming convention keys like 'h.0.attn.c_proj.weight' for GPT-2  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)

### `middle_projection_key_for_layout()`  (happy; EXTRACTED)
- [happy] middle_projection_key_for_layout() produces correct HuggingFace naming keys like 'model.layers.0.self_attn.o_proj.weight' for Qwen2  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)

### `missing_middle_keys_report()`  (happy; EXTRACTED)
- [happy] missing_middle_keys_report() correctly identifies missing middle projection keys and respects max parameter limits  (crates/vox-plugin-mens-candle-metal/src/hf_keymap.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CheckpointState::delete()`** — only: _CheckpointState::delete() removes the checkpoint file such that subsequent load() returns None_
- **`CheckpointState::save() / CheckpointState::load()`** — only: _CheckpointState can be serialized to disk with save() and deserialized with load(), preserving all fields (epoch, global_step, pair_offset, shuffled_indices, rng_seed) correctly_
- **`HfTransformerLayout::from_config_json_str()`** — only: _HfTransformerLayout can parse a GPT-2 config JSON with n_embd, n_head, n_layer, and vocab_size fields_
- **`middle_block_projection_key()`** — only: _middle_block_projection_key() produces correct HuggingFace naming convention keys like 'h.0.attn.c_proj.weight' for GPT-2_
- **`middle_projection_key_for_layout()`** — only: _middle_projection_key_for_layout() produces correct HuggingFace naming keys like 'model.layers.0.self_attn.o_proj.weight' for Qwen2_
- **`missing_middle_keys_report()`** — only: _missing_middle_keys_report() correctly identifies missing middle projection keys and respects max parameter limits_
- **`ordered_full_block_weight_keys()`** — only: _ordered_full_block_weight_keys() includes all MLP projection keys (gate_proj, up_proj, down_proj) and attention keys (q_proj) in full block weights_
- **`ordered_middle_projection_keys()`** — only: _ordered_middle_projection_keys() generates correct middle projection keys in order for GPT-2 architectures_
