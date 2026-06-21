use vox_terminal_core::block::{Block, BlockId, BlockKind, BlockStatus, OutputChunk, Stream};

#[test]
fn block_accumulates_output_and_finishes() {
    let mut b = Block::new(BlockId(1), BlockKind::Shell, "echo hi");
    assert_eq!(b.id, BlockId(1));
    assert_eq!(b.status, BlockStatus::Running);
    b.push(OutputChunk::text(Stream::Stdout, "hi\n"));
    b.finish(0);
    assert_eq!(b.status, BlockStatus::Ok);
    assert_eq!(b.exit_code, Some(0));
    assert_eq!(b.plain_output(), "hi\n");
}

#[test]
fn plain_output_strips_ansi() {
    let mut b = Block::new(BlockId(2), BlockKind::Shell, "ls");
    b.push(OutputChunk::text(Stream::Stdout, "\x1b[31mred\x1b[0m\n"));
    assert_eq!(b.plain_output(), "red\n");
}

#[test]
fn nonzero_exit_marks_failed() {
    let mut b = Block::new(BlockId(3), BlockKind::Shell, "false");
    b.finish(1);
    assert_eq!(b.status, BlockStatus::Failed);
    assert_eq!(b.exit_code, Some(1));
}
