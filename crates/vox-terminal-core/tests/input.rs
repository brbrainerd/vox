use vox_terminal_core::input::{InputIntent, classify};

#[test]
fn default_is_vox_native() {
    assert_eq!(
        classify("let x = 1"),
        InputIntent::VoxNative("let x = 1".into())
    );
}

#[test]
fn slash_sh_is_shell() {
    assert_eq!(
        classify("/sh git status"),
        InputIntent::Shell("git status".into())
    );
}

#[test]
fn bang_is_shell() {
    assert_eq!(classify("!ls -la"), InputIntent::Shell("ls -la".into()));
}

#[test]
fn slash_ai_is_agent() {
    assert_eq!(
        classify("/ai fix the failing test"),
        InputIntent::Agent("fix the failing test".into())
    );
}

#[test]
fn other_slash_is_command() {
    assert_eq!(
        classify("/model list"),
        InputIntent::Command {
            name: "model".into(),
            args: "list".into()
        }
    );
}
