use vox_hir::{HirActivity, HirActor, HirStmt, HirType, HirWorkflow};

use crate::emit::{capitalize, emit_expr, emit_stmt, emit_type};

pub fn emit_activity(func: &HirActivity) -> String {
    let mut out = String::new();
    if func.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
    out.push_str(&format!("pub async fn {}(", func.name));
    for param in &func.params {
        out.push_str(&format!(
            "{}: {}, ",
            param.name,
            emit_type(
                param
                    .type_ann
                    .as_ref()
                    .unwrap_or(&HirType::Named("serde_json::Value".into()))
            )
        ));
    }
    out.push_str(") ");

    // Determine if the return type is a named struct before we emit the signature
    let returns_typed_struct = match &func.return_type {
        Some(HirType::Named(n)) => !matches!(n.as_str(), "str" | "String" | "int" | "float" | "bool"),
        _ => false,
    };

    if func.prompt.is_some() && returns_typed_struct {
        // Wrap in LlmResult<T> for proper error propagation
        let ret_type = emit_type(func.return_type.as_ref().unwrap());
        out.push_str(&format!("-> vox_runtime::LlmResult<{}> ", ret_type));
    } else if let Some(ret) = &func.return_type {
        out.push_str(&format!("-> {} ", emit_type(ret)));
    }
    out.push_str("{\n");

    if let Some(ref prompt) = func.prompt {
        out.push_str("    let mut prompt_content = r#\"");
        out.push_str(prompt);
        out.push_str("\"#.to_string();\n");
        for param in &func.params {
            out.push_str(&format!(
                "    prompt_content = prompt_content.replace(\"{{{{{}}}}}\", &crate::as_string(&{}));\n",
                param.name, param.name
            ));
        }

        // Extract all options from the `with { ... }` block
        let (model, temperature, max_tokens, response_format) =
            if let Some(vox_hir::HirExpr::ObjectLit(fields, _)) = &func.options {
                // Warn about unknown keys — prevents silent config drops
                const KNOWN_ACTIVITY_KEYS: &[&str] = &[
                    "model", "temperature", "max_tokens", "response_format",
                    "retries", "timeout", "initial_backoff", "activity_id",
                ];
                for (key, _) in fields {
                    if !KNOWN_ACTIVITY_KEYS.contains(&key.as_str()) {
                        eprintln!(
                            "⚠ vox-codegen: activity '{}' has unknown `with` key '{}' — this will be ignored. Known keys: {:?}",
                            func.name, key, KNOWN_ACTIVITY_KEYS
                        );
                    }
                }

                let model = fields
                    .iter()
                    .find(|(k, _)| k == "model")
                    .map(|(_, v)| emit_expr(v))
                    .unwrap_or_else(|| "\"openai/gpt-4o-mini\"".to_string());
                let temperature = fields
                    .iter()
                    .find(|(k, _)| k == "temperature")
                    .map(|(_, v)| emit_expr(v));
                let max_tokens = fields
                    .iter()
                    .find(|(k, _)| k == "max_tokens")
                    .map(|(_, v)| emit_expr(v));
                let response_format = fields
                    .iter()
                    .find(|(k, _)| k == "response_format")
                    .map(|(_, v)| emit_expr(v));
                (model, temperature, max_tokens, response_format)
            } else {
                ("\"openai/gpt-4o-mini\"".to_string(), None, None, None)
            };

        // Determine if the return type is a named struct (not String/Unit) for auto-parse
        // Note: returns_typed_struct is already computed above for the signature.

        out.push_str("    let options = vox_runtime::ActivityOptions::default();\n");
        out.push_str(&format!(
            "    let mut config = vox_runtime::llm::LlmConfig::openai({});\n",
            model
        ));
        if let Some(ref temp) = temperature {
            out.push_str(&format!(
                "    config.temperature = Some({} as f32);\n",
                temp
            ));
        }
        if let Some(ref mt) = max_tokens {
            out.push_str(&format!(
                "    config.max_tokens = Some({} as u64);\n",
                mt
            ));
        }
        if let Some(ref rf) = response_format {
            out.push_str(&format!(
                "    config.response_format = Some(serde_json::json!({}));\n",
                rf
            ));
        } else if returns_typed_struct {
            // Auto-enable JSON mode when expecting structured output
            out.push_str(
                "    config.response_format = Some(serde_json::json!({\"type\": \"json_object\"}));\n",
            );
        }

        out.push_str("    let messages = vec![vox_runtime::llm::ChatMessage { role: \"user\".to_string(), content: prompt_content }];\n");
        out.push_str("    match vox_runtime::llm::llm_chat(&options, messages, config).await {\n");

        if returns_typed_struct {
            let ret_type = emit_type(func.return_type.as_ref().unwrap());
            out.push_str(&format!(
                "        vox_runtime::ActivityResult::Ok(Ok(res)) => vox_runtime::LlmResult::<{}>::parse_from(&res.content),\n",
                ret_type
            ));
            out.push_str(
                "        vox_runtime::ActivityResult::Ok(Err(e)) => vox_runtime::LlmResult::Err(vox_runtime::LlmError::ApiError(e.to_string())),\n",
            );
            out.push_str(
                "        _ => vox_runtime::LlmResult::Err(vox_runtime::LlmError::ActivityFailed),\n",
            );
        } else {
            out.push_str("        vox_runtime::ActivityResult::Ok(Ok(res)) => res.content,\n");
            out.push_str(
                "        vox_runtime::ActivityResult::Ok(Err(e)) => format!(\"LLM Error: {}\", e),\n",
            );
            out.push_str("        _ => \"LLM Activity failed\".to_string(),\n");
        }
        out.push_str("    }\n");
    } else {
        for stmt in &func.body {
            out.push_str(&emit_stmt(stmt, 1, false, false));
        }
    }

    out.push_str("}\n\n");
    out
}

