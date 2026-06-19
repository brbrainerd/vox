//! Auto-writes handoff-ledger entries (one per delegation) conforming to the
//! §C schema in docs/superpowers/antigravity-handoff-ledger.md. Serialized so
//! concurrent workers cannot collide on ids or lose appends.

use std::path::Path;
use std::sync::OnceLock;

pub const LEDGER_REL: &str = "docs/superpowers/antigravity-handoff-ledger.md";

static LEDGER_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub subsystem: String,
    pub task: String,
    pub outcome: String, // green | partial | failed
    pub timed_out: bool,
    pub exit_code: i32,
    pub files_changed: usize,
    pub timeout_secs: u64,
    pub date: String,
}

impl LedgerEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(subsystem: &str, task: &str, outcome: &str, timed_out: bool, exit_code: i32, files_changed: usize, timeout_secs: u64, date: &str) -> Self {
        Self { subsystem: subsystem.into(), task: task.into(), outcome: outcome.into(), timed_out, exit_code, files_changed, timeout_secs, date: date.into() }
    }
}

/// Highest real AGH-XXXX in `body` + 1. Skips the literal `AGH-NNNN` template.
pub fn next_agh_id(body: &str) -> String {
    let mut max = 0u32;
    for line in body.lines() {
        if let Some(rest) = line.trim().strip_prefix("# --- AGH-") {
            if let Some(n) = rest.strip_suffix(" ---").and_then(|s| s.parse::<u32>().ok()) {
                max = max.max(n);
            }
        }
    }
    format!("AGH-{:04}", max + 1)
}

pub fn render_entry(id: &str, e: &LedgerEntry) -> String {
    let task_yaml = e.task.replace('\'', "''");
    let errors = if e.timed_out {
        format!("errors_encountered:\n  - {{ what: \"timed out after {}s\", root_cause: \"agy hung or exceeded budget\", category: \"robustness\", who: agent }}\n", e.timeout_secs)
    } else if e.outcome != "green" {
        "errors_encountered:\n  - { what: \"non-green delegation\", root_cause: \"see worktree diff/stderr\", category: \"robustness\", who: agent }\n".to_string()
    } else {
        "errors_encountered: []\n".to_string()
    };
    format!(
        "```yaml\n# --- {id} ---\nid: {id}\ndate: {date}\nplan: docs/superpowers/plans/2026-06-19-agy-delegation-wedge1-2.md\nprompt_artifact: \"vox_agy_delegate (auto-logged)\"\nprompt_version: v1\nsubsystem: {subsystem}\ntarget: gemini-3.5-flash / antigravity\nclaude_inputs: [task-string]\ndelivered: [\"see agy/<slug> worktree diff\"]\nloc: {files}\noutcome: {outcome}\nverification: {{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {code}\" }}\n{errors}agent_deviations: []\nreview_findings: \"pending human review of worktree diff\"\nverdict: request-changes\nprompt_lessons: []\ncorrections_fed_back: []\ncommits: []\n# task: '{task}'\n```\n",
        id = id, date = e.date, subsystem = e.subsystem, outcome = e.outcome,
        code = e.exit_code, files = e.files_changed, task = task_yaml, errors = errors,
    )
}

/// Serialized read-allocate-append. Returns the allocated id.
pub async fn append_entry_locked(repo_root: &Path, entry: LedgerEntry) -> std::io::Result<String> {
    let _guard = LEDGER_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let path = repo_root.join(LEDGER_REL);
    let body = std::fs::read_to_string(&path)?;
    let id = next_agh_id(&body);
    let block = render_entry(&id, &entry);
    let mut out = body;
    if !out.ends_with('\n') { out.push('\n'); }
    out.push('\n');
    out.push_str(&block);
    std::fs::write(&path, out)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_id_skips_sentinel_and_increments() {
        let body = "# --- AGH-NNNN ---\n# --- AGH-0007 ---\nid: AGH-0007\n";
        assert_eq!(next_agh_id(body), "AGH-0008");
    }

    #[test]
    fn render_is_yaml_blockish_and_mineable() {
        let e = LedgerEntry::new("agy-delegation", "Refactor foo", "partial", false, 0, 3, 600, "2026-06-19");
        let block = render_entry("AGH-0008", &e);
        assert!(block.contains("# --- AGH-0008 ---"));
        assert!(block.contains("target: gemini-3.5-flash / antigravity"));
        assert!(block.contains("category:")); // non-green => mineable failure vocab
    }

    #[tokio::test]
    async fn append_roundtrip_advances_id() {
        let dir = std::env::temp_dir().join(format!("agyledger-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs/superpowers")).unwrap();
        let p = dir.join(LEDGER_REL);
        std::fs::write(&p, "## §C\n# --- AGH-0007 ---\nid: AGH-0007\n").unwrap();
        let id = append_entry_locked(&dir, LedgerEntry::new("s","t","green",false,0,1,60,"2026-06-19")).await.unwrap();
        assert_eq!(id, "AGH-0008");
        let id2 = append_entry_locked(&dir, LedgerEntry::new("s","t","green",false,0,1,60,"2026-06-19")).await.unwrap();
        assert_eq!(id2, "AGH-0009");
    }
}
