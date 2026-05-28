//! OpenAI-compatible **non-streaming** chat completion JSON shapes (`/v1/chat/completions`).
//!
//! Streaming (`text/event-stream`) line assembly lives in `vox-openai-sse`.

mod chat_completion;
pub mod sse;

pub use chat_completion::{
    ChatCompletionAssistantMessage, ChatCompletionChoice, ChatCompletionFunctionCall,
    ChatCompletionRequest, ChatCompletionResponse, ChatCompletionToolCall, ChatCompletionUsage,
    ChatMessageContent, ChatMessagePart, ChatMessageTurn, ImageUrl,
};
