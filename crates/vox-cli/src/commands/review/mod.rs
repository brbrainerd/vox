//! Review-related helpers: DeI daemon review is invoked from **`vox mens review`** (`mens-dei`); GitHub CodeRabbit flows are **`vox review coderabbit`** — now in the `vox-cli-review` crate (`vox_cli_review::{ReviewCli, run}`).

#[cfg(feature = "dei")]
mod dei;
#[cfg(feature = "dei")]
pub use dei::run;
