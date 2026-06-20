use vox_terminal_core::transcript::{TranscriptEvent, TranscriptKind};

#[test]
fn event_roundtrips_json() {
    let e = TranscriptEvent {
        session_id: "s1".into(),
        seq: 1,
        kind: TranscriptKind::Submitted {
            intent: "Shell".into(),
            input: "ls".into(),
        },
    };
    let j = serde_json::to_string(&e).unwrap();
    let back: TranscriptEvent = serde_json::from_str(&j).unwrap();
    assert_eq!(back, e);
}

#[test]
fn all_variants_serialize() {
    use vox_terminal_core::block::{Block, BlockId, BlockKind};
    let variants = vec![
        TranscriptKind::Submitted {
            intent: "VoxNative".into(),
            input: "1 + 1".into(),
        },
        TranscriptKind::Output {
            stream: "Stdout".into(),
            text: "2\n".into(),
        },
        TranscriptKind::AgentTurn {
            text: "hello".into(),
        },
        TranscriptKind::ExitStatus { code: 0 },
        TranscriptKind::Accepted {
            block: Block::new(BlockId(1), BlockKind::Shell, "ls"),
        },
    ];
    for v in variants {
        let e = TranscriptEvent {
            session_id: "s1".into(),
            seq: 0,
            kind: v,
        };
        let j = serde_json::to_string(&e).unwrap();
        assert!(!j.is_empty());
    }
}
