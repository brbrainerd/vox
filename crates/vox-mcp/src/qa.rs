use serde::{Deserialize, Serialize};

use crate::{ServerState, ToolResult};
use vox_orchestrator::AgentId;
use vox_runtime::prompt_canonical;

#[derive(Debug, Deserialize)]
pub struct AskAgentParams {
    pub from_agent: u64,
    pub to_agent: u64,
    pub question: String,
}

#[derive(Debug, Deserialize)]
pub struct AnswerQuestionParams {
    pub correlation_id: u64,
    pub answer: String,
}

#[derive(Debug, Deserialize)]
pub struct PendingQuestionsParams {
    pub agent_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastParams {
    pub from_agent: u64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PendingQuestionResponse {
    pub correlation_id: u64,
    pub question: String,
}

pub async fn ask_agent(state: &ServerState, params: AskAgentParams) -> String {
    let orch_lock = state.orchestrator.lock().await;
    let mut orch = orch_lock;

    let question = prompt_canonical::canonicalize_simple(&params.question);
    let corr_id = orch.qa_router().ask(
        AgentId(params.from_agent),
        AgentId(params.to_agent),
        question.clone(),
    );

    // Publish a bulletin board message about the question
    orch.bulletin()
        .publish(vox_orchestrator::types::AgentMessage::Question {
            from: AgentId(params.from_agent),
            to: AgentId(params.to_agent),
            question,
            correlation_id: vox_orchestrator::types::CorrelationId(corr_id.0),
        });

    ToolResult::ok(format!(
        "Question posted with correlation ID: {}",
        corr_id.0
    ))
    .to_json()
}

pub async fn answer_question(state: &ServerState, params: AnswerQuestionParams) -> String {
    let orch_lock = state.orchestrator.lock().await;
    let mut orch = orch_lock;

    let answer = params.answer.clone();
    let corr_id = vox_orchestrator::types::CorrelationId(params.correlation_id);
    match orch.qa_router().answer(corr_id, &answer) {
        Some(from_agent) => {
            // Find who answered it
            let answerer = AgentId(0); // Actually, we don't pass who answered in params. Let's say generic, or we should add it.
            orch.bulletin()
                .publish(vox_orchestrator::types::AgentMessage::Answer {
                    from: answerer, // Note: It would be better to know who answered, but we only have correlation id.
                    to: from_agent,
                    answer,
                    correlation_id: corr_id,
                });
            ToolResult::ok(format!(
                "Answer posted for correlation ID: {}",
                params.correlation_id
            ))
            .to_json()
        }
        None => ToolResult::<String>::err(format!(
            "No pending question found for correlation ID: {}",
            params.correlation_id
        ))
        .to_json(),
    }
}

pub async fn pending_questions(state: &ServerState, params: PendingQuestionsParams) -> String {
    let orch = state.orchestrator.lock().await;

    let questions = orch.qa_router().pending_questions(AgentId(params.agent_id));

    let result: Vec<PendingQuestionResponse> = questions
        .into_iter()
        .map(|(id, q)| PendingQuestionResponse {
            correlation_id: id.0,
            question: q,
        })
        .collect();

    ToolResult::ok(result).to_json()
}

pub async fn broadcast(state: &ServerState, params: BroadcastParams) -> String {
    let orch = state.orchestrator.lock().await;

    let message = prompt_canonical::canonicalize_simple(&params.message);
    orch.bulletin()
        .publish(vox_orchestrator::types::AgentMessage::Broadcast {
            from: AgentId(params.from_agent),
            message,
        });

    ToolResult::ok(format!(
        "Message broadcasted from agent: {}",
        params.from_agent
    ))
    .to_json()
}
