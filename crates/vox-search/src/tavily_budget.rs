//! Session-scoped Tavily credit budget (no HTTP client — safe without `tavily` feature).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Thread-safe atomic credit counter for one MCP/CLI session.
#[derive(Debug, Clone)]
pub struct TavilySessionBudget {
    remaining: Arc<AtomicUsize>,
}

impl TavilySessionBudget {
    /// New budget with `limit` remaining credits.
    pub fn new(limit: usize) -> Self {
        Self {
            remaining: Arc::new(AtomicUsize::new(limit)),
        }
    }

    /// Returns `false` and does NOT decrement if already at zero.
    pub fn try_consume(&self, cost: usize) -> bool {
        let mut current = self.remaining.load(Ordering::SeqCst);
        loop {
            if current < cost {
                return false;
            }
            match self.remaining.compare_exchange_weak(
                current,
                current - cost,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(val) => current = val,
            }
        }
    }

    /// Remaining credits (best-effort; concurrent consumers may race).
    pub fn remaining(&self) -> usize {
        self.remaining.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_exhausts() {
        let b = TavilySessionBudget::new(2);
        assert!(b.try_consume(1));
        assert!(b.try_consume(1));
        assert!(!b.try_consume(1));
        assert_eq!(b.remaining(), 0);
    }
}
