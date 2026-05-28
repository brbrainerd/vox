//! `vox db` subcommand — inspect and manage the local VoxDB database.

mod local;
mod publication;
mod query_plans;

pub use local::*;
pub use publication::*;
pub use query_plans::explain;

pub use super::db_research::*;

#[cfg(test)]
mod tests {
    use super::publication::publication_item_from_manifest;
    use super::query_plans::{QueryPlanExplainRow, collect_query_fn_plans};
    use chrono::Utc;
    use vox_publisher::types::{SyndicationConfig, UnifiedNewsItem};

    fn sample_item() -> UnifiedNewsItem {
        UnifiedNewsItem {
            id: "x".to_string(),
            title: "t".to_string(),
            author: "a".to_string(),
            published_at: Utc::now(),
            tags: vec![],
            content_markdown: "body".to_string(),
            syndication: SyndicationConfig {
                twitter: serde_json::json!({
                    "short_text": "s",
                    "thread": false,
                }),
                rss: true,
                ..Default::default()
            },
            topic_pack: None,
        }
    }

    #[test]
    fn parse_channels_csv_normalizes() {
        let out = Some(vox_publisher::switching::parse_channels_csv(
            " twitter, reddit ,YOUTUBE ",
        ));
        assert_eq!(
            out,
            Some(vec![
                "twitter".to_string(),
                "reddit".to_string(),
                "youtube".to_string()
            ])
        );
    }

    #[test]
    fn filter_channels_keeps_only_allowed() {
        let item = sample_item();
        let allowed = vec!["twitter".to_string()];
        let mut out = item;
        vox_publisher::switching::apply_channel_allowlist(&mut out, allowed.as_slice());
        assert!(!out.syndication.rss);
        assert!(
            out.syndication
                .is_active(vox_publisher::types::SocialChannel::Twitter)
        );
    }

    #[test]
    #[ignore = "db manifest topic-pack hydration; owner: publisher sunset: 2026-12-31"]
    fn publication_item_from_manifest_hydrates_topic_pack() {
        let row = vox_db::PublicationManifestRow {
            publication_id: "p1".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "Title".to_string(),
            author: "Author".to_string(),
            abstract_text: None,
            body_markdown: "Body".to_string(),
            citations_json: None,
            metadata_json: Some(
                r#"{
                    "tags":["research_breakthrough"],
                    "topic_pack":"research_breakthrough",
                    "syndication":{"twitter":{"short_text":null,"thread":false},"rss":true}
                }"#
                .to_string(),
            ),
            revision_history_json: None,
            content_sha3_256: "digest".to_string(),
            state: "draft".to_string(),
            version: 1,
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        let item = publication_item_from_manifest(&row).expect("item");
        assert_eq!(item.topic_pack.as_deref(), Some("research_breakthrough"));
        assert!(
            !item
                .syndication
                .is_active(vox_publisher::types::SocialChannel::Twitter)
        );
    }

    #[test]
    fn collect_query_fn_plans_extracts_hir_db_query_plan_rows() {
        let src = r#"
@table type User { name: str active: bool }
@query fn q1() to int {
    return len(db.User.filter({ active: true }).limit(5))
}
"#;
        let tokens = vox_compiler::lexer::lex(src);
        let module = vox_compiler::parser::parse(tokens).expect("parse");
        let hir = vox_compiler::hir::lower_module(&module);
        let rows = collect_query_fn_plans(&hir, None);
        assert!(!rows.is_empty(), "expected at least one query plan row");
        assert!(rows.iter().any(|r| r.query_fn == "q1"));
        assert!(
            rows.iter()
                .any(|r| matches!(r.plan.op, vox_compiler::hir::HirDbTableOp::FilterRecord))
        );
    }

    #[test]
    fn collect_query_fn_plans_honors_query_name_filter() {
        let src = r#"
@table type User { name: str active: bool }
@query fn qa() to int { return len(db.User.all()) }
@query fn qb() to int { return len(db.User.filter({ active: true })) }
"#;
        let tokens = vox_compiler::lexer::lex(src);
        let module = vox_compiler::parser::parse(tokens).expect("parse");
        let hir = vox_compiler::hir::lower_module(&module);
        let rows = collect_query_fn_plans(&hir, Some("qb"));
        assert!(!rows.is_empty(), "expected rows for filtered query name");
        assert!(rows.iter().all(|r| r.query_fn == "qb"));
    }

    #[test]
    fn query_plan_explain_row_jsonl_line_round_trips() {
        use vox_compiler::hir::{HirDbPlanCapabilities, HirDbQueryPlan, HirDbTableOp};
        let row = QueryPlanExplainRow {
            query_fn: "q".into(),
            route_path: "/q/q".into(),
            plan: HirDbQueryPlan {
                table: "User".into(),
                op: HirDbTableOp::All,
                predicate: None,
                projection: None,
                order_by: None,
                has_limit: false,
                capabilities: HirDbPlanCapabilities::default(),
            },
        };
        let line = serde_json::to_string(&row).expect("serialize");
        assert!(!line.contains('\n'), "jsonl row must be a single line");
        let parsed: QueryPlanExplainRow = serde_json::from_str(&line).expect("deserialize");
        assert_eq!(parsed.query_fn, row.query_fn);
        assert_eq!(parsed.plan.table, row.plan.table);
    }
}
