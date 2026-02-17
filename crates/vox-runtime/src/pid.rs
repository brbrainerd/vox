use std::sync::atomic::{AtomicU64, Ordering};
use std::fmt;

static NEXT_PID: AtomicU64 = AtomicU64::new(1);

/// Process identifier for actors in the Vox runtime.
/// Unique within a runtime instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pid(u64);

impl Pid {
    /// Allocate a fresh, globally unique Pid.
    pub fn new() -> Self {
        Self(NEXT_PID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw numeric value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for Pid {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<0.{}>", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_unique() {
        let a = Pid::new();
        let b = Pid::new();
        assert_ne!(a, b);
    }

    #[test]
    fn test_pid_display() {
        let p = Pid::new();
        let s = p.to_string();
        assert!(s.starts_with("<0."));
        assert!(s.ends_with('>'));
    }
}
