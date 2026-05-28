use crate::types::AgentId;

impl crate::orchestrator::Orchestrator {
    /// Record a single AI model call: emits [`crate::events::AgentEventKind::CostIncurred`],
    /// updates the in-memory budget, and appends an oplog entry — all in one atomic call.
    pub async fn record_ai_usage(
        &self,
        agent_id: AgentId,
        provider: impl Into<String> + Clone,
        model: impl Into<String> + Clone,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
        header_cost_usd: Option<f64>,
        pricing_source: Option<crate::models::spec::PricingSource>,
    ) {
        let provider_str: String = provider.into();
        let model_str: String = model.into();

        let mut breakeven_crossed = false;
        if cost_usd == 0.0 || provider_str == "ollama" || provider_str == "mens" {
            let budget = crate::sync_lock::rw_read(&*self.budget_manager);
            let prev = budget.local_inference_tokens();
            let total_tokens = (input_tokens + output_tokens) as u64;
            budget.record_local_inference_tokens(total_tokens);
            let current = prev + total_tokens;
            let threshold = crate::sync_lock::rw_read(&*self.config).local_breakeven_tokens;
            if prev <= threshold && current > threshold {
                breakeven_crossed = true;
            }
        }

        let idle_ms = crate::types::now_unix_ms().saturating_sub(self.last_activity_ms());
        let mut temporal_context = serde_json::json!({
            "idle_secs": idle_ms / 1000,
            "date": chrono::Local::now().to_rfc3339(),
        });
        if breakeven_crossed {
            temporal_context["local_breakeven_crossed"] = serde_json::json!(true);
        }

        self.event_bus
            .emit(crate::events::AgentEventKind::CostIncurred {
                agent_id,
                provider: provider_str.clone(),
                model: model_str.clone(),
                input_tokens,
                output_tokens,
                cost_usd,
                temporal_context: Some(temporal_context),
            });

        {
            let budget = crate::sync_lock::rw_write(&*self.budget_manager);
            budget.record_usage(agent_id, (input_tokens + output_tokens) as usize);
            budget.record_cost(agent_id, cost_usd, pricing_source);
        }

        if let Some(db) = self.db() {
            let tracker = crate::usage::UsageTracker::new_ref(&db);
            let _ = tracker
                .record_call_detailed(
                    &provider_str,
                    &model_str,
                    input_tokens as u64,
                    output_tokens as u64,
                    header_cost_usd.unwrap_or(cost_usd),
                    None,
                    header_cost_usd,
                    Some(cost_usd),
                    header_cost_usd.or(Some(cost_usd)),
                    Some(if header_cost_usd.is_some() {
                        "openrouter_header"
                    } else {
                        "heuristic"
                    }),
                    None,
                    Some(&agent_id.to_string()),
                )
                .await;
        }

        let (op_id, entry_meta) = {
            let mut oplog = crate::sync_lock::rw_write(&*self.oplog);
            let op_id = oplog.record_ai_call(
                agent_id,
                &provider_str,
                &model_str,
                input_tokens,
                output_tokens,
                cost_usd,
            );
            let entry_meta = oplog.get(op_id).map(|entry| {
                (
                    entry.kind.clone(),
                    entry.description.clone(),
                    entry.predecessor_hash.clone(),
                    entry.model_id.clone(),
                    entry.change_id,
                    entry.timestamp_ms,
                )
            });
            (op_id, entry_meta)
        };
        self.persist_oplog_entry(agent_id, op_id, entry_meta).await;

        tracing::debug!(
            "AI usage recorded: agent={} {}/{} in={} out={} cost=${:.6}",
            agent_id,
            provider_str,
            model_str,
            input_tokens,
            output_tokens,
            cost_usd
        );
    }
}
