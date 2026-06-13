# Semantic Behavior Map — `vox-orchestrator-test-helpers`

Deterministically synthesized from 3 distinct proven-behavior claims (of 3 extracted) across 3 symbols. 0 symbols have an explicit error-path proof; **3 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `MockBulletinBoard::clear`  (happy; EXTRACTED)
- [happy] clear() empties the bulletin board, leaving message_count() at 0  (crates/vox-orchestrator-test-helpers/src/lib.rs)

### `MockBulletinBoard::message_count`  (happy; EXTRACTED)
- [happy] message_count() returns 0 when the bulletin board is newly initialized  (crates/vox-orchestrator-test-helpers/src/lib.rs)

### `MockBulletinBoard::recorded_messages`  (happy; EXTRACTED)
- [happy] recorded_messages() returns an empty vector when the bulletin board is newly initialized  (crates/vox-orchestrator-test-helpers/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`MockBulletinBoard::clear`** — only: _clear() empties the bulletin board, leaving message_count() at 0_
- **`MockBulletinBoard::message_count`** — only: _message_count() returns 0 when the bulletin board is newly initialized_
- **`MockBulletinBoard::recorded_messages`** — only: _recorded_messages() returns an empty vector when the bulletin board is newly initialized_
