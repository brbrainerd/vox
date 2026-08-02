//! Parse coverage for unified login entrypoints (`vox login`, `vox secrets login`, `vox auth login`).

use clap::Parser;
use vox_cli::commands::auth::AuthCmd;
use vox_cli::commands::secrets::SecretsCmd;
use vox_cli::{Cli, VoxCliRoot};

#[test]
fn cli_parses_top_level_login_with_flags() {
    let r = VoxCliRoot::try_parse_from([
        "vox",
        "login",
        "--non-interactive",
        "--vault-url",
        "libsql://example.com",
        "--vault-token",
        "tok",
        "--account",
        "acc1",
        "--backend",
        "vox_cloud",
    ])
    .expect("top-level login");
    match r.cmd {
        Cli::Login { args } => {
            assert!(args.non_interactive);
            assert_eq!(args.vault_url.as_deref(), Some("libsql://example.com"));
            assert_eq!(args.vault_token.as_deref(), Some("tok"));
            assert_eq!(args.account_id.as_deref(), Some("acc1"));
            assert_eq!(args.backend.as_deref(), Some("vox_cloud"));
        }
        _other => panic!("expected Cli::Login, got unexpected Cli variant"),
    }
}

#[test]
fn cli_parses_secrets_login() {
    let r = VoxCliRoot::try_parse_from([
        "vox",
        "secrets",
        "login",
        "--force",
        "--vault-url",
        "https://db.example",
    ])
    .expect("secrets login");
    match r.cmd {
        Cli::Secrets { cmd } => match cmd {
            SecretsCmd::Login { args, .. } => {
                assert!(args.force);
                assert_eq!(args.vault_url.as_deref(), Some("https://db.example"));
            }
            _other => panic!("expected SecretsCmd::Login, got other subcommand"),
        },
        _other => panic!("expected Cli::Secrets, got non-Secrets variant"),
    }
}

#[test]
fn cli_parses_clavis_login_alias() {
    let r = VoxCliRoot::try_parse_from([
        "vox",
        "clavis",
        "login",
        "--force",
        "--vault-url",
        "https://db.example",
    ])
    .expect("clavis login alias");
    match r.cmd {
        Cli::Secrets { cmd } => match cmd {
            SecretsCmd::Login { args, .. } => {
                assert!(args.force);
                assert_eq!(args.vault_url.as_deref(), Some("https://db.example"));
            }
            _other => panic!("expected SecretsCmd::Login, got other subcommand"),
        },
        _other => panic!("expected Cli::Secrets via clavis alias, got non-Secrets variant"),
    }
}

#[test]
fn cli_parses_auth_login_alias_of_connect() {
    let r = VoxCliRoot::try_parse_from([
        "vox",
        "auth",
        "login",
        "--non-interactive",
        "--vault-url",
        "libsql://x",
        "--vault-token",
        "y",
    ])
    .expect("auth login");
    match r.cmd {
        Cli::Auth { cmd } => match cmd {
            AuthCmd::Connect { args } => {
                assert!(args.non_interactive);
                assert_eq!(args.vault_url.as_deref(), Some("libsql://x"));
                assert_eq!(args.vault_token.as_deref(), Some("y"));
            }
            _other => {
                panic!("expected AuthCmd::Connect for alias login, got other auth subcommand")
            }
        },
        _other => panic!("expected Cli::Auth, got non-Auth variant"),
    }
}

#[test]
fn cli_parses_auth_connect_still_works() {
    let r = VoxCliRoot::try_parse_from([
        "vox",
        "auth",
        "connect",
        "--vault-url",
        "libsql://z",
        "--vault-token",
        "t",
    ])
    .expect("auth connect");
    match r.cmd {
        Cli::Auth { cmd } => match cmd {
            AuthCmd::Connect { args } => {
                assert_eq!(args.vault_url.as_deref(), Some("libsql://z"));
                assert_eq!(args.vault_token.as_deref(), Some("t"));
            }
            _other => panic!("expected AuthCmd::Connect, got other auth subcommand"),
        },
        _other => panic!("expected Cli::Auth, got non-Auth variant"),
    }
}
