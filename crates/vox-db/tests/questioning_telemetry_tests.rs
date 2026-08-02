use vox_db::{
    A2aClarificationMessageParams, DbConfig, QuestionEventParams, QuestionSessionCreateParams,
    VoxDb,
};
// Only used by the quarantine-gated tests below (question_option_outcomes);
// unused when the feature is off.
#[cfg(feature = "quarantine")]
use vox_db::{
    QuestionOptionOutcomeParams, QuestionOptionParams, QuestionStopEventParams,
    QuestioningResearchArtifact,
};

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// question_option_outcomes is quarantined (DORMANT, no confirmed caller
// outside vox-db even via wrapper-call detection — Task 4, VoxDB audit
// condensation plan) — off by default, see
// crates/vox-db/src/schema/domains/quarantine.rs.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn question_tables_round_trip() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let ts = now_ms();
    let session_id = db
        .create_question_session(QuestionSessionCreateParams {
            session_id: "mcp:test",
            repository_id: "repo-1",
            task_id: Some("task-1"),
            policy_version: "v1",
            started_at_ms: ts,
        })
        .await
        .expect("create session");

    let event_id = db
        .insert_question_event(QuestionEventParams {
            question_session_id: session_id,
            question_id: "q1",
            turn_index: 0,
            actor: "assistant",
            question_kind: "multiple_choice",
            prompt: "Which output format do you want?",
            expected_information_gain_bits: 0.18,
            expected_user_cost: 0.22,
            utility_bits_per_cost: 0.81,
            answer_text: Some("JSON"),
            answer_type: Some("multiple_choice"),
            answered_at_ms: Some(ts + 10),
            created_at_ms: ts,
        })
        .await
        .expect("insert event");

    db.upsert_question_option(QuestionOptionParams {
        question_event_id: event_id,
        option_id: "a",
        label: "JSON",
        prior_probability: Some(0.5),
        posterior_probability: Some(0.8),
        is_other: false,
    })
    .await
    .expect("upsert option A");
    db.upsert_question_option(QuestionOptionParams {
        question_event_id: event_id,
        option_id: "b",
        label: "Markdown",
        prior_probability: Some(0.5),
        posterior_probability: Some(0.2),
        is_other: false,
    })
    .await
    .expect("upsert option B");

    db.insert_question_option_outcome(QuestionOptionOutcomeParams {
        question_event_id: event_id,
        option_id: "a",
        selected: true,
        diagnostic_weight: 0.8,
        information_contribution_bits: 0.10,
        created_at_ms: ts + 20,
    })
    .await
    .expect("outcome selected");
    db.insert_question_option_outcome(QuestionOptionOutcomeParams {
        question_event_id: event_id,
        option_id: "b",
        selected: false,
        diagnostic_weight: 0.4,
        information_contribution_bits: 0.05,
        created_at_ms: ts + 20,
    })
    .await
    .expect("outcome unselected");

    db.insert_question_stop_event(QuestionStopEventParams {
        question_session_id: session_id,
        stop_reason: "confidence_sufficient",
        confidence_at_stop: Some(0.83),
        marginal_gain_bits: Some(0.03),
        expected_user_cost: Some(0.20),
        turn_index: Some(1),
        created_at_ms: ts + 30,
    })
    .await
    .expect("stop");

    db.close_question_session(session_id, "resolved", ts + 40)
        .await
        .expect("close");

    let sessions = db
        .list_question_sessions("mcp:test", 10)
        .await
        .expect("list sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].resolution_status, "resolved");

    let events = db
        .list_question_events(session_id)
        .await
        .expect("list events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].question_kind, "multiple_choice");

    let options = db
        .list_question_options(event_id)
        .await
        .expect("list options");
    assert_eq!(options.len(), 2);

    let outcomes = db
        .list_question_option_outcomes(event_id)
        .await
        .expect("list outcomes");
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().any(|o| o.selected));

    let stops = db
        .list_question_stop_events(session_id)
        .await
        .expect("list stops");
    assert_eq!(stops.len(), 1);
}

