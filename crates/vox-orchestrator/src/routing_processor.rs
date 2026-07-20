//! Dispatches a task to the chat processor or the full agentic processor
//! based on `task.task_category`, while presenting exactly one
//! [`crate::runtime::TaskProcessor`] to [`crate::runtime::AgentFleet`] (which
//! holds only one `Arc<dyn TaskProcessor>` for the whole fleet).

use crate::runtime::TaskProcessor;
use crate::types::{AgentId, AgentTask, TaskCategory, TaskId};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct RoutingTaskProcessor<A: TaskProcessor, C: TaskProcessor> {
    agentic: Arc<A>,
    chat: Arc<C>,
}

impl<A: TaskProcessor, C: TaskProcessor> RoutingTaskProcessor<A, C> {
    pub fn new(agentic: Arc<A>, chat: Arc<C>) -> Self {
        Self { agentic, chat }
    }
}

#[async_trait::async_trait]
impl<A: TaskProcessor, C: TaskProcessor> TaskProcessor for RoutingTaskProcessor<A, C> {
    async fn process(
        &self,
        agent_id: AgentId,
        task: AgentTask,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<TaskId> {
        match task.task_category {
            TaskCategory::Chat => self.chat.process(agent_id, task, cancel).await,
            _ => self.agentic.process(agent_id, task, cancel).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::TaskProcessor;
    use crate::types::{AgentId, AgentTask, TaskCategory, TaskId, TaskPriority};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    // Two counting stub processors distinguishable by which one got called.
    struct CountingProcessor(std::sync::atomic::AtomicUsize);
    #[async_trait::async_trait]
    impl TaskProcessor for CountingProcessor {
        async fn process(
            &self,
            _a: AgentId,
            task: AgentTask,
            _c: Arc<AtomicBool>,
        ) -> anyhow::Result<TaskId> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(task.id)
        }
    }

    #[tokio::test]
    async fn chat_category_routes_to_chat_processor_others_to_agentic() {
        let chat_calls = Arc::new(CountingProcessor(Default::default()));
        let agentic_calls = Arc::new(CountingProcessor(Default::default()));
        let router = RoutingTaskProcessor::new(agentic_calls.clone(), chat_calls.clone());

        let mut chat_task = AgentTask::new(TaskId(1), "hi", TaskPriority::Normal, vec![]);
        chat_task.task_category = TaskCategory::Chat;
        router
            .process(AgentId(1), chat_task, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert_eq!(
            chat_calls.0.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "chat-category task must route to the chat processor"
        );
        assert_eq!(
            agentic_calls.0.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "chat-category task must not touch the agentic processor"
        );

        let agentic_task = AgentTask::new(TaskId(2), "do a thing", TaskPriority::Normal, vec![]);
        router
            .process(AgentId(1), agentic_task, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert_eq!(
            agentic_calls.0.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "non-chat task must route to the agentic processor"
        );
        assert_eq!(
            chat_calls.0.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "non-chat task must not touch the chat processor again"
        );
    }
}
