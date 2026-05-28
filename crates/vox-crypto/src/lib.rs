//! Cryptographic primitives and thin facades (hashing, signing, age/x25519).

#![forbid(unsafe_code)]

pub mod facades;
pub use facades::*;
