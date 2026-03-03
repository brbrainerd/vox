//! Data Flow Tracer — static analysis for database operation mapping.
//!
//! Tracks which mutations write which tables and which queries read
//! which tables, enabling LLMs to understand data flow without reading
//! source code. Exposed via MCP tools for AI context.

use crate::schema_digest::SchemaDigest;
use serde::{Deserialize, Serialize};

/// Complete data flow map for a Vox module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowMap {
    /// Query → tables it reads from.
    pub query_reads: Vec<DataFlowEntry>,
    /// Mutation → tables it writes to.
    pub mutation_writes: Vec<DataFlowEntry>,
    /// Action → mutations/queries it calls (when detectable).
    pub action_calls: Vec<DataFlowEntry>,
    /// Tables that are read but never written (potential stale data).
    pub read_only_tables: Vec<String>,
    /// Tables that are written but never read (potential dead data).
    pub write_only_tables: Vec<String>,
}

/// A single data flow entry mapping a function to its affected tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowEntry {
    pub function_name: String,
    pub tables: Vec<String>,
}

/// Build a data flow map from a schema digest.
///
/// Uses the `affected_tables` heuristic from the schema digest
/// to determine which functions interact with which tables.
pub fn build_data_flow(digest: &SchemaDigest) -> DataFlowMap {
    let query_reads: Vec<DataFlowEntry> = digest
        .queries
        .iter()
        .filter(|q| !q.affected_tables.is_empty())
        .map(|q| DataFlowEntry {
            function_name: q.name.clone(),
            tables: q.affected_tables.clone(),
        })
        .collect();

    let mutation_writes: Vec<DataFlowEntry> = digest
        .mutations
        .iter()
        .filter(|m| !m.affected_tables.is_empty())
        .map(|m| DataFlowEntry {
            function_name: m.name.clone(),
            tables: m.affected_tables.clone(),
        })
        .collect();

    let action_calls: Vec<DataFlowEntry> = digest
        .actions
        .iter()
        .filter(|a| !a.affected_tables.is_empty())
        .map(|a| DataFlowEntry {
            function_name: a.name.clone(),
            tables: a.affected_tables.clone(),
        })
        .collect();

    // Find tables that are read but never written
    let all_table_names: Vec<&str> = digest.tables.iter().map(|t| t.name.as_str()).collect();
    let written_tables: std::collections::HashSet<&str> = mutation_writes
        .iter()
        .flat_map(|m| m.tables.iter().map(|s| s.as_str()))
        .collect();
    let read_tables: std::collections::HashSet<&str> = query_reads
        .iter()
        .flat_map(|q| q.tables.iter().map(|s| s.as_str()))
        .collect();

    let read_only_tables: Vec<String> = all_table_names
        .iter()
        .filter(|t| read_tables.contains(**t) && !written_tables.contains(**t))
        .map(|t| t.to_string())
        .collect();

    let write_only_tables: Vec<String> = all_table_names
        .iter()
        .filter(|t| written_tables.contains(**t) && !read_tables.contains(**t))
        .map(|t| t.to_string())
        .collect();

    DataFlowMap {
        query_reads,
        mutation_writes,
        action_calls,
        read_only_tables,
        write_only_tables,
    }
}

/// Format the data flow map for LLM context.
pub fn format_data_flow(flow: &DataFlowMap) -> String {
    let mut out = String::new();
    out.push_str("### Data Flow\n\n");

    if !flow.query_reads.is_empty() {
        out.push_str("**Reads:**\n");
        for entry in &flow.query_reads {
            out.push_str(&format!(
                "- `{}` reads from: {}\n",
                entry.function_name,
                entry.tables.join(", ")
            ));
        }
    }

    if !flow.mutation_writes.is_empty() {
        out.push_str("**Writes:**\n");
        for entry in &flow.mutation_writes {
            out.push_str(&format!(
                "- `{}` writes to: {}\n",
                entry.function_name,
                entry.tables.join(", ")
            ));
        }
    }

    if !flow.read_only_tables.is_empty() {
        out.push_str(&format!(
            "\n⚠ Read-only tables (no mutations): {}\n",
            flow.read_only_tables.join(", ")
        ));
    }

    if !flow.write_only_tables.is_empty() {
        out.push_str(&format!(
            "\n⚠ Write-only tables (no queries): {}\n",
            flow.write_only_tables.join(", ")
        ));
    }

    out
}

/// Serialize the data flow map to JSON.
pub fn data_flow_to_json(flow: &DataFlowMap) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(flow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_digest::*;

    fn test_digest() -> SchemaDigest {
        SchemaDigest {
            tables: vec![
                TableInfo {
                    name: "Task".to_string(),
                    fields: vec![],
                    description: None,
                    example_insert: String::new(),
                    example_query: String::new(),
                    is_public: false,
                    auth_provider: None,
                    sample_data: Vec::new(),
                },
                TableInfo {
                    name: "User".to_string(),
                    fields: vec![],
                    description: None,
                    example_insert: String::new(),
                    example_query: String::new(),
                    is_public: false,
                    auth_provider: None,
                    sample_data: Vec::new(),
                },
            ],
            collections: Vec::new(),
            relationships: Vec::new(),
            indexes: Vec::new(),
            queries: vec![FunctionInfo {
                name: "list_tasks".to_string(),
                params: Vec::new(),
                return_type: Some("List[Task]".to_string()),
                affected_tables: vec!["Task".to_string()],
            }],
            mutations: vec![FunctionInfo {
                name: "add_task".to_string(),
                params: Vec::new(),
                return_type: None,
                affected_tables: vec!["Task".to_string()],
            }],
            actions: Vec::new(),
            summary: String::new(),
        }
    }

    #[test]
    fn test_data_flow_basic() {
        let digest = test_digest();
        let flow = build_data_flow(&digest);

        assert_eq!(flow.query_reads.len(), 1);
        assert_eq!(flow.query_reads[0].function_name, "list_tasks");
        assert_eq!(flow.mutation_writes.len(), 1);
        assert_eq!(flow.mutation_writes[0].function_name, "add_task");
    }

    #[test]
    fn test_read_only_detection() {
        let mut digest = test_digest();
        // User table is never written to
        digest.queries.push(FunctionInfo {
            name: "get_user".to_string(),
            params: Vec::new(),
            return_type: Some("User".to_string()),
            affected_tables: vec!["User".to_string()],
        });

        let flow = build_data_flow(&digest);
        assert!(flow.read_only_tables.contains(&"User".to_string()));
    }

    #[test]
    fn test_format_data_flow() {
        let digest = test_digest();
        let flow = build_data_flow(&digest);
        let formatted = format_data_flow(&flow);
        assert!(formatted.contains("list_tasks"));
        assert!(formatted.contains("add_task"));
    }
}
