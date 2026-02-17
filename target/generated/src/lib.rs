use serde::{Serialize, Deserialize};
use vox_runtime::{ProcessContext, Envelope, MessagePayload, Pid, Message};

pub fn as_string<T: serde::Serialize>(v: &T) -> String {
    let val = serde_json::to_value(v).unwrap();
    if let Some(s) = val.as_str() { s.to_string() } else { val.to_string() }
}

pub fn append(list: &Vec<serde_json::Value>, item: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut new_list = list.clone();
    new_list.push(item.clone());
    new_list
}

pub use self::ChatResult::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatResult {
    Success(String, ),
    Error(String, ),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClaudeMessage {
    Send { msg: String, },
}

pub struct Claude;
impl Claude {
    pub async fn run(mut ctx: ProcessContext) {
        while let Some(envelope) = ctx.receive().await {
            match envelope {
                vox_runtime::Envelope::Request(req) => {
                    if let vox_runtime::MessagePayload::Json(json_str) = &req.payload {
                        if let Ok(actor_msg) = serde_json::from_str::<ClaudeMessage>(&json_str) {
                            let reply_str = match actor_msg {
                                ClaudeMessage::Send { msg, } => {
                                    serde_json::to_string(&(Success(("Hello from Vox! You said: ".to_string() + &msg.clone())))).unwrap_or_default()
                                }
                            };
                            ProcessContext::reply(req, reply_str);
                        }
                    }
                }
                vox_runtime::Envelope::Message(msg) => {
                    // Fire-and-forget: process but don't reply
                    if let vox_runtime::MessagePayload::Json(json_str) = msg.payload {
                        if let Ok(actor_msg) = serde_json::from_str::<ClaudeMessage>(&json_str) {
                            let _ = actor_msg; // processed
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone)]
pub struct ClaudeHandle {
    handle: vox_runtime::ProcessHandle,
}
impl ClaudeHandle {
    pub fn new(handle: vox_runtime::ProcessHandle) -> Self { Self { handle } }
    pub fn spawn() -> Self {
        let handle = vox_runtime::spawn_process(Claude::run);
        Self::new(handle)
    }
    pub async fn send(&self, msg: String, ) -> String {
        let msg = ClaudeMessage::Send { msg, };
        let payload = vox_runtime::MessagePayload::Json(serde_json::to_string(&msg).unwrap());
        self.handle.call(payload).await.unwrap_or_else(|e| format!("Actor error: {}", e))
    }
}

