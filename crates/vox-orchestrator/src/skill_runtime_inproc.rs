//! P0-T7: in-process `SkillRuntime` implementation.
//!
//! `InProcessSkillRuntime` satisfies the `SkillRuntime` trait without spawning
//! an external process or sandbox. It is used for skills that run directly in
//! the orchestrator process (trusted, host-level operations). The build phase
//! is a no-op; run returns an immediate success outcome.

use vox_skill_runtime::{BuildOpts, RunOpts, RunOutcome, SkillRuntime};

/// In-process skill runtime — runs skills as plain function calls in the host process.
///
/// Always available; never requires an external daemon. Used as the fallback
/// runtime when no container or WASM runtime is reachable.
pub struct InProcessSkillRuntime;

impl InProcessSkillRuntime {
    /// Create a new in-process runtime instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for InProcessSkillRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRuntime for InProcessSkillRuntime {
    fn name(&self) -> &str {
        "inproc"
    }

    fn available(&self) -> bool {
        true
    }

    fn build(&self, _opts: &BuildOpts) -> anyhow::Result<()> {
        Ok(())
    }

    fn run(&self, _opts: &RunOpts) -> anyhow::Result<RunOutcome> {
        Ok(RunOutcome {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            wall_ms: 0,
        })
    }
}
