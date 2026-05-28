//! Shell / mobile primitive projection from [`crate::hir::HirModule`].
//!
//! `@back_button`, `@deep_link`, and `@push` remain on HIR for lowering; this module is the
//! serde-stable projection consumed by TS emit (`mobile.ts`) and parity tests.

use serde::{Deserialize, Serialize};

use crate::hir::{HirBackButton, HirDeepLink, HirModule, HirPush};

/// Version of [`ShellProjectionModule`] JSON envelope.
pub const SHELL_PROJECTION_SCHEMA_VERSION: u32 = 1;

/// `@back_button` in shell projection (no source spans).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellBackButton {
    pub on_press: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// `@deep_link` in shell projection (no source spans).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellDeepLink {
    pub scheme: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub universal_link: Option<String>,
    pub on_link: String,
}

/// `@push` in shell projection (no source spans).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellPush {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_register: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_notification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_action: Option<String>,
}

/// Module-level shell projection for native / Capacitor-style wiring.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellProjectionModule {
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_button: Option<ShellBackButton>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_link: Option<ShellDeepLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<ShellPush>,
}

fn shell_back_button(b: &HirBackButton) -> ShellBackButton {
    ShellBackButton {
        on_press: b.on_press.clone(),
        fallback: b.fallback.clone(),
    }
}

fn shell_deep_link(d: &HirDeepLink) -> ShellDeepLink {
    ShellDeepLink {
        scheme: d.scheme.clone(),
        universal_link: d.universal_link.clone(),
        on_link: d.on_link.clone(),
    }
}

fn shell_push(p: &HirPush) -> ShellPush {
    ShellPush {
        on_register: p.on_register.clone(),
        on_notification: p.on_notification.clone(),
        on_action: p.on_action.clone(),
    }
}

/// Project shell primitives from a lowered module.
#[must_use]
pub fn project_shell_from_hir(m: &HirModule) -> ShellProjectionModule {
    ShellProjectionModule {
        schema_version: SHELL_PROJECTION_SCHEMA_VERSION,
        back_button: m.back_button.as_ref().map(shell_back_button),
        deep_link: m.deep_link.as_ref().map(shell_deep_link),
        push: m.push.as_ref().map(shell_push),
    }
}

/// Canonical JSON bytes for stable hashing / parity tests (sorted object keys at every depth).
pub fn canonical_shell_projection_bytes(
    m: &ShellProjectionModule,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_value(m)?;
    crate::canonical_json::sort_json_value_keys(&mut v);
    serde_json::to_vec(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_module_shell_projection_round_trips() {
        let m = HirModule::default();
        let s = project_shell_from_hir(&m);
        assert_eq!(s.schema_version, SHELL_PROJECTION_SCHEMA_VERSION);
        assert!(s.back_button.is_none());
        let bytes = canonical_shell_projection_bytes(&s).expect("bytes");
        let bytes2 = canonical_shell_projection_bytes(&s).expect("bytes2");
        assert_eq!(bytes, bytes2);
    }
}
