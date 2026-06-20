use vox_terminal_core::block::BlockStatus;
use vox_terminal_core::session::Session;

#[test]
fn shell_block_lifecycle_from_osc_events() {
    let mut s = Session::new("s1");
    s.on_pty_bytes(b"\x1b]633;A\x07\x1b]633;E;ls\x07\x1b]633;C\x07out\n\x1b]633;D;0\x07");
    let blocks = s.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].input, "ls");
    assert_eq!(blocks[0].plain_output(), "out\n");
    assert_eq!(blocks[0].status, BlockStatus::Ok);
}

#[test]
fn multiple_blocks_sequential() {
    let mut s = Session::new("s2");
    s.on_pty_bytes(b"\x1b]633;A\x07\x1b]633;E;echo a\x07\x1b]633;C\x07a\n\x1b]633;D;0\x07");
    s.on_pty_bytes(b"\x1b]633;A\x07\x1b]633;E;echo b\x07\x1b]633;C\x07b\n\x1b]633;D;1\x07");
    let blocks = s.blocks();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].input, "echo a");
    assert_eq!(blocks[0].status, BlockStatus::Ok);
    assert_eq!(blocks[1].input, "echo b");
    assert_eq!(blocks[1].status, BlockStatus::Failed);
}

#[test]
fn block_ids_are_unique() {
    let mut s = Session::new("s3");
    s.on_pty_bytes(b"\x1b]633;A\x07\x1b]633;D;0\x07");
    s.on_pty_bytes(b"\x1b]633;A\x07\x1b]633;D;0\x07");
    let ids: Vec<_> = s.blocks().iter().map(|b| b.id).collect();
    assert_ne!(ids[0], ids[1]);
}
