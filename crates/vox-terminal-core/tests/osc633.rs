use vox_terminal_core::osc633::{Osc633Event, Osc633Parser};

#[test]
fn parses_command_and_exit_markers() {
    let mut p = Osc633Parser::new();
    let mut evs = vec![];
    evs.extend(p.feed(b"\x1b]633;A\x07"));
    evs.extend(p.feed(b"\x1b]633;E;ls -la\x07"));
    evs.extend(p.feed(b"\x1b]633;C\x07"));
    evs.extend(p.feed(b"total 0\n"));
    evs.extend(p.feed(b"\x1b]633;D;0\x07"));
    assert!(evs.contains(&Osc633Event::PromptStart));
    assert!(evs.contains(&Osc633Event::CommandLine("ls -la".into())));
    assert!(evs.contains(&Osc633Event::PreExec));
    assert!(evs
        .iter()
        .any(|e| matches!(e, Osc633Event::Output(s) if s == "total 0\n")));
    assert!(evs.contains(&Osc633Event::Exit(0)));
}

#[test]
fn marker_split_across_feeds() {
    // Partial ESC sequence fed across two calls — must not lose data
    let mut p = Osc633Parser::new();
    let mut evs = vec![];
    // split "\x1b]633;A\x07" after ESC
    evs.extend(p.feed(b"\x1b"));
    evs.extend(p.feed(b"]633;A\x07"));
    assert!(evs.contains(&Osc633Event::PromptStart));
}

#[test]
fn decode_command_general_hex() {
    use vox_terminal_core::osc633::decode_command;
    // \x3b = ';', \x20 = ' '
    assert_eq!(decode_command(r"ls\x3b\x20rm"), "ls; rm");
}

#[test]
fn output_passthrough_between_markers() {
    let mut p = Osc633Parser::new();
    let mut evs = vec![];
    evs.extend(p.feed(b"\x1b]633;C\x07hello world\n\x1b]633;D;1\x07"));
    assert!(evs
        .iter()
        .any(|e| matches!(e, Osc633Event::Output(s) if s == "hello world\n")));
    assert!(evs.contains(&Osc633Event::Exit(1)));
}
