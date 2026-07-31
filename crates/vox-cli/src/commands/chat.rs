//! `vox chat <message>` — send one message through the harness's chat pipeline
//! from the terminal and print the model's reply to stdout.
//!
//! Useful as a live, hermetic-ish-adjacent eval test surface (it makes a real
//! network call by nature, so it is not itself hermetic) and as a
//! quick-and-dirty way to talk to a model without the GUI. Reuses the exact
//! same [`vox_actor_runtime::llm::llm_chat`] durable-activity chat facade the
//! rest of the harness (e.g. `vox model eval`, see
//! `crates/vox-cli/src/commands/model/eval.rs::eval_one_model`) calls — no
//! separate chat implementation.

use anyhow::{Context, Result, bail};
use clap::Parser;
use vox_actor_runtime::ActivityOptions;
use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, llm_chat};

/// `vox chat` arguments.
#[derive(Parser, Debug)]
pub struct ChatArgs {
    /// The message to send.
    pub message: String,
    /// OpenRouter model id to use (default: `openrouter/auto`, provider-side routing).
    #[arg(long, default_value = vox_config::bootstrap_inference::OPENROUTER_AUTO)]
    pub model: String,
    /// Optional system prompt prepended before the user message.
    #[arg(long)]
    pub system: Option<String>,
}

/// Run `vox chat`: send `args.message` (with optional `args.system` prompt) to
/// `args.model` via the shared `llm_chat` facade and print the reply.
pub async fn run(args: ChatArgs) -> Result<()> {
    if args.message.trim().is_empty() {
        bail!("vox chat: message must not be empty");
    }

    let mut messages = Vec::with_capacity(2);
    if let Some(system) = &args.system {
        messages.push(LlmChatMessage {
            role: "system".to_string(),
            content: system.clone(),
            ..Default::default()
        });
    }
    messages.push(LlmChatMessage {
        role: "user".to_string(),
        content: args.message.clone(),
        ..Default::default()
    });

    let mut config = LlmConfig::openrouter(&args.model);
    config.telemetry_task_category = Some("cli-chat".to_string());

    let opts = ActivityOptions::new();
    let outcome = llm_chat(&opts, messages, config).await;

    let flattened: Result<vox_actor_runtime::llm::LlmResponse, String> = match outcome {
        vox_actor_runtime::ActivityResult::Ok(inner) => inner,
        vox_actor_runtime::ActivityResult::Failed(e) => Err(e.to_string()),
        vox_actor_runtime::ActivityResult::Cancelled => Err("activity cancelled".to_string()),
    };

    let response = flattened
        .map_err(|e| anyhow::anyhow!(e))
        .context("vox chat: llm_chat failed")?;
    println!("{}", response.content);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `vox chat` arg parsing: the message is positional and required, `--model`
    /// defaults to `openrouter/auto`, and both are overridable. Mirrors the
    /// `cli-harness-eval-arg-parsing` golden-task pattern
    /// (`crates/vox-cli/src/commands/harness/eval.rs`) of exercising clap-derive
    /// dispatch directly, without a live backend.
    #[test]
    fn chat_args_parse_defaults_and_overrides() {
        let default_args = ChatArgs::try_parse_from(["chat", "hello there"])
            .expect("default parse should succeed");
        assert_eq!(default_args.message, "hello there");
        assert_eq!(default_args.model, "openrouter/auto");
        assert_eq!(default_args.system, None);

        let explicit_args = ChatArgs::try_parse_from([
            "chat",
            "hi",
            "--model",
            "anthropic/claude-3.5-sonnet",
            "--system",
            "be terse",
        ])
        .expect("explicit parse should succeed");
        assert_eq!(explicit_args.message, "hi");
        assert_eq!(explicit_args.model, "anthropic/claude-3.5-sonnet");
        assert_eq!(explicit_args.system.as_deref(), Some("be terse"));
    }

    #[test]
    fn chat_args_require_message() {
        assert!(ChatArgs::try_parse_from(["chat"]).is_err());
    }
}
