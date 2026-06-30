//! CodeRabbit tier limits and rate constants.
//!
//! Source: [CodeRabbit FAQ – Usage Limits](https://docs.coderabbit.ai/faq/) (per developer).
//! Last verified: 2026-04-06.
//! Re-verify quarterly or when billing tiers/features change.

/// CodeRabbit subscription tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodeRabbitTier {
    /// Free plan: 150 files, 3 reviews/hour (summary only).
    Free,
    /// 14-day trial: 150 files, 4 reviews/hour.
    Trial,
    /// Open-source plan: 150 files, 2 reviews/hour.
    #[default]
    Oss,
    /// Pro plan: 300 files, 8 reviews/hour.
    Pro,
    /// Enterprise: 300 files, 12 reviews/hour.
    Enterprise,
}

impl CodeRabbitTier {
    /// Maximum files per pull request review for this tier.
    pub fn files_per_review(self) -> u32 {
        match self {
            CodeRabbitTier::Free | CodeRabbitTier::Trial | CodeRabbitTier::Oss => 150,
            CodeRabbitTier::Pro | CodeRabbitTier::Enterprise => 300,
        }
    }

    /// Maximum PR reviews per hour for this tier.
    pub fn reviews_per_hour(self) -> u32 {
        match self {
            CodeRabbitTier::Free => 3,
            CodeRabbitTier::Trial => 4,
            CodeRabbitTier::Oss => 2,
            CodeRabbitTier::Pro => 8,
            CodeRabbitTier::Enterprise => 12,
        }
    }

    /// Minimum seconds to wait between triggering PRs to stay under reviews/hour.
    pub fn min_delay_between_prs_secs(self) -> u64 {
        3600 / self.reviews_per_hour() as u64
    }

    /// Recommended max_files_per_pr for batch planner (safe margin under cap).
    pub fn recommended_max_files_per_pr(self) -> u32 {
        match self {
            CodeRabbitTier::Free | CodeRabbitTier::Trial | CodeRabbitTier::Oss => 140,
            CodeRabbitTier::Pro | CodeRabbitTier::Enterprise => 250,
        }
    }

    /// Hard cap for batch planner (same as files_per_review).
    pub fn hard_cap(self) -> u32 {
        self.files_per_review()
    }
}

/// Clamp requested max files per PR to \([1, tier.files_per_review()]\).
#[must_use]
pub fn clamp_max_files_per_pr(tier: CodeRabbitTier, requested: u32) -> u32 {
    let cap = tier.files_per_review();
    requested.max(1).min(cap)
}

/// Clamp batch `hard_cap` to tier maximum (never above `files_per_review()`).
#[must_use]
pub fn clamp_hard_cap_to_tier(tier: CodeRabbitTier, requested: u32) -> u32 {
    let cap = tier.hard_cap();
    requested.max(1).min(cap)
}

/// Effective batch planner caps: both values stay within the tier hard cap and `max <= hard`.
#[must_use]
pub fn clamp_batch_caps(tier: CodeRabbitTier, max_files_per_pr: u32, hard_cap: u32) -> (u32, u32) {
    let hard = clamp_hard_cap_to_tier(tier, hard_cap);
    let max = clamp_max_files_per_pr(tier, max_files_per_pr).min(hard);
    (max.max(1), hard)
}

impl std::str::FromStr for CodeRabbitTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(CodeRabbitTier::Free),
            "trial" => Ok(CodeRabbitTier::Trial),
            "oss" => Ok(CodeRabbitTier::Oss),
            "pro" => Ok(CodeRabbitTier::Pro),
            "enterprise" => Ok(CodeRabbitTier::Enterprise),
            _ => Err(format!(
                "Unknown tier: {s}. Use: free, trial, oss, pro, enterprise"
            )),
        }
    }
}

impl std::fmt::Display for CodeRabbitTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeRabbitTier::Free => write!(f, "Free"),
            CodeRabbitTier::Trial => write!(f, "Trial"),
            CodeRabbitTier::Oss => write!(f, "OSS"),
            CodeRabbitTier::Pro => write!(f, "Pro"),
            CodeRabbitTier::Enterprise => write!(f, "Enterprise"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_files_per_review() {
        assert_eq!(CodeRabbitTier::Free.files_per_review(), 150);
        assert_eq!(CodeRabbitTier::Pro.files_per_review(), 300);
    }

    #[test]
    fn tier_min_delay_secs() {
        assert_eq!(CodeRabbitTier::Pro.min_delay_between_prs_secs(), 450);
    }

    #[test]
    fn clamp_max_respects_tier_cap() {
        assert_eq!(clamp_max_files_per_pr(CodeRabbitTier::Pro, 500), 300);
        assert_eq!(clamp_max_files_per_pr(CodeRabbitTier::Oss, 500), 150);
        assert_eq!(clamp_max_files_per_pr(CodeRabbitTier::Pro, 0), 1);
    }

    #[test]
    fn clamp_batch_caps_both_bounded() {
        let (max, hard) = clamp_batch_caps(CodeRabbitTier::Pro, 400, 500);
        assert_eq!(hard, 300);
        assert_eq!(max, 300);
        let (max2, hard2) = clamp_batch_caps(CodeRabbitTier::Oss, 200, 200);
        assert_eq!(hard2, 150);
        assert_eq!(max2, 150);
    }
}
