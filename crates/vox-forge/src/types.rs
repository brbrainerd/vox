//! Forge-neutral types used across all `GitForgeProvider` implementations.
//!
//! All types here are Forge-agnostic. The key naming convention:
//! - GitHub calls these "Pull Requests"
//! - GitLab calls these "Merge Requests"
//! - Vox calls them all "ChangeRequests" (internal term)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ChangeRequest (PR/MR abstraction)
// ---------------------------------------------------------------------------

/// Forge-neutral identifier for a ChangeRequest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeRequestId(pub u64);

/// State of a ChangeRequest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeRequestState {
    Open,
    Closed,
    Merged,
    Draft,
}

/// CI/merge status of a ChangeRequest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeRequestStatus {
    Pending,
    Success,
    Failure,
    Error,
    Unknown,
}

/// A forge-neutral Change Request (PR on GitHub, MR on GitLab, PR on Gitea/Forgejo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    /// Forge-internal numeric ID.
    pub id: ChangeRequestId,
    /// Short human-readable number (e.g., #42).
    pub number: u64,
    /// Title of the change request.
    pub title: String,
    /// Description / body (markdown).
    pub body: String,
    /// Source branch (the branch being merged).
    pub source_branch: String,
    /// Target branch (the branch being merged into).
    pub target_branch: String,
    /// Current state.
    pub state: ChangeRequestState,
    /// CI/merge status.
    pub status: ChangeRequestStatus,
    /// Author login.
    pub author: String,
    /// Assignees.
    pub assignees: Vec<String>,
    /// Labels attached to this CR.
    pub labels: Vec<Label>,
    /// URL on the forge.
    pub web_url: String,
    /// When created (ISO 8601).
    pub created_at: String,
    /// When last updated (ISO 8601).
    pub updated_at: String,
    /// Whether this CR is a draft.
    pub is_draft: bool,
    /// Whether it is currently mergeable.
    pub mergeable: Option<bool>,
}

impl ChangeRequest {
    /// True if this CR is in an open, non-draft state.
    pub fn is_actionable(&self) -> bool {
        self.state == ChangeRequestState::Open && !self.is_draft
    }
}

// ---------------------------------------------------------------------------
// Label
// ---------------------------------------------------------------------------

/// A label on a ChangeRequest or issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Review
// ---------------------------------------------------------------------------

/// State of a code review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Pending,
}

/// A code review on a ChangeRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub reviewer: String,
    pub state: ReviewState,
    pub body: Option<String>,
    pub submitted_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Repository info
// ---------------------------------------------------------------------------

/// Forge-neutral repository metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeRepoInfo {
    /// Owner (user or org).
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// Full path (e.g., "owner/repo").
    pub full_name: String,
    /// Clone URL (HTTPS).
    pub clone_url: String,
    /// SSH URL.
    pub ssh_url: Option<String>,
    /// Default branch name.
    pub default_branch: String,
    /// Whether the repo is private.
    pub is_private: bool,
    /// Star count.
    pub stars: u64,
    /// Fork count.
    pub forks: u64,
    /// Open issues count.
    pub open_issues: u64,
    /// Description.
    pub description: Option<String>,
    /// Web URL.
    pub web_url: String,
}

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

/// A forge user account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeUser {
    pub login: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub web_url: String,
    pub is_bot: bool,
}

// ---------------------------------------------------------------------------
// Webhook events
// ---------------------------------------------------------------------------

/// A webhook event received from a forge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookEvent {
    /// A push to a branch.
    Push {
        branch: String,
        commits: Vec<String>,
        pusher: String,
    },
    /// A ChangeRequest was opened.
    ChangeRequestOpened { cr_number: u64, author: String },
    /// A ChangeRequest was merged.
    ChangeRequestMerged { cr_number: u64, merged_by: String },
    /// A ChangeRequest was closed (without merge).
    ChangeRequestClosed { cr_number: u64 },
    /// A review was submitted.
    ReviewSubmitted {
        cr_number: u64,
        reviewer: String,
        state: ReviewState,
    },
    /// A CI check completed.
    CheckCompleted {
        cr_number: Option<u64>,
        name: String,
        status: ChangeRequestStatus,
    },
    /// An unknown event type.
    Unknown { event_type: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_request_actionable() {
        let cr = ChangeRequest {
            id: ChangeRequestId(1),
            number: 42,
            title: "Fix parser bug".into(),
            body: String::new(),
            source_branch: "fix/parser".into(),
            target_branch: "main".into(),
            state: ChangeRequestState::Open,
            status: ChangeRequestStatus::Pending,
            author: "alice".into(),
            assignees: vec![],
            labels: vec![],
            web_url: "https://github.com/org/repo/pull/42".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            is_draft: false,
            mergeable: Some(true),
        };
        assert!(cr.is_actionable());
    }

    #[test]
    fn draft_cr_not_actionable() {
        let mut cr = ChangeRequest {
            id: ChangeRequestId(2),
            number: 43,
            title: "WIP: big refactor".into(),
            body: String::new(),
            source_branch: "wip/refactor".into(),
            target_branch: "main".into(),
            state: ChangeRequestState::Open,
            status: ChangeRequestStatus::Pending,
            author: "bob".into(),
            assignees: vec![],
            labels: vec![],
            web_url: "https://example.com/pr/43".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            is_draft: true,
            mergeable: None,
        };
        assert!(!cr.is_actionable());
        cr.is_draft = false;
        assert!(cr.is_actionable());
    }
}
