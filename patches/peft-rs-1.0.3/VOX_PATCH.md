# Vox fork notes (peft-rs 1.0.3)

Upstream: `crates.io` peft-rs 1.0.3.

Changes in `Cargo.toml`:

1. **Bumped `candle-core` and `candle-nn` from `0.9` to `0.10`** so peft-rs shares the same
   candle crate instance as our workspace and the qlora-rs patch. Without this, Cargo resolves
   two separate copies of candle_core (0.9 for peft-rs, 0.10 for workspace), causing type
   incompatibility errors at the qlora-rs/peft-rs boundary.

Reconcile with upstream when bumping peft-rs or candle-core.
