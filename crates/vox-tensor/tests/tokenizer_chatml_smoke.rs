//! Cross-module smoke for `data` tokenizer + ChatML pipeline (`vox-tensor`).
//!
//! Pure-CPU coverage of the lab tokenizer: ASCII round-trip, multi-turn
//! ChatML encoding, and the supervision-mask invariants enforced by
//! `tokenize_for_training`. None of these surfaces require GPU features
//! or external tokenizer assets.

use vox_tensor::data::{ChatmlTurn, VOCAB_SIZE, VoxTokenizer};

#[test]
fn ascii_text_roundtrips_through_tokenizer() {
    let text = "fn add(a: int, b: int) to int: return a + b";
    let ids = VoxTokenizer::encode(text);
    assert!(!ids.is_empty(), "non-empty input must produce tokens");
    assert!(
        ids.iter().all(|&id| (id as usize) < VOCAB_SIZE),
        "all token ids must fit inside the lab vocab"
    );
    assert_eq!(VoxTokenizer::decode(&ids), text);
}

#[test]
fn encode_chatml_turns_decodes_role_content_pairs() {
    let turns = vec![
        ChatmlTurn {
            role: "system".into(),
            content: "be concise".into(),
        },
        ChatmlTurn {
            role: "user".into(),
            content: "name a fruit".into(),
        },
        ChatmlTurn {
            role: "assistant".into(),
            content: "apple".into(),
        },
    ];
    let ids = VoxTokenizer::encode_chatml_turns(&turns);
    let decoded = VoxTokenizer::decode(&ids);
    assert!(decoded.contains("be concise"));
    assert!(decoded.contains("name a fruit"));
    assert!(decoded.contains("apple"));
    assert!(decoded.contains("<|im_start|>"));
}

#[test]
fn tokenize_for_training_masks_prompt_with_minus_100() {
    let max_len = 96usize;
    let (input_ids, labels) =
        VoxTokenizer::tokenize_for_training("sys-msg", "user-msg", "assistant-resp", max_len);
    assert_eq!(input_ids.len(), max_len);
    assert_eq!(labels.len(), max_len);

    assert!(
        labels.contains(&-100),
        "prompt region must be masked with -100"
    );
    assert!(
        labels.iter().any(|&l| l > 0),
        "assistant region must contain real (positive) supervision tokens"
    );
}
