use vox_research_shim::research::{
    planner::{decompose_query_with_config, plan_to_json},
    types::{ResearchPlan, ResearchQuery, ResearchScope},
};

#[tokio::test]
async fn planner_without_available_model_falls_back_to_single_subquery() {
    let q = ResearchQuery {
        query: "test".into(),
        scope: ResearchScope::Both,
        max_sources: 3,
        persist_to_docs: false,
        verify_claims: false,
        site_scope: None,
    };
    let plan = decompose_query_with_config(&q, None, None, None, None, None)
        .await
        .expect("planner fallback returns Ok");
    assert_eq!(plan.original_query, "test");
    assert_eq!(plan.subqueries, vec!["test".to_string()]);
}

#[test]
fn plan_to_json_serializes() {
    let plan = ResearchPlan {
        original_query: "q".into(),
        subqueries: vec!["q".into()],
        scope: ResearchScope::Both,
        max_sources_per_subquery: 3,
    };
    let v = plan_to_json(&plan);
    assert!(v.is_object());
}
