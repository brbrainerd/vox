# Semantic Behavior Map — `vox-plugin-mens-candle-cuda`

Deterministically synthesized from 15 distinct proven-behavior claims (of 15 extracted) across 9 symbols. 2 symbols have an explicit error-path proof; **7 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `CheckpointState::load`  (edge, error, happy; EXTRACTED)
- [happy] load() returns Some with correct epoch, global_step, pair_offset, shuffled_indices, and rng_seed after save  (crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs)
- [edge] load() returns None when checkpoint file is missing from directory  (crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs)
- [error] load() returns None when checkpoint file contains invalid JSON  (crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs)
- [error] load() returns None when checkpoint schema version does not match expected version  (crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs)

### `ordered_middle_projection_keys`  (happy; EXTRACTED)
- [happy] ordered_middle_projection_keys() returns vector containing GPT2 projection key for single layer  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)
- [happy] ordered_middle_projection_keys() returns keys for all layers in multi-layer Qwen2 layout  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)
- [happy] ordered_middle_projection_keys() returns different projection keys based on layer type in Qwen3.5 config (linear_attn vs self_attn)  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)

### `ordered_full_block_weight_keys`  (happy; EXTRACTED)
- [happy] ordered_full_block_weight_keys() returns vector containing Qwen2 MLP projection keys (gate_proj, up_proj, down_proj, q_proj)  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)
- [happy] ordered_full_block_weight_keys() returns vector containing GPT2 attention and MLP keys (attn.c_attn, mlp.c_fc)  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)

### `CheckpointState::delete`  (happy; EXTRACTED)
- [happy] delete() removes checkpoint state so subsequent load() returns None  (crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs)

### `CheckpointState::save`  (happy; EXTRACTED)
- [happy] save() persists checkpoint state to disk such that load() recovers identical field values  (crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs)

### `middle_block_projection_key`  (happy; EXTRACTED)
- [happy] middle_block_projection_key(HfArchitecture::Gpt2, 0) returns 'h.0.attn.c_proj.weight'  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)

### `middle_projection_key_for_layout`  (happy; EXTRACTED)
- [happy] middle_projection_key_for_layout() returns 'model.layers.0.self_attn.o_proj.weight' for Qwen2 layout layer 0  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)

### `missing_middle_keys_report`  (happy; EXTRACTED)
- [happy] missing_middle_keys_report() returns only missing keys that are not in present set, respecting layer order  (crates/vox-plugin-mens-candle-cuda/src/hf_keymap.rs)

### `preflight_native_qlora()`  (error; EXTRACTED)
- [error] preflight_native_qlora() returns an error when qlora_require_full_proxy_stack is true and o_proj weights are missing from the model  (crates/vox-plugin-mens-candle-cuda/src/qlora_preflight.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CheckpointState::delete`** — only: _delete() removes checkpoint state so subsequent load() returns None_
- **`CheckpointState::save`** — only: _save() persists checkpoint state to disk such that load() recovers identical field values_
- **`middle_block_projection_key`** — only: _middle_block_projection_key(HfArchitecture::Gpt2, 0) returns 'h.0.attn.c_proj.weight'_
- **`middle_projection_key_for_layout`** — only: _middle_projection_key_for_layout() returns 'model.layers.0.self_attn.o_proj.weight' for Qwen2 layout layer 0_
- **`missing_middle_keys_report`** — only: _missing_middle_keys_report() returns only missing keys that are not in present set, respecting layer order_
- **`ordered_full_block_weight_keys`** — only: _ordered_full_block_weight_keys() returns vector containing Qwen2 MLP projection keys (gate_proj, up_proj, down_proj, q_proj)_
- **`ordered_middle_projection_keys`** — only: _ordered_middle_projection_keys() returns vector containing GPT2 projection key for single layer_
