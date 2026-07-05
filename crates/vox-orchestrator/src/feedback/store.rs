use crate::feedback::types::{
    FeedbackId, FeedbackKind, FeedbackRequest, FeedbackResolution, Surface,
};
use crate::types::{AgentId, TaskId};
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FeedbackStore {
    inner: Arc<RwLock<FeedbackStoreInner>>,
}

#[derive(Debug, Default)]
struct FeedbackStoreInner {
    seq: u64,
    items: Vec<FeedbackRequest>,
}

impl Default for FeedbackStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FeedbackStoreInner::default())),
        }
    }
}

impl FeedbackStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &self,
        kind: FeedbackKind,
        prompt: String,
        options: Vec<String>,
        gates: Vec<TaskId>,
        doubted_task_id: Option<TaskId>,
        info_gain_bits: f64,
        scaled_cost_ms: u64,
        surface: Surface,
        session_id: Option<String>,
        agent_id: Option<AgentId>,
        created_at_ms: u64,
        meta: Option<serde_json::Value>,
    ) -> FeedbackId {
        let mut inner = self.inner.write();
        inner.seq += 1;
        let id = FeedbackId(format!("F-{:06}", inner.seq));
        let req = FeedbackRequest {
            id: id.clone(),
            kind,
            prompt,
            options,
            gates,
            doubted_task_id,
            info_gain_bits,
            scaled_cost_ms,
            surface,
            session_id,
            agent_id,
            created_at_ms,
            resolution: None,
            meta,
        };
        inner.items.push(req);
        id
    }

    /// T1.4: restore visibility for a `FeedbackRequested` op-log entry that
    /// had no matching `FeedbackResolved` as of the last durable record
    /// before a daemon restart. Unlike [`register`](Self::register), the id
    /// is **preserved** from the durable `request_id` (so it lines up with
    /// whatever the GUI/CLI already displayed for it) and this is a no-op if
    /// an item with that id is already present (idempotent under repeated
    /// rehydration calls). The op-log entry carries only `request_id`,
    /// `task_id`, and a `kind` string — never the original `prompt`/
    /// `options`/`gates` — so the reconstructed item's `prompt` is a
    /// recovery-labeled placeholder, not the original text; this restores
    /// *that a decision is still owed*, not the original interactive
    /// question verbatim.
    pub fn rehydrate_open(
        &self,
        id: FeedbackId,
        kind: FeedbackKind,
        doubted_task_id: Option<TaskId>,
        created_at_ms: u64,
    ) {
        let mut inner = self.inner.write();
        if inner.items.iter().any(|i| i.id == id) {
            return;
        }
        let prompt = format!(
            "[recovered-on-restart] a {:?} feedback request from before the last restart \
             is still awaiting a decision (original prompt text was not durably recorded)",
            kind
        );
        let req = FeedbackRequest {
            id,
            kind,
            prompt,
            options: vec![],
            gates: doubted_task_id.into_iter().collect(),
            doubted_task_id,
            info_gain_bits: 0.0,
            scaled_cost_ms: 0,
            surface: Surface::NeedsYou,
            session_id: None,
            agent_id: None,
            created_at_ms,
            resolution: None,
            meta: None,
        };
        inner.items.push(req);
    }

    pub fn open_needs_you(&self) -> Vec<FeedbackRequest> {
        let inner = self.inner.read();
        inner
            .items
            .iter()
            .filter(|item| item.resolution.is_none() && item.surface == Surface::NeedsYou)
            .cloned()
            .collect()
    }

    pub fn withheld(&self) -> Vec<FeedbackRequest> {
        let inner = self.inner.read();
        inner
            .items
            .iter()
            .filter(|item| item.resolution.is_none() && item.surface == Surface::Withheld)
            .cloned()
            .collect()
    }

    pub fn get(&self, id: &FeedbackId) -> Option<FeedbackRequest> {
        let inner = self.inner.read();
        inner.items.iter().find(|item| item.id == *id).cloned()
    }

    pub fn resolve(&self, id: &FeedbackId, res: FeedbackResolution) -> Option<FeedbackRequest> {
        let mut inner = self.inner.write();
        if let Some(item) = inner.items.iter_mut().find(|item| item.id == *id) {
            if item.resolution.is_none() {
                item.resolution = Some(res);
                return Some(item.clone());
            }
        }
        None
    }

    pub fn promote_withheld<F>(&self, f: F)
    where
        F: Fn(&FeedbackRequest) -> Surface,
    {
        let mut inner = self.inner.write();
        for item in inner.items.iter_mut() {
            if item.resolution.is_none() && item.surface == Surface::Withheld {
                item.surface = f(item);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::{FeedbackAction, FeedbackKind, FeedbackResolution, Surface};
    use crate::types::TaskId;

    fn reg(s: &FeedbackStore, surface: Surface, gain: f64) -> FeedbackId {
        s.register(
            FeedbackKind::Clarification,
            "q?".into(),
            vec![],
            vec![TaskId(1)],
            None,
            gain,
            500,
            surface,
            None,
            None,
            1,
            None,
        )
    }

    #[test]
    fn needs_you_vs_withheld_partition() {
        let s = FeedbackStore::new();
        reg(&s, Surface::NeedsYou, 0.8);
        reg(&s, Surface::Withheld, 0.05);
        assert_eq!(s.open_needs_you().len(), 1);
        assert_eq!(s.withheld().len(), 1);
    }

    #[test]
    fn resolve_is_idempotent_and_removes_from_open() {
        let s = FeedbackStore::new();
        let id = reg(&s, Surface::NeedsYou, 0.8);
        let res = FeedbackResolution {
            action: FeedbackAction::Skip,
            decided_at_ms: 2,
            decided_by: "gui".into(),
        };
        assert!(s.resolve(&id, res.clone()).is_some());
        assert!(s.resolve(&id, res).is_none()); // already resolved => None
        assert_eq!(s.open_needs_you().len(), 0);
    }

    #[test]
    fn resolve_unknown_id_returns_none() {
        let s = FeedbackStore::new();
        let res = FeedbackResolution {
            action: FeedbackAction::Skip,
            decided_at_ms: 2,
            decided_by: "x".into(),
        };
        assert!(s.resolve(&FeedbackId("nope".into()), res).is_none());
    }
}
