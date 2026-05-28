use std::fs;
use std::sync::Arc;

use super::super::errors::SessionError;
use super::super::state::{Session, SessionState};
use super::SessionManager;
use super::db_io::run_session_db_io;

impl SessionManager {
    /// List all session IDs currently in memory.
    pub fn list(&self) -> Vec<&str> {
        self.sessions.keys().map(|s| s.as_str()).collect()
    }

    /// List all sessions sorted by last_active descending.
    pub fn list_sessions(&self) -> Vec<&Session> {
        let mut sessions: Vec<&Session> = self.sessions.values().collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_active));
        sessions
    }

    /// Archive idle sessions.
    ///
    /// Returns number of sessions archived.
    pub fn tick_lifecycle(&mut self) -> usize {
        let idle_timeout = self.config.idle_timeout_secs;
        let archive_timeout = self.config.archive_timeout_secs;
        let mut changed = 0;

        for session in self.sessions.values_mut() {
            match session.state {
                SessionState::Active | SessionState::Compacted => {
                    if session.is_timed_out(idle_timeout) {
                        session.state = SessionState::Idle;
                        changed += 1;
                    }
                }
                SessionState::Idle => {
                    if session.is_timed_out(archive_timeout) {
                        session.state = SessionState::Archived;
                        changed += 1;
                    }
                }
                SessionState::Archived => {}
            }
        }
        changed
    }

    /// Remove all archived sessions from memory and disk.
    ///
    /// Returns the number of sessions removed.
    pub fn cleanup(&mut self) -> Result<usize, SessionError> {
        let archived: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.state == SessionState::Archived)
            .map(|(id, _)| id.clone())
            .collect();

        let count = archived.len();
        for id in archived {
            self.sessions.remove(&id);
            if self.config.persist {
                let path = self.session_path(&id);
                if path.exists() {
                    fs::remove_file(&path)?;
                }
            }
            if let Some(db) = &self.db {
                let db_clone = db.clone();
                let sid = id.clone();
                let _ = run_session_db_io(
                    async move { db_clone.close_session(&sid, "archived").await },
                );
            }
        }
        Ok(count)
    }

    /// Attach a database handle late.
    pub fn attach_db(&mut self, db: Arc<vox_db::VoxDb>) {
        self.db = Some(db);
    }
}
