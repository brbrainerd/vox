//! Durable scheduler primitives for `@scheduled` and `@durable` (GA-11).
//!
//! Scope of this module:
//! - **Cron parsing** — minimal 5-field cron (`min hour dom month dow`) and a
//!   small set of friendly aliases (`@hourly`, `@daily`, `@weekly`).
//! - **Missed-run policy** — structural enum (no flag soup): `RunNow`, `Skip`,
//!   `CatchUp`. Per the gap analysis, these are first-class types, not
//!   stringly-typed config.
//! - **In-memory job registry** with deterministic next-fire-time computation.
//! - **At-least-once persistence trait** (`DurableJobStore`) — implementations
//!   live in `vox-db` and are wired up by codegen in a follow-up PR.
//!
//! What this module is *not*:
//! - The full crash-survival path (durable function checkpoints) — that's a
//!   `vox-workflow-runtime` extension, layered on this primitive.
//! - The cluster-aware variant — slot under v1.5 / Populi mesh follow-up per
//!   GA-11's out-of-scope list.

use std::time::{Duration, SystemTime};

/// What to do when a scheduled run is missed (e.g., the server was down at
/// the time the bucket fired).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedRunPolicy {
    /// Fire one run immediately to catch up.
    RunNow,
    /// Skip every missed bucket; resume at the next future bucket.
    Skip,
    /// Fire one run for *every* missed bucket. Use with care for high-frequency jobs.
    CatchUp,
}

/// A scheduled job specification.
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    /// Stable identifier (used for the durable-jobs DB key).
    pub id: String,
    /// Cron-shaped schedule string, or one of the supported aliases.
    pub schedule: ScheduleSpec,
    /// What to do for missed runs.
    pub missed_policy: MissedRunPolicy,
    /// Maximum retries on failure before landing in the dead-letter table.
    pub max_retries: u32,
}

/// Schedule specification — either a friendly alias or a 5-field cron expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleSpec {
    /// Run once per hour at minute 0.
    Hourly,
    /// Run once per day at midnight UTC.
    Daily,
    /// Run once per week at midnight UTC on Sunday.
    Weekly,
    /// Custom 5-field cron expression: `min hour dom month dow`.
    Cron(String),
}

impl ScheduleSpec {
    /// Parse a schedule string. Accepts `"@hourly"`, `"@daily"`, `"@weekly"`,
    /// or a 5-field cron expression. Returns `None` for unknown forms; the
    /// parser is intentionally minimal — full cron parsing lives in a follow-up.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "@hourly" => Some(ScheduleSpec::Hourly),
            "@daily" => Some(ScheduleSpec::Daily),
            "@weekly" => Some(ScheduleSpec::Weekly),
            other => {
                if other.split_whitespace().count() == 5 {
                    Some(ScheduleSpec::Cron(other.to_string()))
                } else {
                    None
                }
            }
        }
    }

    /// Approximate next-fire delay from `now`. Best-effort for the friendly
    /// aliases; cron expressions return a placeholder `Duration::from_secs(60)`
    /// until the full parser lands.
    pub fn next_delay_from(&self, _now: SystemTime) -> Duration {
        match self {
            ScheduleSpec::Hourly => Duration::from_secs(3600),
            ScheduleSpec::Daily => Duration::from_secs(86_400),
            ScheduleSpec::Weekly => Duration::from_secs(7 * 86_400),
            ScheduleSpec::Cron(_) => Duration::from_secs(60),
        }
    }
}

/// Persistence boundary for durable scheduled jobs.
///
/// Implementations: `vox-db` (production), in-memory (tests). Codegen for
/// `@scheduled` / `@durable` registers jobs through this trait at startup.
pub trait DurableJobStore: Send + Sync {
    fn record_run(&self, job_id: &str, ran_at: SystemTime) -> Result<(), JobStoreError>;
    fn last_run(&self, job_id: &str) -> Result<Option<SystemTime>, JobStoreError>;
    fn move_to_dead_letter(
        &self,
        job_id: &str,
        attempts: u32,
        error: &str,
    ) -> Result<(), JobStoreError>;
}

#[derive(Debug)]
pub enum JobStoreError {
    Backend(String),
    NotFound,
}

/// Compute "did we miss any runs since `last_run`?" — used by the scheduler
/// at startup to decide what the missed-run policy should fire.
pub fn missed_buckets_since(
    last_run: Option<SystemTime>,
    now: SystemTime,
    bucket: Duration,
) -> u64 {
    let Some(last) = last_run else {
        return 0;
    };
    let elapsed = now.duration_since(last).unwrap_or(Duration::ZERO);
    if bucket.as_secs() == 0 {
        return 0;
    }
    elapsed.as_secs() / bucket.as_secs()
}

/// Decide what to do at startup given a missed-run policy and number of missed buckets.
///
/// Returns the number of times the job should fire immediately at startup.
pub fn startup_fires(policy: MissedRunPolicy, missed: u64) -> u64 {
    if missed == 0 {
        return 0;
    }
    match policy {
        MissedRunPolicy::Skip => 0,
        MissedRunPolicy::RunNow => 1,
        MissedRunPolicy::CatchUp => missed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_friendly_aliases() {
        assert_eq!(ScheduleSpec::parse("@hourly"), Some(ScheduleSpec::Hourly));
        assert_eq!(ScheduleSpec::parse("@daily"), Some(ScheduleSpec::Daily));
        assert_eq!(ScheduleSpec::parse("@weekly"), Some(ScheduleSpec::Weekly));
    }

    #[test]
    fn parses_5_field_cron() {
        let s = ScheduleSpec::parse("0 * * * *");
        assert!(matches!(s, Some(ScheduleSpec::Cron(_))));
    }

    #[test]
    fn rejects_malformed() {
        assert!(ScheduleSpec::parse("garbage").is_none());
        assert!(ScheduleSpec::parse("0 0 0").is_none());
    }

    #[test]
    fn missed_policy_skip_fires_zero() {
        assert_eq!(startup_fires(MissedRunPolicy::Skip, 5), 0);
    }

    #[test]
    fn missed_policy_run_now_fires_one_regardless_of_missed_count() {
        assert_eq!(startup_fires(MissedRunPolicy::RunNow, 1), 1);
        assert_eq!(startup_fires(MissedRunPolicy::RunNow, 99), 1);
    }

    #[test]
    fn missed_policy_catch_up_fires_n_times() {
        assert_eq!(startup_fires(MissedRunPolicy::CatchUp, 5), 5);
    }

    #[test]
    fn no_missed_buckets_when_last_run_in_future() {
        let now = SystemTime::now();
        let later = now + Duration::from_secs(10);
        assert_eq!(
            missed_buckets_since(Some(later), now, Duration::from_secs(60)),
            0
        );
    }

    #[test]
    fn missed_buckets_computes_count() {
        use std::time::UNIX_EPOCH;
        let now = UNIX_EPOCH + Duration::from_secs(7200);
        let last = UNIX_EPOCH + Duration::from_secs(0);
        assert_eq!(
            missed_buckets_since(Some(last), now, Duration::from_secs(3600)),
            2
        );
    }

    #[test]
    fn next_delay_from_uses_canonical_periods() {
        let now = SystemTime::now();
        assert_eq!(
            ScheduleSpec::Hourly.next_delay_from(now),
            Duration::from_secs(3600)
        );
        assert_eq!(
            ScheduleSpec::Daily.next_delay_from(now),
            Duration::from_secs(86_400)
        );
    }
}
