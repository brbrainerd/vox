//! T0.3 Part B: persisted per-repo "always allow this tool" allowlist.
//!
//! Backed by the existing `vox_db::preferences` facade over the
//! `user_preferences` table (registry `"approval_allowlist"`, key
//! `"{repo_id}.{tool_name}"`) — deliberately NOT a new table/migration, per
//! the T0.3 task scope. Persists across restarts automatically (it's a real
//! DB row) and is scoped per-repository: the same tool allowed in one repo
//! is NOT auto-approved in a different repo, since `repo_id` is part of the
//! preference key.
//!
//! This is tier 3 of the 5-tier precedence order documented in
//! `contracts/orchestration/permission-modes.v1.yaml` — checked by
//! `dispatch.rs`'s gate only after tier 2 (`permission_mode`, see
//! [`crate::permission_modes`]) says "still park" for this call.

const REGISTRY: &str = "approval_allowlist";

fn allowlist_key(repo_id: &str, tool: &str) -> String {
    format!("{repo_id}.{tool}")
}

/// Is `(repo_id, tool)` on the persisted allowlist? Best-effort: any DB
/// error (unattached DB, connection failure, ...) is treated as "not
/// allowlisted" — the gate must fail safe to parking for approval, never
/// silently auto-approve because a persistence lookup failed.
pub async fn is_allowlisted(repo_id: &str, tool: &str) -> bool {
    let key = allowlist_key(repo_id, tool);
    match vox_db::preferences::get_registry_preference(REGISTRY, &key).await {
        Ok(Some(value)) => value == "true",
        Ok(None) => false,
        Err(e) => {
            tracing::warn!(
                repo_id,
                tool,
                error = %e,
                "approval_allowlist lookup failed; treating as not-allowlisted (fail safe)"
            );
            false
        }
    }
}

/// Add `(repo_id, tool)` to the persisted allowlist (the GUI's "always
/// allow this tool in this repo" action). Idempotent — re-adding an already
/// allowlisted entry is a no-op success.
pub async fn add_entry(repo_id: &str, tool: &str) -> Result<(), vox_db::store::StoreError> {
    let key = allowlist_key(repo_id, tool);
    vox_db::preferences::set_registry_preference(REGISTRY, &key, "true").await
}

/// List every tool currently allowlisted for `repo_id` (for the GUI to
/// display current state). Best-effort: a DB error yields an empty list
/// rather than propagating, matching [`is_allowlisted`]'s fail-safe posture.
pub async fn list_for_repo(repo_id: &str) -> Vec<String> {
    let prefix = format!("{repo_id}.");
    match vox_db::preferences::get_all_registry_preferences(REGISTRY).await {
        Ok(entries) => entries
            .into_iter()
            .filter_map(|(key, value)| {
                if value != "true" {
                    return None;
                }
                key.strip_prefix(&prefix).map(ToString::to_string)
            })
            .collect(),
        Err(e) => {
            tracing::warn!(
                repo_id,
                error = %e,
                "approval_allowlist list failed; returning empty list (fail safe)"
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_key_scopes_by_repo_and_tool() {
        assert_eq!(
            allowlist_key("repo-a", "vox_write_file"),
            "repo-a.vox_write_file"
        );
        assert_ne!(
            allowlist_key("repo-a", "vox_write_file"),
            allowlist_key("repo-b", "vox_write_file")
        );
    }
}
