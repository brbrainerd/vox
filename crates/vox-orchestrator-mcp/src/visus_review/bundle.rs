//! Review-bundle loader: reads the capture harness's per-worker
//! entries-*.jsonl files (crates/vox-gui/ui/review-bundle/latest).

use std::path::Path;

fn default_theme() -> String {
    "default".into()
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BundleEntry {
    pub id: String,
    pub surface: String,
    pub state: String,
    pub viewport: String,
    pub browser: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    pub file: String,
    pub sha256: String,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub state_ok: bool,
    #[serde(default)]
    pub state_error: String,
    #[serde(default)]
    pub axe_violations: Vec<serde_json::Value>,
    #[serde(default)]
    pub console_errors: Vec<String>,
    #[serde(default)]
    pub console_warnings: Vec<String>,
    #[serde(default)]
    pub page_errors: Vec<String>,
    #[serde(default)]
    pub icon_issues: Vec<serde_json::Value>,
    #[serde(default)]
    pub overflow: serde_json::Value,
    #[serde(default)]
    pub capture_ms: u64,
    #[serde(default)]
    pub captured_at: String,
}

/// Load every `entries-*.jsonl` in `dir`. Returns (entries, skipped_lines).
pub fn load_bundle(dir: &Path) -> std::io::Result<(Vec<BundleEntry>, usize)> {
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for f in std::fs::read_dir(dir)? {
        let f = f?;
        let name = f.file_name().to_string_lossy().to_string();
        if !(name.starts_with("entries-") && name.ends_with(".jsonl")) {
            continue;
        }
        for line in std::fs::read_to_string(f.path())?.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<BundleEntry>(line) {
                Ok(e) => entries.push(e),
                Err(_) => skipped += 1,
            }
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((entries, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_capture_entry_line() {
        let line = r#"{"id":"chat--default--wide--chromium","surface":"chat","state":"default","viewport":"wide","browser":"chromium","theme":"default","file":"chat--default--wide--chromium.png","sha256":"ab","state_ok":true,"state_error":"","axe_violations":[{"id":"color-contrast","impact":"serious"}],"console_errors":["error: x"],"console_warnings":["warn: y"],"page_errors":[],"icon_issues":[],"overflow":{"bodyHorizontalOverflowPx":0,"scrollHostHorizontalOverflowPx":12,"contentHeightPx":2400},"capture_ms":1234,"captured_at":"t"}"#;
        let e: BundleEntry = serde_json::from_str(line).unwrap();
        assert_eq!(e.id, "chat--default--wide--chromium");
        assert_eq!(e.axe_violations.len(), 1);
        assert_eq!(e.overflow["scrollHostHorizontalOverflowPx"], 12);
        assert_eq!(e.capture_ms, 1234);
    }
    #[test]
    fn tolerates_missing_optional_fields() {
        let line = r#"{"id":"x","surface":"x","state":"default","viewport":"wide","browser":"firefox","file":"x.png","sha256":"cd"}"#;
        let e: BundleEntry = serde_json::from_str(line).unwrap();
        assert!(e.state_ok);
        assert!(e.console_errors.is_empty());
        assert_eq!(e.theme, "default");
    }
    #[test]
    fn load_bundle_reads_all_jsonl_files_and_skips_bad_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("entries-chromium-w0.jsonl"),
            "{\"id\":\"a\",\"surface\":\"s\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"chromium\",\"file\":\"a.png\",\"sha256\":\"1\"}\nnot-json\n").unwrap();
        std::fs::write(dir.path().join("entries-firefox-w1.jsonl"),
            "{\"id\":\"b\",\"surface\":\"s\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"firefox\",\"file\":\"b.png\",\"sha256\":\"2\"}\n").unwrap();
        let (entries, skipped) = load_bundle(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(skipped, 1);
    }
}
