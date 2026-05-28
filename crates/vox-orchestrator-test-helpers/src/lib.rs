//! Test fixtures, mocks, and golden helpers for `vox-orchestrator`.
//!
//! This crate is `kind = "test-only"` per `layers.toml` — production code must not depend on it.

use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum FixtureError {
    NotFound(PathBuf),
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "fixture not found: {}", p.display()),
            Self::Read { path, source } => {
                write!(f, "could not read fixture {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "could not parse fixture {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for FixtureError {}

/// Load and deserialize a golden fixture from `fixtures/<relative_path>`.
///
/// The fixtures directory is resolved relative to this crate's manifest at compile time.
/// `relative_path` is relative to `crates/vox-orchestrator-test-helpers/fixtures/`.
pub fn load_golden_fixture<T: DeserializeOwned>(
    relative_path: impl AsRef<Path>,
) -> Result<T, FixtureError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest_dir)
        .join("fixtures")
        .join(relative_path.as_ref());

    if !path.exists() {
        return Err(FixtureError::NotFound(path));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| FixtureError::Read {
        path: path.clone(),
        source: e,
    })?;

    serde_json::from_str(&content).map_err(|e| FixtureError::Parse { path, source: e })
}

// ---------------------------------------------------------------------------
// MockBulletinBoard
// ---------------------------------------------------------------------------

use std::sync::{Arc, Mutex};
use vox_orchestrator::types::AgentMessage;

/// In-memory bulletin board for use in tests.
/// Records all published messages for assertion.
#[derive(Clone, Default)]
pub struct MockBulletinBoard {
    messages: Arc<Mutex<Vec<AgentMessage>>>,
}

impl MockBulletinBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, msg: AgentMessage) {
        self.messages.lock().unwrap().push(msg);
    }

    pub fn recorded_messages(&self) -> Vec<AgentMessage> {
        self.messages.lock().unwrap().clone()
    }

    pub fn find_message<F>(&self, predicate: F) -> Option<AgentMessage>
    where
        F: Fn(&AgentMessage) -> bool,
    {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .find(|m| predicate(m))
            .cloned()
    }

    pub fn message_count(&self) -> usize {
        self.messages.lock().unwrap().len()
    }

    pub fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_bulletin_board_starts_empty_and_clears() {
        let board = MockBulletinBoard::new();
        assert_eq!(board.message_count(), 0);
        assert!(board.recorded_messages().is_empty());
        board.clear();
        assert_eq!(board.message_count(), 0);
    }
}