pub fn emit_workflow(wf: &HirWorkflow) -> String {
    let mut out = String::new();
    if wf.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
    out.push_str(&format!("pub async fn {}(", wf.name));
    for param in &wf.params {
        out.push_str(&format!(
            "{}: {}, ",
            param.name,
            emit_type(
                param
                    .type_ann
                    .as_ref()
                    .unwrap_or(&HirType::Named("serde_json::Value".into()))
            )
        ));
    }
    out.push_str(") ");
    if let Some(ret) = &wf.return_type {
        out.push_str(&format!("-> {} ", emit_type(ret)));
    }
    out.push_str("{\n");
    for stmt in &wf.body {
        out.push_str(&emit_stmt(stmt, 1, false, false));
    }
    out.push_str("}\n\n");
    out
}

pub fn emit_actor(actor: &HirActor) -> String {
    let mut out = String::new();
    if actor.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
    let msg_enum = format!("{}Message", actor.name);

    // Actor Message Enum
    out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
    out.push_str(&format!("pub enum {} {{\n", msg_enum));
    for handler in &actor.handlers {
        out.push_str(&format!("    {} {{ ", capitalize(&handler.event_name)));
        for param in &handler.params {
            out.push_str(&format!(
                "{}: {}, ",
                param.name,
                emit_type(
                    param
                        .type_ann
                        .as_ref()
                        .unwrap_or(&HirType::Named("serde_json::Value".into()))
                )
            ));
        }
        out.push_str("},\n");
    }
    out.push_str("}\n\n");

    // Actor Logic
    out.push_str(&format!("pub struct {};\n", actor.name));
    out.push_str(&format!("impl {} {{\n", actor.name));
    out.push_str("    pub async fn run(mut ctx: ProcessContext) {\n");
    out.push_str("        while let Some(envelope) = ctx.receive().await {\n");
    out.push_str("            match envelope {\n");
    out.push_str("                vox_runtime::Envelope::Request(req) => {\n");
    out.push_str(
        "                    if let vox_runtime::MessagePayload::Json(json_str) = &req.payload {\n",
    );
    out.push_str(&format!(
        "                        if let Ok(actor_msg) = serde_json::from_str::<{}>(&&json_str) {{\n",
        msg_enum
    ));
    out.push_str("                            let reply_str = match actor_msg {\n");

    for handler in &actor.handlers {
        out.push_str(&format!(
            "                                {}::{} {{ ",
            msg_enum,
            capitalize(&handler.event_name)
        ));
        for param in &handler.params {
            out.push_str(&format!("{}, ", param.name));
        }
        out.push_str("} => {\n");
        if handler.body.is_empty() {
            out.push_str("                                    String::new()\n");
        } else {
            for stmt in &handler.body[..handler.body.len().saturating_sub(1)] {
                out.push_str(&emit_stmt(stmt, 10, false, true));
            }
            if let Some(last) = handler.body.last() {
                match last {
                    HirStmt::Return {
                        value: Some(val), ..
                    } => {
                        let val_str = emit_expr(val);
                        out.push_str(&format!("                                    serde_json::to_string(&({})).unwrap_or_default()\n", val_str));
                    }
                    HirStmt::Expr { expr, .. } => {
                        let val_str = emit_expr(expr);
                        out.push_str(&format!("                                    serde_json::to_string(&({})).unwrap_or_default()\n", val_str));
                    }
                    _ => {
                        out.push_str(&emit_stmt(last, 10, false, true));
                        out.push_str("                                    String::new()\n");
                    }
                }
            }
        }
        out.push_str("                                }\n");
    }

    out.push_str("                            };\n");
    out.push_str("                            ProcessContext::reply(req, reply_str);\n");
    out.push_str("                        }\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                vox_runtime::Envelope::Message(msg) => {\n");
    out.push_str("                    // Fire-and-forget: process but don't reply\n");
    out.push_str(
        "                    if let vox_runtime::MessagePayload::Json(json_str) = msg.payload {\n",
    );
    out.push_str(&format!(
        "                        if let Ok(actor_msg) = serde_json::from_str::<{}>(&&json_str) {{\n",
        msg_enum
    ));
    out.push_str("                            let _ = actor_msg; // processed\n");
    out.push_str("                        }\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                _ => {}\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // Typed Handle
    out.push_str(&format!(
        "#[derive(Clone)]\npub struct {}Handle {{\n",
        actor.name
    ));
    out.push_str("    handle: vox_runtime::ProcessHandle,\n");
    out.push_str("}\n");
    out.push_str(&format!("impl {}Handle {{\n", actor.name));
    out.push_str(
        "    pub fn new(handle: vox_runtime::ProcessHandle) -> Self { Self { handle } }\n",
    );
    out.push_str("    pub fn spawn() -> Self {\n");
    out.push_str(&format!(
        "        let handle = vox_runtime::spawn_process({}::run, None, None);\n",
        actor.name
    ));
    out.push_str("        Self::new(handle)\n");
    out.push_str("    }\n");

    for handler in &actor.handlers {
        out.push_str(&format!("    pub async fn {}(&self, ", handler.event_name));
        for param in &handler.params {
            out.push_str(&format!(
                "{}: {}, ",
                param.name,
                emit_type(
                    param
                        .type_ann
                        .as_ref()
                        .unwrap_or(&HirType::Named("serde_json::Value".into()))
                )
            ));
        }
        out.push_str(") -> String {\n");
        out.push_str(&format!(
            "        let msg = {}::{} {{ ",
            msg_enum,
            capitalize(&handler.event_name)
        ));
        for param in &handler.params {
            out.push_str(&format!("{}, ", param.name));
        }
        out.push_str("};\n");
        out.push_str("        let payload = vox_runtime::MessagePayload::Json(serde_json::to_string(&msg).unwrap_or_else(|e| format!(\"serialize error: {}\", e)));\n");
        out.push_str("        self.handle.call(payload, std::time::Duration::from_secs(5)).await.unwrap_or_else(|e| format!(\"Actor error: {}\", e))\n");
        out.push_str("    }\n");
    }
    out.push_str("}\n\n");

    out
}
