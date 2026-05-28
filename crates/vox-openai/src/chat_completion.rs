use serde::{Deserialize, Serialize};

/// Parsed JSON body from a successful chat completion response.
#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Option<ChatCompletionUsage>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionChoice {
    pub message: Option<ChatCompletionAssistantMessage>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionAssistantMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ChatCompletionToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionToolCall {
    pub function: ChatCompletionFunctionCall,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ChatCompletionUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub total_cost: Option<f64>,
    #[serde(default)]
    pub prompt_tokens_details: Option<ChatCompletionPromptDetails>,
    /// Anthropic-style cache metrics.
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ChatCompletionPromptDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

/// Request body for OpenAI-compatible chat (non-stream).
#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatMessageTurn<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    pub max_tokens: u64,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageTurn<'a> {
    pub role: &'a str,
    pub content: ChatMessageContent<'a>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum ChatMessageContent<'a> {
    Text(&'a str),
    Parts(Vec<ChatMessagePart<'a>>),
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatMessagePart<'a> {
    Text { text: &'a str },
    ImageUrl { image_url: ImageUrl<'a> },
}

#[derive(Debug, Serialize, Clone)]
pub struct ImageUrl<'a> {
    pub url: &'a str,
}
