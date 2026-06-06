//! Vox-native VCS value types. jj-lib types never leak across this boundary.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeId(pub u64);

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "chg-{:06}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub id: ChangeId,
    pub label: Option<String>,
    pub changed_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    pub changed_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub path: PathBuf,
    pub sides: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolveStrategy {
    TakeLeft,
    TakeRight,
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn change_id_displays_with_prefix() {
        assert_eq!(format!("{}", ChangeId(42)), "chg-000042");
    }
    #[test]
    fn diff_default_is_empty() {
        assert!(Diff::default().changed_paths.is_empty());
    }
}
