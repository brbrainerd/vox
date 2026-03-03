//! Security model for the Vox agent system.
//!
//! Provides:
//! - `SecurityPolicy` — per-agent permission rules
//! - `SecurityGuard` — validates requests against a policy
//! - `AuditLog` — append-only security event log
//! - Rate limiting primitives

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Policy types
// ---------------------------------------------------------------------------

/// Actions that can be permitted or denied.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityAction {
    ReadFile { path_glob: String },
    WriteFile { path_glob: String },
    ExecShell,
    NetworkRequest { domain_glob: String },
    DbRead,
    DbWrite,
    SpawnAgent,
    KillAgent,
    AccessSecrets,
}

/// A policy rule — either allow or deny an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub action: SecurityAction,
    pub allow: bool,
    pub reason: String,
}

/// Security policy for an agent or skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub id: String,
    pub rules: Vec<PolicyRule>,
    /// Default action when no rule matches
    pub default_allow: bool,
}

impl SecurityPolicy {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            rules: Vec::new(),
            default_allow: false, // deny-by-default
        }
    }

    /// Allow all actions by default.
    pub fn permissive(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            rules: Vec::new(),
            default_allow: true,
        }
    }

    pub fn allow(mut self, action: SecurityAction, reason: impl Into<String>) -> Self {
        self.rules.push(PolicyRule {
            action,
            allow: true,
            reason: reason.into(),
        });
        self
    }

    pub fn deny(mut self, action: SecurityAction, reason: impl Into<String>) -> Self {
        self.rules.push(PolicyRule {
            action,
            allow: false,
            reason: reason.into(),
        });
        self
    }

    /// Check whether an action is permitted. Returns `Ok(reason)` or `Err(reason)`.
    pub fn check(&self, action: &SecurityAction) -> Result<String, String> {
        for rule in &self.rules {
            if &rule.action == action {
                return if rule.allow {
                    Ok(rule.reason.clone())
                } else {
                    Err(rule.reason.clone())
                };
            }
        }
        if self.default_allow {
            Ok("default allow".into())
        } else {
            Err("no matching rule (default deny)".into())
        }
    }
}

// ---------------------------------------------------------------------------
// SecurityGuard
// ---------------------------------------------------------------------------

/// Validates requests against policies and rate limits.
pub struct SecurityGuard {
    policies: Mutex<HashMap<String, SecurityPolicy>>,
    rate_limits: Mutex<HashMap<String, RateLimiter>>,
}

impl SecurityGuard {
    pub fn new() -> Self {
        Self {
            policies: Mutex::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_policy(&self, policy: SecurityPolicy) {
        let id = policy.id.clone();
        self.policies
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, policy);
    }

    pub fn get_policy(&self, id: &str) -> Option<SecurityPolicy> {
        self.policies
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    /// Check an action for a given policy ID. Returns Ok() or Err(denial reason).
    pub fn check(&self, policy_id: &str, action: &SecurityAction) -> Result<(), String> {
        let policies = self.policies.lock().unwrap_or_else(|e| e.into_inner());
        match policies.get(policy_id) {
            Some(policy) => policy.check(action).map(|_| ()),
            None => Err(format!("No policy found for '{policy_id}'")),
        }
    }

    /// Check rate limit for a key (e.g. "agent_id:action_type").
    /// Returns Ok(()) if within limit, Err(wait_ms) if rate-limited.
    pub fn rate_check(&self, key: &str, limit: u32, window: Duration) -> Result<(), u64> {
        let mut limits = self.rate_limits.lock().unwrap_or_else(|e| e.into_inner());
        let limiter = limits
            .entry(key.to_string())
            .or_insert_with(|| RateLimiter::new(limit, window));
        if limiter.check() {
            Ok(())
        } else {
            Err(limiter.retry_after_ms())
        }
    }
}

impl Default for SecurityGuard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Rate limiter (sliding window)
// ---------------------------------------------------------------------------

struct RateLimiter {
    limit: u32,
    window: Duration,
    timestamps: VecDeque<Instant>,
}

impl RateLimiter {
    fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            timestamps: VecDeque::new(),
        }
    }

    fn check(&mut self) -> bool {
        let now = Instant::now();
        // Prune old timestamps
        while let Some(&front) = self.timestamps.front() {
            if now.duration_since(front) >= self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }
        if self.timestamps.len() < self.limit as usize {
            self.timestamps.push_back(now);
            true
        } else {
            false
        }
    }

