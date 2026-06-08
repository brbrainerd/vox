## vox-identity — Semantic Behavior Map

Mapped 11 extracted Behavior claims (deduped to 8 distinct; 3 were repeated round-trip assertions) across 6 symbols spanning three surfaces: Ed25519 ephemeral signing (`ephemeral.rs`), X25519 pairing-key generation (`pairing_x25519.rs`), and the password-encrypted on-disk identity store (`storage.rs`). Coverage is strongest on the storage rotation path — `rotate_identity_at()` has happy, error, and invariant proof — and thinnest on the two cryptographic constructors, which are proven only on their happy paths despite having obvious failure/uniqueness contracts.

### `verify()` — `ephemeral.rs`
- **Happy:** reconstructing a `VerifyingKey` from hex-encoded pubkey bytes and verifying a signature against the original message succeeds.
- **Error:** verification returns false when the message is tampered with but signature and key are authentic.
- Error-path: yes. Edge/invariant: no (no malformed-signature or wrong-key rejection proven).

### `verifying_key_from_bytes()` — `ephemeral.rs`
- **Happy:** converting 32 valid bytes to a `VerifyingKey` does not error (asserted by two tests; deduped).
- Error-path: no. Edge/invariant: no. This is a fallible deserializer with no malformed/non-canonical/wrong-length input proven.

### `PairingKey::generate()` — `pairing_x25519.rs`
- **Invariant:** generated X25519 public key is exactly 32 bytes.
- Error-path: n/a (infallible generator). Edge/invariant: size only — no uniqueness/entropy proof.

### `rotate_identity_at()` — `storage.rs`
- **Happy:** with a nonexistent path, creates a new identity file and succeeds.
- **Invariant:** two rotations with the same password yield distinct identities with different `node_id`s.
- **Error:** wrong password returns `Err("Incorrect master password")`; empty password returns `Err`.
- Error-path: yes (two). Edge/invariant: yes. Best-covered symbol in the crate.

### `load_identity_at()` — `storage.rs`
- **Happy:** a freshly saved identity loads back with the correct password and yields a matching `node_id`.
- **Invariant:** after a failed rotation attempt, the original identity remains intact and still loads with the correct password.
- Error-path: no (its own error modes untested). Edge/invariant: yes (post-failure intactness).

## Semantic gaps

Symbols proven only on the happy path whose contract clearly has a failure, empty, or conflict mode:

1. **`verifying_key_from_bytes()` — validator with no rejection test (most actionable).** It is a fallible converter, yet every claim feeds it valid 32-byte input. The entire reason it returns a `Result` — rejecting malformed, non-canonical, or wrong-length bytes — is unproven. A round-trip with attacker-controlled pubkey bytes is a security surface; add a test that garbage / short / non-canonical points return `Err`.

2. **`PairingKey::generate()` — integrity/security surface with size-only proof.** Only the 32-byte length is asserted. The security-relevant property is distinctness/non-determinism across generations — notably, `rotate_identity_at()` IS proven to produce distinct `node_id`s, so the same uniqueness assertion is conspicuously missing here. Add a two-call distinctness test.

3. **`load_identity_at()` — loader with no direct error path.** Its failure modes (wrong password, missing file, corrupt/truncated identity blob) are only tested indirectly via rotation. As the read side of an encrypted store it should have its own wrong-password and corrupt-file rejection tests rather than relying on `rotate_identity_at`'s coverage.

Well-covered (no gap): `rotate_identity_at()` (happy+error+invariant) and `verify()` (happy+error).