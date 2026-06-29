//! Server-side allowlist filter — defense-in-depth (spec §3.3).
//!
//! The client already redacts before sending, but the server MUST re-apply
//! the taxonomy allowlist to every incoming record.  A compromised or
//! misconfigured client must not be able to exfiltrate free-form data.

use std::collections::{HashMap, HashSet};

use crate::schema::{Category, Taxonomy};

/// Build an allowlist map: `event_name → Set<allowed field names>`.
pub fn build_allowlist(taxonomy: &Taxonomy) -> HashMap<String, HashSet<String>> {
    taxonomy
        .categories
        .iter()
        .map(|cat: &Category| {
            let fields: HashSet<String> = cat.fields.iter().map(|f| f.name.clone()).collect();
            (cat.otlp_event_name.clone(), fields)
        })
        .collect()
}

/// A parsed, server-side-filtered telemetry record ready for ClickHouse insert.
#[derive(Debug, Clone)]
pub struct FilteredRecord {
    pub install_id: String,
    pub event_name: String,
    pub ts_ms: i64,
    /// Only allowlisted fields survive.
    pub attrs: HashMap<String, serde_json::Value>,
}

/// Apply server-side allowlist to a raw attribute map.
/// Fields not in the allowlist are silently dropped.
/// Returns `None` if the event_name is not in the taxonomy (unknown category → discard).
pub fn filter_record(
    install_id: &str,
    event_name: &str,
    ts_ms: i64,
    raw_attrs: HashMap<String, serde_json::Value>,
    allowlist: &HashMap<String, HashSet<String>>,
) -> Option<FilteredRecord> {
    let allowed = allowlist.get(event_name)?;
    let attrs: HashMap<String, serde_json::Value> = raw_attrs
        .into_iter()
        .filter(|(k, _)| allowed.contains(k))
        .collect();
    Some(FilteredRecord {
        install_id: install_id.to_string(),
        event_name: event_name.to_string(),
        ts_ms,
        attrs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::load_taxonomy;
    use serde_json::Value;
    use std::collections::HashMap;

    fn taxonomy_allowlist() -> HashMap<String, HashSet<String>> {
        let t = load_taxonomy().expect("taxonomy");
        build_allowlist(&t)
    }

    #[test]
    fn known_category_survives_filter() {
        let al = taxonomy_allowlist();
        let mut raw = HashMap::new();
        raw.insert("verb".to_string(), Value::String("build".to_string()));
        raw.insert(
            "exit_class".to_string(),
            Value::String("success".to_string()),
        );
        raw.insert(
            "duration_bucket".to_string(),
            Value::String("lt1s".to_string()),
        );
        let rec = filter_record("install123", "vox.command", 0, raw, &al);
        assert!(rec.is_some(), "known category must survive");
        let r = rec.unwrap();
        assert_eq!(r.attrs.get("verb").unwrap(), "build");
    }

    #[test]
    fn unknown_category_is_discarded() {
        let al = taxonomy_allowlist();
        let raw = HashMap::new();
        let rec = filter_record("install123", "vox.unknown_evil_category", 0, raw, &al);
        assert!(rec.is_none(), "unknown category must be discarded");
    }

    #[test]
    fn unlisted_field_is_dropped() {
        let al = taxonomy_allowlist();
        let mut raw = HashMap::new();
        raw.insert("verb".to_string(), Value::String("build".to_string()));
        // Inject a field that is not in the taxonomy for this category.
        raw.insert(
            "user_email".to_string(),
            Value::String("secret@example.com".to_string()),
        );
        raw.insert(
            "raw_path".to_string(),
            Value::String("/home/secret/project".to_string()),
        );
        let rec = filter_record("install123", "vox.command", 0, raw, &al).unwrap();
        assert!(
            !rec.attrs.contains_key("user_email"),
            "unlisted 'user_email' must be dropped"
        );
        assert!(
            !rec.attrs.contains_key("raw_path"),
            "unlisted 'raw_path' must be dropped"
        );
        assert!(
            rec.attrs.contains_key("verb"),
            "allowlisted 'verb' must survive"
        );
    }

    #[test]
    fn all_taxonomy_categories_have_allowlist_entry() {
        let t = load_taxonomy().expect("taxonomy");
        let al = build_allowlist(&t);
        for cat in &t.categories {
            assert!(
                al.contains_key(&cat.otlp_event_name),
                "allowlist missing category '{}'",
                cat.otlp_event_name
            );
        }
    }
}
