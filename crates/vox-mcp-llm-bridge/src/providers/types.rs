use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: Option<GeminiUsageMeta>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiCandidate {
    pub content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiContent {
    pub parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiPart {
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiUsageMeta {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OllamaChatResponse {
    pub message: Option<OllamaMsg>,
    #[serde(default)]
    pub eval_count: u32,
    #[serde(default)]
    pub prompt_eval_count: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OllamaMsg {
    pub content: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiGenerateBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSys<'a>>,
    pub contents: Vec<GeminiTurn<'a>>,
    pub generation_config: GeminiGenCfg<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiSys<'a> {
    pub parts: Vec<GeminiPartOut<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiPartOut<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<GeminiInlineData<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiInlineData<'a> {
    pub mime_type: &'a str,
    pub data: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiTurn<'a> {
    pub role: &'a str,
    pub parts: Vec<GeminiPartOut<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiGenCfg<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    pub max_output_tokens: u32,
    #[serde(rename = "responseMimeType", skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<&'a str>,
}

#[derive(Serialize)]
pub(crate) struct OllamaChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<OllamaChatMsg<'a>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<serde_json::Value>,
    pub options: OllamaOptions,
}

#[derive(Serialize)]
pub(crate) struct OllamaChatMsg<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

#[derive(Serialize)]
pub(crate) struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    pub num_predict: i32,
}
