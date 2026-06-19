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
    /// Real verification summary (e.g. "build: pass, test: pass"). None ⇒ the
    /// legacy "n/a" default is rendered, so existing callers are unaffected.
    pub verification: Option<String>,
}

impl LedgerEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(subsystem: &str, task: &str, outcome: &str, timed_out: bool, exit_code: i32, files_changed: usize, timeout_secs: u64, date: &str) -> Self {
        Self { subsystem: subsystem.into(), task: task.into(), outcome: outcome.into(), timed_out, exit_code, files_changed, timeout_secs, date: date.into(), verification: None }
    }

    /// Attach a real verification summary; overrides the "n/a" default in render.
    pub fn with_verification(mut self, v: impl Into<String>) -> Self {
        self.verification = Some(v.into());
        self
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
    let verification = e.verification.clone().unwrap_or_else(|| {
        format!("{{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {}\" }}", e.exit_code)
    });
    format!(
        "```yaml\n# --- {id} ---\nid: {id}\ndate: {date}\nplan: docs/superpowers/plans/2026-06-19-agy-delegation-wedge1-2.md\nprompt_artifact: \"vox_agy_delegate (auto-logged)\"\nprompt_version: v1\nsubsystem: {subsystem}\ntarget: gemini-3.5-flash / antigravity\nclaude_inputs: [task-string]\ndelivered: [\"see agy/<slug> worktree diff\"]\nloc: {files}\noutcome: {outcome}\nverification: {verification}\n{errors}agent_deviations: []\nreview_findings: \"pending human review of worktree diff\"\nverdict: request-changes\nprompt_lessons: []\ncorrections_fed_back: []\ncommits: []\n# task: '{task}'\n```\n",
        id = id, date = e.date, subsystem = e.subsystem, outcome = e.outcome,
        files = e.files_changed, task = task_yaml, errors = errors, verification = verification,
    )
}

/// The Claude-side adversarial review outcome for one handoff.
#[derive(Debug, Clone)]
pub struct ReviewRecord {
    pub verdict: String,         // approve | approve-with-followups | request-changes
    pub categories: Vec<String>, // from the stable §B vocabulary
    pub findings: String,
    pub lessons: Vec<String>,
    pub date: String,
}

fn yaml_inline(s: &str) -> String {
    s.replace('"', "'").replace(['\n', '\r'], " ")
}

pub fn render_review(id: &str, r: &ReviewRecord) -> String {
    let cats = r.categories.iter().map(|c| yaml_inline(c)).collect::<Vec<_>>().join(", ");
    let lessons = if r.lessons.is_empty() {
        "  []".to_string()
    } else {
        r.lessons.iter().map(|l| format!("  - \"{}\"", yaml_inline(l))).collect::<Vec<_>>().join("\n")
    };
    format!(
        "```yaml\n# --- {id}-review ---\nreview_of: {id}\ndate: {date}\nverdict: {verdict}\ncategories: [{cats}]\nreview_findings: \"{findings}\"\nprompt_lessons:\n{lessons}\n```\n",
        id = id, date = r.date, verdict = yaml_inline(&r.verdict), cats = cats,
        findings = yaml_inline(&r.findings), lessons = lessons,
    )
}

/// Append a `{id}-review` addendum under the same lock (append-only).
pub async fn append_review_locked(repo_root: &Path, id: &str, r: &ReviewRecord) -> std::io::Result<()> {
    let _guard = LEDGER_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let path = repo_root.join(LEDGER_REL);
    let mut body = std::fs::read_to_string(&path)?;
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push('\n');
    body.push_str(&render_review(id, r));
    std::fs::write(&path, body)?;
    Ok(())
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

    #[test]
    fn with_verification_overrides_the_default_block() {
        let e = LedgerEntry::new("agy-pipeline", "Do X", "green", false, 0, 2, 600, "2026-06-19")
            .with_verification("build: pass, test: pass");
        let block = render_entry("AGH-0010", &e);
        assert!(block.contains("build: pass, test: pass"));
        assert!(!block.contains("tests: \"n/a\""));
    }

    #[tokio::test]
    async fn append_review_writes_keyed_addendum() {
        let dir = std::env::temp_dir().join(format!("agyrev-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs/superpowers")).unwrap();
        let p = dir.join(LEDGER_REL);
        std::fs::write(&p, "## §C\n# --- AGH-0007 ---\nid: AGH-0007\n").unwrap();
        let rec = ReviewRecord {
            verdict: "request-changes".into(),
            categories: vec!["hallucinated-api".into(), "scope-creep".into()],
            findings: "Invented useQuery(api.x) with no import".into(),
            lessons: vec!["Verify framework primitives in-repo".into()],
            date: "2026-06-19".into(),
        };
        append_review_locked(&dir, "AGH-0007", &rec).await.unwrap();
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("# --- AGH-0007-review ---"));
        assert!(body.contains("verdict: request-changes"));
        assert!(body.contains("hallucinated-api"));
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
