//! Micro-VM runtime seam (P6-T3).
//!
//! `MicroVmRuntime` is a mock implementation of `SkillRuntime` that documents
//! the Firecracker/Kata Containers seam. In Phase 6 it always returns
//! `Err(NotImplemented)` — real bindings are deferred to v1.x.
//!
//! The Firecracker REST API and the Kata shim are NOT pulled in as deps;
//! this struct is a compile-time seam and a documentation artifact.

use crate::runtime::{BuildOpts, RunOpts, RunOutcome, SkillRuntime, Tier};

/// Mock micro-VM runtime for Firecracker / Kata Containers (P6-T3 seam).
///
/// All methods return `Err(anyhow!("MicroVmRuntime: not yet implemented ..."))`.
/// When real support lands (v1.x), this struct will be replaced by a
/// `FirecrackerRuntime` or `KataRuntime` backed by the respective REST/shim API.
#[derive(Debug, Default)]
pub struct MicroVmRuntime {
    /// Logical name (e.g. `"firecracker"` or `"kata"`). Used in error messages.
    pub backend_name: &'static str,
}

impl MicroVmRuntime {
    /// Create a new `MicroVmRuntime` with the given backend name.
    pub fn new(backend_name: &'static str) -> Self {
        Self { backend_name }
    }
}

impl SkillRuntime for MicroVmRuntime {
    fn name(&self) -> &str {
        self.backend_name
    }

    fn available(&self) -> bool {
        // The micro-VM backend is not available in Phase 6.
        false
    }

    fn tier(&self) -> Tier {
        Tier::MicroVm
    }

    fn build(&self, _opts: &BuildOpts) -> anyhow::Result<()> {
        anyhow::bail!(
            "MicroVmRuntime ({}): not yet implemented — firecracker/kata bindings deferred to v1.x",
            self.backend_name
        )
    }

    fn run(&self, _opts: &RunOpts) -> anyhow::Result<RunOutcome> {
        anyhow::bail!(
            "MicroVmRuntime ({}): not yet implemented — firecracker/kata bindings deferred to v1.x",
            self.backend_name
        )
    }
}
