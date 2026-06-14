//! Identity types and monotonic ID generators.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Returns true when `v == 0.0`; used as `skip_serializing_if` for `attention_weight`.
pub fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

/// Unique identifier for a task within the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T-{:04}", self.0)
    }
}

/// Unique identifier for an agent within the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A-{:02}", self.0)
    }
}

/// Unique identifier mapping a question and response together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub u64);

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Q-{:04}", self.0)
    }
}

/// Helper parsing error for identifiers.
#[derive(Debug, thiserror::Error)]
#[error("Invalid ID format")]
pub struct IdParseError;

impl FromStr for TaskId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("T-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(TaskId(n))
    }
}

impl FromStr for AgentId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("A-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(AgentId(n))
    }
}

impl FromStr for CorrelationId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("Q-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(CorrelationId(n))
    }
}

/// Unique identifier for a batch submission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(pub u64);

impl fmt::Display for BatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B-{:04}", self.0)
    }
}

impl FromStr for BatchId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("B-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(BatchId(n))
    }
}

/// Handle for an active lock on a resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockToken(pub u64);

impl fmt::Display for LockToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L-{:04}", self.0)
    }
}

impl FromStr for LockToken {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("L-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(LockToken(n))
    }
}

/// Thread-safe counter for generating sequential TaskIds.
pub struct TaskIdGenerator(AtomicU64);

impl TaskIdGenerator {
    /// Starts issuing ids at `1`.
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    /// Returns the next monotonic task id.
    pub fn next(&self) -> TaskId {
        TaskId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TaskIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe counter for generating sequential AgentIds.
pub struct AgentIdGenerator(AtomicU64);

impl AgentIdGenerator {
    /// Starts issuing ids at `1`.
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    /// Returns the next monotonic agent id.
    pub fn next(&self) -> AgentId {
        AgentId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for AgentIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe counter for generating sequential CorrelationIds.
pub struct CorrelationIdGenerator(AtomicU64);

impl CorrelationIdGenerator {
    /// Starts issuing ids at `1`.
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    /// Returns the next monotonic correlation id for Q/A pairing.
    pub fn next(&self) -> CorrelationId {
        CorrelationId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for CorrelationIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_display() {
        assert_eq!(TaskId(42).to_string(), "T-0042");
    }

    #[test]
    fn agent_id_display() {
        assert_eq!(AgentId(3).to_string(), "A-03");
    }

    #[test]
    fn id_generators_are_sequential() {
        let tg = TaskIdGenerator::new();
        assert_eq!(tg.next(), TaskId(1));
        assert_eq!(tg.next(), TaskId(2));
        assert_eq!(tg.next(), TaskId(3));

        let ag = AgentIdGenerator::new();
        assert_eq!(ag.next(), AgentId(1));
        assert_eq!(ag.next(), AgentId(2));
    }
}

#[cfg(test)]
mod semcov_wave3_tests {
    #![allow(unused_imports)]
    use super::*;
    use std::str::FromStr;

    #[test]
    fn batch_id_from_str_with_prefix() {
        let id = BatchId::from_str("B-0042").expect("parse with prefix");
        assert_eq!(id, BatchId(42));
    }

    #[test]
    fn batch_id_from_str_bare_number() {
        let id = BatchId::from_str("7").expect("parse bare number");
        assert_eq!(id, BatchId(7));
    }

    #[test]
    fn batch_id_from_str_invalid_returns_err() {
        assert!(BatchId::from_str("not-a-number").is_err());
    }

    #[test]
    fn lock_token_display_format() {
        assert_eq!(LockToken(5).to_string(), "L-0005");
        assert_eq!(LockToken(0).to_string(), "L-0000");
        assert_eq!(LockToken(9999).to_string(), "L-9999");
    }

    #[test]
    fn lock_token_from_str_with_prefix() {
        let tok = LockToken::from_str("L-0012").expect("parse with prefix");
        assert_eq!(tok, LockToken(12));
    }

    #[test]
    fn lock_token_from_str_bare_number() {
        let tok = LockToken::from_str("99").expect("parse bare number");
        assert_eq!(tok, LockToken(99));
    }

    #[test]
    fn lock_token_from_str_invalid_returns_err() {
        assert!(LockToken::from_str("garbage").is_err());
    }

    #[test]
    fn is_zero_f64_true_for_zero() {
        assert!(is_zero_f64(&0.0));
    }

    #[test]
    fn is_zero_f64_false_for_nonzero() {
        assert!(!is_zero_f64(&0.001));
        assert!(!is_zero_f64(&-1.0));
        assert!(!is_zero_f64(&f64::INFINITY));
    }
}
