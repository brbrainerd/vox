//! Shared text construction for training pairs (ChatML encoding).
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/training_text.rs` (SP3 sub-batch C).

use crate::config::ChatmlConfig;

#[must_use]
pub fn chatml_prefix_open_assistant(system: &str, user: &str, cfg: &ChatmlConfig) -> String {
    format!(
        "{start}{sys}\n{system}{end}\n\
         {start}{usr}\n{user}{end}\n\
         {start}{asst}\n",
        start = cfg.im_start,
        end = cfg.im_end,
        sys = cfg.role_system,
        usr = cfg.role_user,
        asst = cfg.role_assistant
    )
}

#[must_use]
pub fn chatml_turns_text(turns: &[vox_tensor::data::ChatmlTurn], cfg: &ChatmlConfig) -> String {
    let mut out = String::new();
    for turn in turns {
        out.push_str(&format!(
            "{start}{role}\n{content}{end}\n",
            start = cfg.im_start,
            end = cfg.im_end,
            role = turn.role,
            content = turn.content
        ));
    }
    out.trim_end().to_string()
}

#[must_use]
pub fn chatml_turns_prefix_open_assistant(
    turns: &[vox_tensor::data::ChatmlTurn],
    cfg: &ChatmlConfig,
) -> String {
    let mut out = String::new();
    for (i, turn) in turns.iter().enumerate() {
        if i == turns.len() - 1 && turn.role == "assistant" {
            out.push_str(&format!(
                "{start}{role}\n",
                start = cfg.im_start,
                role = cfg.role_assistant
            ));
            break;
        }
        out.push_str(&format!(
            "{start}{role}\n{content}{end}\n",
            start = cfg.im_start,
            end = cfg.im_end,
            role = turn.role,
            content = turn.content
        ));
    }
    out
}

#[must_use]
pub fn chatml_supervised_text(
    system: &str,
    user: &str,
    assistant: &str,
    cfg: &ChatmlConfig,
) -> String {
    format!(
        "{start}{sys}\n{system}{end}\n\
         {start}{usr}\n{user}{end}\n\
         {start}{asst}\n{assistant}{end}",
        start = cfg.im_start,
        end = cfg.im_end,
        sys = cfg.role_system,
        usr = cfg.role_user,
        asst = cfg.role_assistant
    )
}
