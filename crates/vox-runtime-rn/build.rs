//! No build-time codegen needed.
//!
//! `vox-runtime-rn` uses uniffi's proc-macro mode: the `#[uniffi::export]`
//! attributes plus `uniffi::setup_scaffolding!()` in `src/lib.rs` produce
//! every piece of FFI glue at macro-expansion time. The build.rs file
//! exists solely so future cross-compile / `uniffi-bindgen-react-native`
//! integration has an obvious place to hook in.

fn main() {}