#[tokio::test]
async fn a2a_clarification_payload_is_sent_and_pollable() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    db.send_a2a_clarification_message(A2aClarificationMessageParams {
        message_uuid: "msg-clarify-1",
        sender_agent: "agent-a",
        receiver_agent: "agent-b",
        msg_type: "clarification_request",
        repository_id: "repo-1",
        thread_id: Some("thread-1"),
        priority: 10,
        clarification_intent: "resolve_scope",
        hypothesis_set_id: "hyp-1",
        question_kind: Some("multiple_choice"),
        expected_information_gain_bits: Some(0.15),
        expected_user_cost: Some(0.20),
        requested_evidence_dimensions_json: Some(r#"["scope","risk"]"#),
        urgency: Some("normal"),
        stop_policy_json: Some(r#"{"max_turns":3}"#),
    })
    .await
    .expect("send");

    let inbox = db.poll_a2a_inbox("agent-b", "repo-1").await.expect("poll");
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].msg_type, "clarification_request");
    assert!(
        inbox[0]
            .payload
            .contains("\"clarification_intent\":\"resolve_scope\""),
        "payload mismatch: {}",
        inbox[0].payload
    );
}

// question_option_outcomes quarantined, see above.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn dual_write_and_kpi_rollup_work() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

    let (_digest, _doc_id) = db
        .persist_questioning_research_artifact_dual_write(QuestioningResearchArtifact {
            publication_id: "q-ssot",
            source_ref: Some("docs/src/reference/information-theoretic-questioning.md"),
            title: "Questioning SSOT",
            author: "vox",
            abstract_text: Some("summary"),
            body_markdown: "# Q\n\nBody",
            citations_json: None,
            metadata_json: None,
            state: "draft",
        })
        .await
        .expect("dual write");

    let sid = db
        .create_question_session(QuestionSessionCreateParams {
            session_id: "mcp:repo-1",
            repository_id: "repo-1",
            task_id: None,
            policy_version: "v1",
            started_at_ms: now_ms(),
        })
        .await
        .expect("session");
    let qid = db
        .insert_question_event(QuestionEventParams {
            question_session_id: sid,
            question_id: "q-kpi",
            turn_index: 0,
            actor: "assistant",
            question_kind: "entry",
            prompt: "Provide repo id",
            expected_information_gain_bits: 0.12,
            expected_user_cost: 0.15,
            utility_bits_per_cost: 0.8,
            answer_text: Some("repo-1"),
            answer_type: Some("entry"),
            answered_at_ms: Some(now_ms()),
            created_at_ms: now_ms(),
        })
        .await
        .expect("event");
    db.insert_question_option_outcome(QuestionOptionOutcomeParams {
        question_event_id: qid,
        option_id: "repo-1",
        selected: true,
        diagnostic_weight: 1.0,
        information_contribution_bits: 0.12,
        created_at_ms: now_ms(),
    })
    .await
    .expect("outcome");

    let kpis = db
        .aggregate_questioning_kpis(Some("mcp:repo-1"), 100)
        .await
        .expect("kpi");
    assert!(kpis.sample_size >= 1);
    assert!(kpis.mean_expected_information_gain_bits > 0.0);
}

#[tokio::test]
async fn open_session_turn_and_user_answer_helpers() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let ts = now_ms();
    let sid = db
        .create_question_session(QuestionSessionCreateParams {
            session_id: "mcp-s1",
            repository_id: "repo-x",
            task_id: None,
            policy_version: "v1",
            started_at_ms: ts,
        })
        .await
        .expect("session");

    assert!(
        db.find_open_question_session_for_repo("mcp-s1", "repo-x")
            .await
            .expect("open")
            .is_some()
    );

    assert_eq!(db.next_question_event_turn_index(sid).await.expect("ti"), 0);

    let _ = db
        .insert_question_event(QuestionEventParams {
            question_session_id: sid,
            question_id: "q-a",
            turn_index: 0,
            actor: "assistant",
            question_kind: "open_ended",
            prompt: "What branch?",
            expected_information_gain_bits: 0.1,
            expected_user_cost: 0.2,
            utility_bits_per_cost: 0.5,
            answer_text: None,
            answer_type: None,
            answered_at_ms: None,
            created_at_ms: ts,
        })
        .await
        .expect("ev1");

    assert_eq!(
        db.count_assistant_questions_in_open_session("mcp-s1", "repo-x")
            .await
            .expect("cnt"),
        1
    );
    assert_eq!(
        db.next_question_event_turn_index(sid).await.expect("ti2"),
        1
    );
    assert!(
        db.has_pending_clarification_for_mcp_session("mcp-s1", "repo-x")
            .await
            .expect("pend")
    );

    let resolved = db
        .record_questioning_user_answer(sid, None, "main", "free_text", None, 0.12, ts + 1)
        .await
        .expect("answer");
    assert_eq!(resolved, "q-a");

    db.merge_question_session_belief_answer(sid, &resolved, "main", ts + 1, None)
        .await
        .expect("belief merge");

    let open_row = db
        .find_open_question_session_for_repo("mcp-s1", "repo-x")
        .await
        .expect("open2")
        .expect("session row");
    let belief = open_row.belief_state_json.expect("belief json");
    assert!(belief.contains("\"answer_text\":\"main\""));

    assert!(
        !db.has_pending_clarification_for_mcp_session("mcp-s1", "repo-x")
            .await
            .expect("pend2")
    );
}

