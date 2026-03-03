//! Error Enrichment for VoxDB — LLM-first error messages.
//!
//! When a database operation fails, this module enriches the error with
//! schema context so AI models can self-correct without needing to re-read
//! the schema. This is a key differentiator over traditional databases
//! where error messages are opaque to LLMs.

use crate::schema_digest::{FieldInfo, SchemaDigest, TableInfo};

/// An enriched database error with schema context.
#[derive(Debug, Clone)]
pub struct EnrichedDbError {
    /// The original error message.
    pub original_message: String,
    /// A human/LLM-readable explanation with schema context.
    pub enriched_message: String,
    /// Suggestions for fixing the error.
    pub suggestions: Vec<String>,
    /// Relevant table info (if applicable).
    pub related_table: Option<TableInfo>,
}

/// Enrich a database error with schema context.
///
/// Takes a raw error message and the current schema digest, then produces
/// an enriched error that includes:
/// - Which table was involved
/// - What fields are available
/// - Fuzzy-matched suggestions for typos
/// - Example correct usage
pub fn enrich_error(raw_message: &str, digest: &SchemaDigest) -> EnrichedDbError {
    let msg_lower = raw_message.to_lowercase();

    // Try to detect which table is mentioned
    let related_table = digest
        .tables
        .iter()
        .find(|t| msg_lower.contains(&t.name.to_lowercase()))
        .cloned();

    let mut suggestions = Vec::new();
    let mut enriched = raw_message.to_string();

    // Check for field-not-found errors
    if msg_lower.contains("field") || msg_lower.contains("column") {
        if let Some(ref table) = related_table {
            let available = field_list_str(&table.fields);
            enriched = format!(
                "{}\n\nAvailable fields on '{}': {}",
                raw_message, table.name, available
            );

            // Try to fuzzy-match a misspelled field name
            let words: Vec<&str> = raw_message.split_whitespace().collect();
            for word in &words {
                let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
                if !clean.is_empty() {
                    for field in &table.fields {
                        let dist = levenshtein(clean, &field.name);
                        if dist > 0 && dist <= 2 {
                            suggestions.push(format!(
                                "Did you mean '{}' instead of '{}'?",
                                field.name, clean
                            ));
                        }
                    }
                }
            }
        }
    }

    // Check for table-not-found errors
    if msg_lower.contains("table") && msg_lower.contains("not found") {
        let all_tables: Vec<&str> = digest.tables.iter().map(|t| t.name.as_str()).collect();
        enriched = format!(
            "{}\n\nAvailable tables: {}",
            raw_message,
            all_tables.join(", ")
        );

        // Try to fuzzy-match a misspelled table name
        let words: Vec<&str> = raw_message.split_whitespace().collect();
        for word in &words {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            for table in &digest.tables {
                let dist = levenshtein(clean, &table.name);
                if dist > 0 && dist <= 2 {
                    suggestions.push(format!(
                        "Did you mean '{}' instead of '{}'?",
                        table.name, clean
                    ));
                }
            }
        }
    }

    // Check for type mismatch errors
    if msg_lower.contains("type")
        && (msg_lower.contains("mismatch") || msg_lower.contains("expected"))
    {
        if let Some(ref table) = related_table {
            let field_types: Vec<String> = table
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, f.type_str))
                .collect();
            suggestions.push(format!(
                "Field types for '{}': {}",
                table.name,
                field_types.join(", ")
            ));
        }
    }

    // Check for missing required field errors
    if msg_lower.contains("required") || msg_lower.contains("missing") {
        if let Some(ref table) = related_table {
            let required: Vec<&str> = table
                .fields
                .iter()
                .filter(|f| !f.is_optional)
                .map(|f| f.name.as_str())
                .collect();
            suggestions.push(format!(
                "Required fields for '{}': {}",
                table.name,
                required.join(", ")
            ));
            suggestions.push(format!("Example: {}", table.example_insert));
        }
    }

    EnrichedDbError {
        original_message: raw_message.to_string(),
        enriched_message: enriched,
        suggestions,
        related_table,
    }
}

/// Format an enriched error for display (suitable for LLM consumption).
pub fn format_enriched_error(err: &EnrichedDbError) -> String {
    let mut out = err.enriched_message.clone();
    if !err.suggestions.is_empty() {
        out.push_str("\n\nSuggestions:");
        for s in &err.suggestions {
            out.push_str(&format!("\n  • {}", s));
        }
    }
    out
}

// ── Private Helpers ─────────────────────────────────────

fn field_list_str(fields: &[FieldInfo]) -> String {
    fields
        .iter()
        .map(|f| {
            let opt = if f.is_optional { "?" } else { "" };
            format!("{}{}({})", f.name, opt, f.type_str)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Simple Levenshtein distance for typo detection.
fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();

    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_digest::*;

    fn test_digest() -> SchemaDigest {
        SchemaDigest {
            tables: vec![TableInfo {
                name: "Task".to_string(),
                fields: vec![
                    FieldInfo {
                        name: "title".to_string(),
                        type_str: "str".to_string(),
                        is_optional: false,
                        references_table: None,
                    },
                    FieldInfo {
                        name: "done".to_string(),
                        type_str: "bool".to_string(),
                        is_optional: false,
                        references_table: None,
                    },
                    FieldInfo {
                        name: "priority".to_string(),
                        type_str: "int".to_string(),
                        is_optional: false,
                        references_table: None,
                    },
                ],
                description: None,
                example_insert: "db.insert(Task, { title: \"example\", done: false, priority: 0 })"
                    .to_string(),
                example_query: "db.query(Task).collect()".to_string(),
                is_public: false,
                auth_provider: None,
                sample_data: Vec::new(),
            }],
            collections: Vec::new(),
            relationships: Vec::new(),
            indexes: Vec::new(),
            queries: Vec::new(),
            mutations: Vec::new(),
            actions: Vec::new(),
            summary: String::new(),
        }
    }

    #[test]
    fn test_field_typo_suggestion() {
        let digest = test_digest();
        let err = enrich_error("field 'titl' not found on Task", &digest);
        assert!(!err.suggestions.is_empty());
        assert!(err.suggestions.iter().any(|s| s.contains("title")));
    }

    #[test]
    fn test_field_list_in_error() {
        let digest = test_digest();
        let err = enrich_error("field 'xyz' not found on Task", &digest);
        assert!(err.enriched_message.contains("title"));
        assert!(err.enriched_message.contains("done"));
        assert!(err.enriched_message.contains("priority"));
    }

    #[test]
    fn test_missing_required_fields() {
        let digest = test_digest();
        let err = enrich_error("required field missing on Task", &digest);
        assert!(err.suggestions.iter().any(|s| s.contains("title")));
        assert!(err.suggestions.iter().any(|s| s.contains("db.insert")));
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein("title", "title"), 0);
        assert_eq!(levenshtein("titl", "title"), 1);
        assert_eq!(levenshtein("ttle", "title"), 1);
        assert_eq!(levenshtein("xyz", "title"), 5);
    }
}
