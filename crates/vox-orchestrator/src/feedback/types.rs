use crate::types::{AgentId, TaskId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeedbackId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Clarification,
    Doubt,
    SkillProposal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    NeedsYou,
    Withheld,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FeedbackAction {
    Answer {
        option: Option<usize>,
        text: Option<String>,
    },
    Skip,
    Overrule,
    LetVerify,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackResolution {
    pub action: FeedbackAction,
    pub decided_at_ms: u64,
    pub decided_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub id: FeedbackId,
    pub kind: FeedbackKind,
    pub prompt: String,
    pub options: Vec<String>,
    pub gates: Vec<TaskId>,
    pub doubted_task_id: Option<TaskId>,
    pub info_gain_bits: f64,
    pub scaled_cost_ms: u64,
    pub surface: Surface,
    pub session_id: Option<String>,
    pub agent_id: Option<AgentId>,
    pub created_at_ms: u64,
    pub resolution: Option<FeedbackResolution>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trips_with_snake_case_tags() {
        let req = FeedbackRequest {
            id: FeedbackId("F-000001".into()),
            kind: FeedbackKind::Clarification,
            prompt: "schema?".into(),
            options: vec!["a".into()],
            gates: vec![TaskId(7)],
            doubted_task_id: None,
            info_gain_bits: 0.8,
            scaled_cost_ms: 1000,
            surface: Surface::NeedsYou,
            session_id: None,
            agent_id: None,
            created_at_ms: 1,
            resolution: None,
        };
        let j = serde_json::to_string(&req).unwrap();
        assert!(j.contains("\"kind\":\"clarification\""));
        assert!(j.contains("\"surface\":\"needs_you\""));
        let back: FeedbackRequest = serde_json::from_str(&j).unwrap();
        assert_eq!(back.gates, vec![TaskId(7)]);
    }
    #[test]
    fn action_is_internally_tagged() {
        let a = FeedbackAction::Answer {
            option: Some(1),
            text: None,
        };
        assert!(
            serde_json::to_string(&a)
                .unwrap()
                .contains("\"action\":\"answer\"")
        );
        assert!(
            serde_json::to_string(&FeedbackAction::Overrule)
                .unwrap()
                .contains("overrule")
        );
    }
}
