//! JSONL loaders + replay buffer wiring (`vox-tensor`).

use std::io::Write;

use tempfile::NamedTempFile;
use vox_tensor::data::{self, TrainingPair};
use vox_tensor::replay::{ReplayBuffer, ReplayConfig, ReplaySample};

#[test]
fn count_and_load_jsonl_with_instruction_alias() {
    let mut f = NamedTempFile::new().expect("tempfile");
    writeln!(
        f,
        "{{\"instruction\":\"prompt\",\"output\":\"resp\",\"rating\":5}}"
    )
    .unwrap();
    writeln!(f).unwrap(); // empty line ignored by count
    writeln!(f, "{{\"invalid\":").unwrap(); // malformed skipped by load_all

    let path = f.path();
    assert_eq!(data::count_jsonl_records(path).unwrap(), 2);

    let loaded = data::load_all(path, 0).expect("load_all");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].effective_prompt(), Some(&"prompt".to_string()));
    assert_eq!(loaded[0].effective_response(), Some(&"resp".to_string()));
}

#[test]
fn replay_buffer_add_and_select_round_robin() {
    let mut buf = ReplayBuffer::new(ReplayConfig {
        replay_ratio: 0.5,
        max_buffer_size: 100,
        mix_cd_enabled: false,
        loss_increase_threshold: 0.1,
    });

    buf.add_sample(TrainingPair {
        prompt: Some("p1".into()),
        response: Some("r1".into()),
        ..Default::default()
    });
    buf.add_sample(TrainingPair {
        prompt: Some("p2".into()),
        response: Some("r2".into()),
        ..Default::default()
    });
    assert_eq!(buf.len(), 2);

    let batch = buf.select_replay_batch(2);
    assert_eq!(batch.len(), 2);
}

#[test]
fn replay_sample_loss_delta_and_at_risk() {
    let mut s = ReplaySample {
        pair: TrainingPair::default(),
        prev_loss: Some(1.0),
        curr_loss: Some(1.5),
        replay_count: 0,
    };
    assert!((s.loss_delta() - 0.5).abs() < f64::EPSILON);
    assert!(s.is_at_risk(0.2));

    s.curr_loss = Some(0.5);
    assert!(s.loss_delta() < 0.0);
    assert!(!s.is_at_risk(0.1));
}
