//! SSOT for serde `#[serde(default = "…")]` helper functions.
//!
//! `vox-drift-check`'s `drift/serde-default-dup` rule flags local copies of
//! `default_true` / `default_false` etc. — import from here instead.

/// `#[serde(default = "vox_config::serde_defaults::default_true")]`.
pub const fn default_true() -> bool {
    true
}

/// `#[serde(default = "vox_config::serde_defaults::default_false")]`.
pub const fn default_false() -> bool {
    false
}
