use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::types::{AgentId, CorrelationId, CorrelationIdGenerator};

pub struct PendingQuestion {
    pub from: AgentId,
    pub to: AgentId,
    pub question: String,
    pub asked_at: Instant,
}

#[derive(Clone)]
pub struct QARouter {
    pending: Arc<RwLock<HashMap<CorrelationId, PendingQuestion>>>,
    correlator: Arc<CorrelationIdGenerator>,
}

impl QARouter {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            correlator: Arc::new(CorrelationIdGenerator::new()),
        }
    }

    pub fn ask(&self, from: AgentId, to: AgentId, question: impl Into<String>) -> CorrelationId {
        let corr_id = self.correlator.next();
        let q = PendingQuestion {
            from,
            to,
            question: question.into(),
            asked_at: Instant::now(),
        };
        self.pending.write().unwrap().insert(corr_id, q);
        corr_id
    }

    pub fn answer(&self, corr_id: CorrelationId, _answer: &str) -> Option<AgentId> {
        let q = self.pending.write().unwrap().remove(&corr_id)?;
        Some(q.from)
    }

    pub fn pending_questions(&self, to_agent: AgentId) -> Vec<(CorrelationId, String)> {
        self.pending
            .read()
            .unwrap()
            .iter()
            .filter(|(_, q)| q.to == to_agent)
            .map(|(k, q)| (*k, q.question.clone()))
            .collect()
    }
}

impl Default for QARouter {
    fn default() -> Self {
        Self::new()
    }
}
