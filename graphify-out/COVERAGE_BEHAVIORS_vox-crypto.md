# Semantic Behavior Map — `vox-crypto`

Deterministically synthesized from 18 distinct proven-behavior claims (of 18 extracted) across 12 symbols. 3 symbols have an explicit error-path proof; **5 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `encrypt_with_nonce`  (error; EXTRACTED)
- [error] encrypt_with_nonce rejects 11-byte nonces with an error containing 'Invalid nonce length'  (crates/vox-crypto/src/facades.rs)
- [error] encrypt_with_nonce rejects 13-byte nonces with an Err result  (crates/vox-crypto/src/facades.rs)
- [error] encrypt_with_nonce rejects empty (0-byte) nonces with an Err result  (crates/vox-crypto/src/facades.rs)

### `checked_nonce`  (happy, invariant; EXTRACTED)
- [invariant] checked_nonce rejects all nonce lengths except exactly 12 bytes  (crates/vox-crypto/src/facades.rs)
- [happy] checked_nonce accepts exactly 12-byte nonces  (crates/vox-crypto/src/facades.rs)

### `decrypt`  (happy; EXTRACTED)
- [happy] decrypt recovers original plaintext from ciphertext produced by encrypt  (crates/vox-crypto/src/facades.rs)
- [happy] decrypt recovers original plaintext from ciphertext with a fixed key  (crates/vox-crypto/tests/facades_integration.rs)

### `encrypt`  (happy; EXTRACTED)
- [happy] encrypt produces valid ciphertext from plaintext  (crates/vox-crypto/src/facades.rs)
- [happy] encrypt produces valid ciphertext from plaintext with a fixed key  (crates/vox-crypto/tests/facades_integration.rs)

### `verify`  (error, happy; EXTRACTED)
- [happy] verify returns true when verifying a signature produced by sign for the same message  (crates/vox-crypto/tests/facades_integration.rs)
- [error] verify returns false when verifying a signature against a different (tampered) message  (crates/vox-crypto/tests/facades_integration.rs)

### `compliance_hash`  (invariant; EXTRACTED)
- [invariant] compliance_hash produces identical output for identical input across invocations  (crates/vox-crypto/tests/facades_integration.rs)

### `decrypt_with_nonce`  (error; EXTRACTED)
- [error] decrypt_with_nonce rejects 11-byte nonces with an Err result  (crates/vox-crypto/src/facades.rs)

### `fast_hash`  (invariant; EXTRACTED)
- [invariant] fast_hash produces identical output for identical input across invocations  (crates/vox-crypto/tests/facades_integration.rs)

### `generate_sym_key`  (happy; INFERRED)
- [happy] generate_sym_key produces a key usable for encrypt/decrypt operations  (crates/vox-crypto/src/facades.rs)

### `seal`  (happy; EXTRACTED)
- [happy] seal produces a sealed box from plaintext and a recipient public key  (crates/vox-crypto/tests/facades_integration.rs)

### `secure_hash`  (invariant; EXTRACTED)
- [invariant] secure_hash produces identical output for identical input across invocations  (crates/vox-crypto/tests/facades_integration.rs)

### `unseal`  (happy; EXTRACTED)
- [happy] unseal recovers original plaintext from a sealed box with the corresponding secret key  (crates/vox-crypto/tests/facades_integration.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`decrypt`** — only: _decrypt recovers original plaintext from ciphertext produced by encrypt_
- **`encrypt`** — only: _encrypt produces valid ciphertext from plaintext_
- **`generate_sym_key`** — only: _generate_sym_key produces a key usable for encrypt/decrypt operations_
- **`seal`** — only: _seal produces a sealed box from plaintext and a recipient public key_
- **`unseal`** — only: _unseal recovers original plaintext from a sealed box with the corresponding secret key_
