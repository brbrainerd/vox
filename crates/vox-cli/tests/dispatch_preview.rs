//! P2-T6 acceptance: `vox dispatch preview` CLI argument parsing + RoutingDecision serde.

use clap::Parser;
use vox_cli::VoxCliRoot;

#[test]
fn dispatch_preview_parses() {
    let cli = VoxCliRoot::try_parse_from([
        "vox",
        "dispatch",
        "preview",
        "my::workflow",
        "--",
        "arg1",
        "arg2",
    ]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}

#[test]
fn dispatch_preview_no_args_parses() {
    let cli = VoxCliRoot::try_parse_from(["vox", "dispatch", "preview", "my::workflow"]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}

#[test]
fn routing_decision_serializes_round_trip() {
    use vox_cli::commands::dispatch::preview::RoutingDecision;

    let cases = vec![
        RoutingDecision::Local,
        RoutingDecision::Remote {
            peer_id: "p1".into(),
            reason: "label match".into(),
        },
        RoutingDecision::Cached {
            activity_id: "a".into(),
            arg_hash_hex: "ab12".into(),
        },
    ];
    for c in cases {
        let s = serde_json::to_string(&c).expect("serialize");
        let _: RoutingDecision = serde_json::from_str(&s).expect("deserialize");
    }
}