// question_option_outcomes quarantined, see above.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn pending_json_and_mc_belief_posterior_update() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let ts = now_ms();
    let sid = db
        .create_question_session(QuestionSessionCreateParams {
            session_id: "mcp-mc",
            repository_id: "repo-r1",
            task_id: None,
            policy_version: "v1",
            started_at_ms: ts,
        })
        .await
        .expect("session");

    let qev = db
        .insert_question_event(QuestionEventParams {
            question_session_id: sid,
            question_id: "q-mc",
            turn_index: 0,
            actor: "assistant",
            question_kind: "multiple_choice",
            prompt: "Format?",
            expected_information_gain_bits: 1.0,
            expected_user_cost: 0.1,
            utility_bits_per_cost: 1.0,
            answer_text: None,
            answer_type: None,
            answered_at_ms: None,
            created_at_ms: ts,
        })
        .await
        .expect("ev");

    db.upsert_question_option(QuestionOptionParams {
        question_event_id: qev,
        option_id: "a",
        label: "JSON",
        prior_probability: Some(0.5),
        posterior_probability: Some(0.5),
        is_other: false,
    })
    .await
    .expect("opt a");
    db.upsert_question_option(QuestionOptionParams {
        question_event_id: qev,
        option_id: "b",
        label: "YAML",
        prior_probability: Some(0.5),
        posterior_probability: Some(0.5),
        is_other: false,
    })
    .await
    .expect("opt b");

    let pending = db
        .pending_clarifications_json_for_repo("mcp-mc", "repo-r1")
        .await
        .expect("pending json");
    assert_eq!(pending["open"], serde_json::json!(true));
    let p = pending["pending"].as_array().expect("pending arr");
    assert_eq!(p.len(), 1);
    assert_eq!(p[0]["options"].as_array().expect("opts").len(), 2);

    let answered_at = ts + 5;
    let resolved = db
        .record_questioning_user_answer(
            sid,
            Some("q-mc"),
            "JSON",
            "multiple_choice",
            Some("a"),
            0.2,
            answered_at,
        )
        .await
        .expect("answer");
    assert_eq!(resolved, "q-mc");

    db.merge_question_session_belief_answer(sid, &resolved, "JSON", answered_at, Some("a"))
        .await
        .expect("belief merge mc");

    let row = db
        .find_open_question_session_for_repo("mcp-mc", "repo-r1")
        .await
        .expect("open")
        .expect("sess");
    let belief_str = row.belief_state_json.expect("belief");
    let belief: serde_json::Value = serde_json::from_str(&belief_str).expect("parse belief");
    let p_a = belief["hypothesis_mass"]["by_question"]["q-mc"]["a"]
        .as_f64()
        .expect("posterior a");
    let p_b = belief["hypothesis_mass"]["by_question"]["q-mc"]["b"]
        .as_f64()
        .expect("posterior b");
    assert!((p_a - 2.0 / 3.0).abs() < 1e-6, "a={p_a}");
    assert!((p_b - 1.0 / 3.0).abs() < 1e-6, "b={p_b}");

    let opts = db.list_question_options(qev).await.expect("opts2");
    let post_a = opts
        .iter()
        .find(|o| o.option_id == "a")
        .unwrap()
        .posterior_probability;
    let post_b = opts
        .iter()
        .find(|o| o.option_id == "b")
        .unwrap()
        .posterior_probability;
    assert!(post_a.is_some());
    assert!(post_b.is_some());
    assert!((post_a.unwrap() - 2.0 / 3.0).abs() < 1e-6);
    assert!((post_b.unwrap() - 1.0 / 3.0).abs() < 1e-6);

    let pending2 = db
        .pending_clarifications_json_for_repo("mcp-mc", "repo-r1")
        .await
        .expect("pending after");
    assert_eq!(pending2["pending"].as_array().expect("arr").len(), 0);
}
