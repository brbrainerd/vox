//! String-typed ID newtypes for DB rows. These wrap stringly-typed IDs from
//! libSQL columns (UUIDs, hashes, human-readable IDs) without committing to
//! a specific format. Pair these with the orchestrator's `TaskId(u64)` only
//! at the orchestrator boundary — they live in different layers.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
            pub fn into_string(self) -> String { self.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self { Self(s) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_string()) }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }
    };
}

string_id!(/// Stringly-typed agent ID as stored in DB rows.
    DbAgentId);
string_id!(/// Stringly-typed session ID as stored in DB rows.
    DbSessionId);
string_id!(/// Stringly-typed task ID as stored in DB rows.
    DbTaskId);
string_id!(/// Stringly-typed correlation ID as stored in DB rows.
    DbCorrelationId);
string_id!(/// Stringly-typed user ID as stored in DB rows.
    DbUserId);
string_id!(/// Stringly-typed plan-session ID as stored in DB rows.
    DbPlanSessionId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_serde_json() {
        let id = DbAgentId::new("agent-42");
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, "\"agent-42\"");
        let back: DbAgentId = serde_json::from_str(&s).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn display_matches_inner_string() {
        let id = DbSessionId::new("S-001");
        assert_eq!(format!("{id}"), "S-001");
    }

    #[test]
    fn distinct_types_do_not_unify() {
        let a = DbAgentId::new("a");
        let b = DbSessionId::new("a");
        assert_eq!(a.as_str(), b.as_str());
    }
}
