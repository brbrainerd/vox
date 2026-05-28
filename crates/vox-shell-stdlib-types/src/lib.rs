//! Shared data types for the Vox shell stdlib surface (`std.fs.*`, `std.csv`).
//!
//! Both `vox-compiler` (interpreter path) and `vox-actor-runtime` (codegen path)
//! need the same file-record shape. This L0 crate holds the canonical definition
//! so neither side has to duplicate it or create a cycle.

pub mod fs_types;
