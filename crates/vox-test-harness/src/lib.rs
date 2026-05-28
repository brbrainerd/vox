//! Shared test infrastructure for the Vox compiler and tooling pipelines.
//!
//! Provides shared builders, assertions, and helpers so no test file needs to
//! define `dummy_span()`, `minimal_module()`, or pipeline helpers locally.
//!
//! # Usage
//!
//! ```rust,ignore
//! use vox_test_harness::spans::dummy_span;
//! use vox_test_harness::hir_builders::minimal_hir_module;
//! use vox_test_harness::assertions::{has_error, error_messages};
//! use vox_test_harness::pipeline::{parse_str_unwrap, typecheck_str};
//! ```

pub mod assertions;
pub mod barriers;
pub mod diagnosis;
pub mod env_scratch;
pub mod hir_builders;
pub mod pipeline;
pub mod portpicker;
pub mod spans;
pub mod temp_root;
pub mod workspace_paths;