    fn retry_after_ms(&self) -> u64 {
        if let Some(&oldest) = self.timestamps.front() {
            let elapsed = Instant::now().duration_since(oldest);
            if elapsed < self.window {
                return (self.window - elapsed).as_millis() as u64;
            }
        }
        0
    }
}

// ---------------------------------------------------------------------------
// AuditLog
// ---------------------------------------------------------------------------

/// An audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub agent_id: String,
    pub action: String,
    pub result: AuditResult,
    pub reason: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Allowed,
    Denied,
    RateLimited,
}

impl std::fmt::Display for AuditResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allowed => write!(f, "allowed"),
            Self::Denied => write!(f, "denied"),
            Self::RateLimited => write!(f, "rate_limited"),
        }
    }
}

/// In-memory append-only audit log (ring buffer, bounded to `capacity` entries).
pub struct AuditLog {
    entries: Mutex<VecDeque<AuditEntry>>,
    capacity: usize,
}

impl AuditLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn record(&self, entry: AuditEntry) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        if entry.result != AuditResult::Allowed {
            warn!(
                agent = %entry.agent_id,
                action = %entry.action,
                result = %entry.result,
                reason = %entry.reason,
                "Security audit event"
            );
        } else {
            info!(
                agent = %entry.agent_id,
                action = %entry.action,
                "Audit: allowed"
            );
        }
        entries.push_back(entry);
    }

    pub fn recent(&self, n: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.iter().rev().take(n).cloned().collect()
    }

    pub fn denied_count(&self) -> usize {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .iter()
            .filter(|e| e.result == AuditResult::Denied)
            .count()
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[allow(dead_code)]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_default_deny() {
        let p = SecurityPolicy::new("agent-1");
        let result = p.check(&SecurityAction::ExecShell);
        assert!(result.is_err());
    }

    #[test]
    fn policy_allow_rule() {
        let p = SecurityPolicy::new("agent-1").allow(SecurityAction::DbRead, "can read memory");
        assert!(p.check(&SecurityAction::DbRead).is_ok());
        assert!(p.check(&SecurityAction::DbWrite).is_err());
    }

    #[test]
    fn policy_deny_overrides() {
        let p = SecurityPolicy::permissive("agent-1")
            .deny(SecurityAction::ExecShell, "no shell access");
        assert!(p.check(&SecurityAction::DbRead).is_ok());
        assert!(p.check(&SecurityAction::ExecShell).is_err());
    }

    #[test]
    fn security_guard_checks_policy() {
        let guard = SecurityGuard::new();
        let policy = SecurityPolicy::new("agent-x").allow(SecurityAction::DbRead, "ok");
        guard.set_policy(policy);
        assert!(guard.check("agent-x", &SecurityAction::DbRead).is_ok());
        assert!(guard.check("agent-x", &SecurityAction::ExecShell).is_err());
    }

    #[test]
    fn rate_limiter_allows_within_limit() {
        let guard = SecurityGuard::new();
        for _ in 0..5 {
            assert!(guard.rate_check("key", 5, Duration::from_secs(60)).is_ok());
        }
        // 6th request should be rate-limited
        assert!(guard.rate_check("key", 5, Duration::from_secs(60)).is_err());
    }

    #[test]
    fn audit_log_records_and_retrieves() {
        let log = AuditLog::new(100);
        log.record(AuditEntry {
            agent_id: "agent-1".into(),
            action: "db_read".into(),
            result: AuditResult::Allowed,
            reason: "ok".into(),
            timestamp_ms: now_ms(),
        });
        log.record(AuditEntry {
            agent_id: "agent-1".into(),
            action: "exec_shell".into(),
            result: AuditResult::Denied,
            reason: "no shell access".into(),
            timestamp_ms: now_ms(),
        });
        assert_eq!(log.len(), 2);
        assert_eq!(log.denied_count(), 1);
        let recent = log.recent(5);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn audit_log_ring_buffer() {
        let log = AuditLog::new(3);
        for i in 0..5 {
            log.record(AuditEntry {
                agent_id: format!("agent-{i}"),
                action: "test".into(),
                result: AuditResult::Allowed,
                reason: "".into(),
                timestamp_ms: 0,
            });
        }
        assert_eq!(log.len(), 3, "should cap at capacity");
    }
}
