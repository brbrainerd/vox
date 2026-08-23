//! Review-related helpers: DeI daemon review, invoked from **`vox mens review`**
//! (`mens-dei`). The GitHub CodeRabbit flows that used to live alongside this
//! were retired with the `vox-cli-review` crate.

#[cfg(feature = "dei")]
mod dei;
#[cfg(feature = "dei")]
pub use dei::run;
