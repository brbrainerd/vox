//! Host capability probing and `requires-tag` satisfaction.
//!
//! `catalog.toml` entries carry an optional `requires-tag` (e.g. `"nvidia-gpu"`,
//! `"apple-silicon"`) naming a hardware/platform capability a plugin needs.
//! [`probe`] inspects the current host and produces the [`CapabilitySet`] of
//! tags it actually has; [`CapabilitySet::satisfies`] checks a plugin's
//! `requires-tag` against that set. Probing degrades gracefully: any failure
//! to detect a capability simply omits its tag, it never panics and never
//! returns an error.

use std::collections::BTreeSet;

/// Host hardware/platform tags a plugin's `requires-tag` is checked against.
/// Always includes `"cpu-only"`. Probing degrades to fewer tags on any
/// failure — it must never panic and never require a toolchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet(BTreeSet<String>);

impl CapabilitySet {
    /// Build a set from an explicit list of tags (mainly for tests and
    /// callers that already know the host's capabilities).
    pub fn from_tags<I: IntoIterator<Item = T>, T: Into<String>>(tags: I) -> Self {
        Self(tags.into_iter().map(Into::into).collect())
    }

    /// `None` (no `requires-tag` declared) is always satisfied. `Some(tag)` is
    /// satisfied iff `tag` is in the set.
    pub fn satisfies(&self, requires_tag: Option<&str>) -> bool {
        match requires_tag {
            None => true,
            Some(tag) => self.0.contains(tag),
        }
    }
}

/// Probe this host's capabilities. Never panics; a probe failure for one
/// capability yields fewer tags, never a propagated error.
pub fn probe() -> CapabilitySet {
    let mut tags = BTreeSet::new();
    tags.insert("cpu-only".to_string());
    if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
        tags.insert("apple-silicon".to_string());
        // Every Apple Silicon Mac has a Metal-capable GPU. There is no
        // lightweight Metal-probe library in this tree (candle_core's Metal
        // support requires its heavy `metal` cargo feature, which
        // vox-plugin-host must not depend on just to answer "is there a
        // GPU"), so `metal` is derived from `apple-silicon` at compile time
        // rather than probed at runtime. Revisit only as a deliberate,
        // separate follow-up if a false positive is ever observed.
        tags.insert("metal".to_string());
    }
    if cuda_driver_present() {
        tags.insert("nvidia-gpu".to_string());
    }
    CapabilitySet(tags)
}

/// Whether a CUDA-capable driver can be loaded on this host. Any load
/// failure (missing library, no permissions, wrong arch, unsupported OS)
/// means "not present", not an error to propagate.
// The workspace denies unsafe_code by default (Cargo.toml `[workspace.lints]`);
// this is the one deliberate FFI-adjacent exception for this task, see the
// SAFETY comment on the unsafe block below.
#[allow(unsafe_code)]
fn cuda_driver_present() -> bool {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &["nvcuda.dll"]
    } else if cfg!(target_os = "macos") {
        &[] // CUDA ships no macOS driver on Apple Silicon or Intel since 2021.
    } else {
        &["libcuda.so.1", "libcuda.so"]
    };
    candidates.iter().any(|name| {
        // SAFETY: `libloading::Library::new` is unsafe because loading a
        // shared library runs its initializers, but we only load
        // well-known system driver libraries by name to check whether they
        // exist and are loadable — no symbols are resolved or called here,
        // so there is no untrusted code path beyond what the OS's dynamic
        // linker already runs for any loaded library.
        unsafe { libloading::Library::new(name) }.is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_with_no_requires_tag_is_always_satisfied() {
        let caps = CapabilitySet::from_tags(["cpu-only"]);
        assert!(caps.satisfies(None));
    }

    #[test]
    fn a_requires_tag_must_be_present_in_the_probe() {
        let caps = CapabilitySet::from_tags(["apple-silicon", "metal"]);
        assert!(caps.satisfies(Some("apple-silicon")));
        assert!(!caps.satisfies(Some("nvidia-gpu")));
    }

    #[test]
    fn probe_never_panics_and_always_reports_cpu_only() {
        let caps = probe();
        assert!(caps.satisfies(Some("cpu-only")));
    }
}
