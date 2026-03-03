//! Gamification notifications and messaging.

use crate::util::now_unix;
use serde::{Deserialize, Serialize};

/// The type of gamification notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationType {
    LevelUp,
    AchievementUnlocked,
    StreakContinued,
    StreakLost,
    ChallengeCompleted,
    CompanionStatus,
    QuestCompleted,
}

/// A notification meant for the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub user_id: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub message: String,
    pub read: bool,
    pub created_at: i64,
}

impl Notification {
    pub fn new(
        user_id: impl Into<String>,
        notif_type: NotificationType,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id: format!("notif-{}-{}", now_unix(), seq),
            user_id: user_id.into(),
            notification_type: notif_type,
            title: title.into(),
            message: message.into(),
            read: false,
            created_at: now_unix(),
        }
    }

    /// Mark the notification as read.
    pub fn mark_read(&mut self) {
        self.read = true;
    }
}

/// Local storage of unread notifications during a session.
#[derive(Debug, Clone, Default)]
pub struct NotificationManager {
    notifications: Vec<Notification>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
        }
    }

    /// Add a new notification.
    pub fn push(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Retrieve all unread notifications.
    pub fn get_unread(&self) -> Vec<&Notification> {
        self.notifications.iter().filter(|n| !n.read).collect()
    }

    /// Mark a specific notification as read.
    pub fn mark_read(&mut self, notif_id: &str) -> bool {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == notif_id) {
            n.mark_read();
            true
        } else {
            false
        }
    }

    /// Mark all as read.
    pub fn mark_all_read(&mut self) {
        for n in &mut self.notifications {
            n.mark_read();
        }
    }

    /// Clear all read notifications.
    pub fn clear_read(&mut self) {
        self.notifications.retain(|n| !n.read);
    }
}

// ── Tests ─────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_and_read_notification() {
        let mut mgr = NotificationManager::new();
        let notif = Notification::new(
            "user-1",
            NotificationType::LevelUp,
            "Level Up!",
            "You reached Level 5",
        );
        let id = notif.id.clone();
        mgr.push(notif);

        assert_eq!(mgr.get_unread().len(), 1);
        assert!(mgr.mark_read(&id));
        assert_eq!(mgr.get_unread().len(), 0);
    }

    #[test]
    fn clear_read() {
        let mut mgr = NotificationManager::new();
        mgr.push(Notification::new(
            "u1",
            NotificationType::StreakContinued,
            "T",
            "M",
        ));
        mgr.push(Notification::new(
            "u1",
            NotificationType::StreakLost,
            "T",
            "M",
        ));
        mgr.mark_all_read();
        mgr.push(Notification::new(
            "u1",
            NotificationType::QuestCompleted,
            "T",
            "M",
        ));

        mgr.clear_read();
        assert_eq!(mgr.notifications.len(), 1); // Only the unread one should remain
    }
}
