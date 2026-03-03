//! Schema definitions for shared agent stats and usage tracking.

use vox_db::schema_digest::{SchemaDigest, TableInfo, FieldInfo, CollectionInfo};

/// Returns the standard schema digest for Vox Orchestrator shared stats.
pub fn orchestrator_schema() -> SchemaDigest {
    SchemaDigest {
        tables: vec![
            TableInfo {
                name: "provider_usage".to_string(),
                fields: vec![
                    FieldInfo { name: "user_id".to_string(), type_str: "str".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "provider".to_string(), type_str: "str".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "model".to_string(), type_str: "str".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "date".to_string(), type_str: "str".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "calls".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "tokens_in".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "tokens_out".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "cost_usd".to_string(), type_str: "float".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "is_rate_limited".to_string(), type_str: "bool".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "last_429".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                ],
                description: Some("Tracks daily LLM token usage per provider/model".to_string()),
                example_insert: "".to_string(),
                example_query: "".to_string(),
                is_public: false,
                auth_provider: None,
                sample_data: vec![],
            },
            TableInfo {
                name: "agent_budgets".to_string(),
                fields: vec![
                    FieldInfo { name: "agent_id".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "max_tokens".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "max_cost_usd".to_string(), type_str: "float".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "tokens_used".to_string(), type_str: "int".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "cost_used".to_string(), type_str: "float".to_string(), is_optional: false, references_table: None },
                    FieldInfo { name: "last_rollover".to_string(), type_str: "str".to_string(), is_optional: false, references_table: None },
                ],
                description: Some("Persistent budgets for agents".to_string()),
                example_insert: "".to_string(),
                example_query: "".to_string(),
                is_public: false,
                auth_provider: None,
                sample_data: vec![],
            },
        ],
        collections: vec![
            CollectionInfo {
                name: "handoff_payloads".to_string(),
                fields: vec![],
                description: Some("Schemaless storage for agent handoff documents".to_string()),
                is_public: false,
                sample_data: vec![],
            },
        ],
        relationships: vec![],
        indexes: vec![],
        queries: vec![],
        mutations: vec![],
        actions: vec![],
        summary: "Vox Orchestrator Core Schema".to_string(),
    }
}
