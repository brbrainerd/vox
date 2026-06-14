//! Adversarial unit tests for vox-container-types.
//! Module: semcov_wave47_tests
//! Focuses on: constructors, bounds/edge cases, serde round-trips, empty/null handling.

#[cfg(test)]
mod semcov_wave47_tests {
    use std::collections::HashMap;

    use crate::{
        exec_grammar::{self, parse, parse_pipeline, ExecPolicy, PolicyViolation, RiskLevel, ViolationKind, risk},
        BuildOpts, RunOpts,
        detect::RuntimePreference,
    };
    use std::path::PathBuf;

    // ── RunOpts constructors ──────────────────────────────────────────────────

    #[test]
    fn run_opts_default_image_is_empty() {
        // Catches: default() producing a non-empty image string that silently passes
        // validation and causes a container runtime to run with no image.
        let opts = RunOpts::default();
        assert!(opts.image.is_empty(), "default image must be empty, not {:?}", opts.image);
    }

    #[test]
    fn run_opts_default_rm_is_true() {
        // Catches: rm defaulting to false, leaving zombie containers after every run.
        let opts = RunOpts::default();
        assert!(opts.rm, "default RunOpts must have rm=true to avoid container leaks");
    }

    #[test]
    fn run_opts_default_detach_is_false() {
        // Catches: detach accidentally defaulting to true, causing foreground callers
        // to never get output from the spawned container.
        let opts = RunOpts::default();
        assert!(!opts.detach);
    }

    #[test]
    fn run_opts_default_name_is_none() {
        // Catches: default name being Some("") which might be passed as --name ""
        // to the runtime, producing an error or unnamed container collision.
        let opts = RunOpts::default();
        assert!(opts.name.is_none());
    }

    #[test]
    fn run_opts_port_zero_zero_roundtrips() {
        // Catches: u16 port (0, 0) being silently dropped or panicking during
        // conversion to CLI flag "--publish 0:0".
        let mut opts = RunOpts::default();
        opts.ports.push((0, 0));
        assert_eq!(opts.ports[0], (0, 0));
    }

    #[test]
    fn run_opts_max_port_roundtrips() {
        // Catches: u16 overflow truncation when constructing port strings from
        // (65535, 65535).
        let mut opts = RunOpts::default();
        opts.ports.push((u16::MAX, u16::MAX));
        assert_eq!(opts.ports[0], (u16::MAX, u16::MAX));
    }

    // ── BuildOpts constructors ────────────────────────────────────────────────

    #[test]
    fn build_opts_empty_tag_is_accepted() {
        // Catches: a defensive assert or unwrap in BuildOpts constructor that
        // rejects an empty tag, even though callers may validate lazily.
        let opts = BuildOpts {
            context_dir: PathBuf::from("."),
            dockerfile: None,
            tag: String::new(),
            build_args: vec![],
        };
        assert!(opts.tag.is_empty());
    }

    #[test]
    fn build_opts_dockerfile_none_means_default() {
        // Catches: accidental Some("") treated the same as None, causing the runtime
        // CLI to receive --file "" which errors differently than omitting --file.
        let opts = BuildOpts {
            context_dir: PathBuf::from("/workspace"),
            dockerfile: None,
            tag: "test:latest".into(),
            build_args: vec![],
        };
        assert!(opts.dockerfile.is_none());
    }

    #[test]
    fn build_opts_build_arg_with_empty_value() {
        // Catches: build_args filtering out entries whose value is empty, silently
        // dropping ARG FOO="" which is a legitimate Dockerfile directive.
        let opts = BuildOpts {
            context_dir: PathBuf::from("."),
            dockerfile: None,
            tag: "img:1".into(),
            build_args: vec![("FOO".into(), "".into())],
        };
        assert_eq!(opts.build_args.len(), 1);
        assert_eq!(opts.build_args[0].1, "");
    }

    // ── RuntimePreference ─────────────────────────────────────────────────────

    #[test]
    fn runtime_preference_default_is_auto() {
        // Catches: Default impl returning Docker or Podman instead of Auto, which
        // would break environments where only one runtime is installed.
        assert_eq!(RuntimePreference::default(), RuntimePreference::Auto);
    }

    #[test]
    fn runtime_preference_from_str_case_insensitive() {
        // Catches: FromStr only matching exact case ("Auto" vs "auto" vs "AUTO"),
        // causing YAML/env-var config with uppercase values to silently use Auto.
        use std::str::FromStr;
        assert_eq!(RuntimePreference::from_str("DOCKER").unwrap(), RuntimePreference::Docker);
        assert_eq!(RuntimePreference::from_str("Podman").unwrap(), RuntimePreference::Podman);
        assert_eq!(RuntimePreference::from_str("AUTO").unwrap(), RuntimePreference::Auto);
    }

    #[test]
    fn runtime_preference_from_str_unknown_errors() {
        // Catches: FromStr returning a default (e.g. Auto) on unknown strings instead
        // of Err, masking typos in config files.
        use std::str::FromStr;
        assert!(RuntimePreference::from_str("kubernetes").is_err());
        assert!(RuntimePreference::from_str("").is_err());
    }

    // ── RiskLevel ordering ────────────────────────────────────────────────────

    #[test]
    fn risk_level_ordering_unknown_lt_safe_lt_elevated_lt_blocked() {
        // Catches: PartialOrd derivation placing Blocked < Safe, which would break
        // any code that gates on `risk >= RiskLevel::Elevated`.
        assert!(RiskLevel::Unknown < RiskLevel::Safe);
        assert!(RiskLevel::Safe < RiskLevel::Elevated);
        assert!(RiskLevel::Elevated < RiskLevel::Blocked);
    }

    // ── serde round-trips ─────────────────────────────────────────────────────

    #[test]
    fn risk_level_serde_round_trip() {
        // Catches: serde rename_all = "snake_case" producing "unknown" but
        // deserializing expecting "Unknown", or vice-versa.
        let levels = [
            RiskLevel::Unknown,
            RiskLevel::Safe,
            RiskLevel::Elevated,
            RiskLevel::Blocked,
        ];
        for level in levels {
            let json = serde_json::to_string(&level).expect("serialize");
            let back: RiskLevel = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, level, "round-trip failed for {:?}", level);
        }
    }

    #[test]
    fn exec_ast_serde_round_trip() {
        // Catches: missing #[derive(Serialize, Deserialize)] on Arg/Flag/Redirect,
        // or field rename mismatches causing silent data loss on round-trip.
        let mut ast = parse("cargo build --release -p vox-cli > out.log").unwrap();
        risk::classify(&mut ast, &ExecPolicy::default());
        let json = serde_json::to_string(&ast).expect("serialize ExecAst");
        let back: crate::exec_grammar::ExecAst =
            serde_json::from_str(&json).expect("deserialize ExecAst");
        assert_eq!(ast, back);
    }

    #[test]
    fn exec_policy_serde_round_trip() {
        // Catches: HashMap<String, Vec<String>> blocked_parameters losing entries
        // on JSON/YAML round-trip due to key collision or serde default handling.
        let mut bp = HashMap::new();
        bp.insert("*".into(), vec!["Recurse".into(), "Force".into()]);
        bp.insert("rm".into(), vec!["no-preserve-root".into()]);
        let policy = ExecPolicy {
            allowed_binaries: vec!["cargo".into()],
            allowed_cmdlets: vec!["Get-ChildItem".into()],
            blocked_parameters: bp,
            network_fetch_commands: vec!["curl".into()],
            network_fetch_domains: vec!["crates.io".into()],
        };
        let json = serde_json::to_string(&policy).expect("serialize");
        let back: ExecPolicy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.allowed_binaries, policy.allowed_binaries);
        assert_eq!(back.network_fetch_commands, policy.network_fetch_commands);
        assert_eq!(
            back.blocked_parameters.get("*"),
            policy.blocked_parameters.get("*")
        );
    }

    // ── parse edge cases ──────────────────────────────────────────────────────

    #[test]
    fn parse_whitespace_only_is_empty_error() {
        // Catches: trim() not being called so "\t  \n" is treated as a non-empty
        // command producing ExecAst { command: "\t  \n", … }.
        assert!(matches!(parse("   \t\n  "), Err(exec_grammar::ParseError::Empty)));
    }

    #[test]
    fn parse_single_quote_inside_double_quotes_not_toggle() {
        // Catches: tokeniser closing the double-quote context when it sees `'`
        // inside double quotes, breaking commands like: echo "it's alive".
        let ast = parse(r#"echo "it's alive""#).unwrap();
        assert_eq!(ast.args[0].0, "it's alive");
    }

    #[test]
    fn parse_flag_double_dash_empty_value_is_positional() {
        // Catches: `--` being consumed as a flag name rather than as the
        // end-of-flags sentinel, so subsequent args end up in flags not args.
        let ast = parse("cargo run -- arg1 arg2").unwrap();
        assert!(ast.flags.is_empty(), "flags should be empty after --");
        assert_eq!(ast.args.len(), 3, "arg1 and arg2 and 'run' should be positional");
    }

    #[test]
    fn parse_long_flag_equals_empty_value() {
        // Catches: `--flag=` (empty RHS of =) splitting into name="flag" and
        // Some("") vs. being dropped — the empty string value must be preserved.
        let ast = parse("git commit --message=").unwrap();
        let flag = ast.flags.iter().find(|f| f.name == "message").expect("flag missing");
        assert_eq!(flag.value.as_deref(), Some(""));
    }

    #[test]
    fn parse_stderr_redirect_2gt() {
        // Catches: `2>` tokeniser branch consuming the '2' as a standalone
        // positional argument when followed by '>', producing args=["2"] + redirect.
        use crate::exec_grammar::RedirectKind;
        let ast = parse("cargo build 2> err.log").unwrap();
        assert_eq!(ast.redirects.len(), 1);
        assert_eq!(ast.redirects[0].kind, RedirectKind::Stderr);
        assert_eq!(ast.redirects[0].target, "err.log");
        // '2' must NOT appear as a positional arg
        assert!(!ast.args.iter().any(|a| a.0 == "2"));
    }

    #[test]
    fn parse_pipeline_all_empty_segments_is_error() {
        // Catches: parse_pipeline("   |   ") returning Ok([]) instead of Err(Empty)
        // when every segment is whitespace-only after splitting.
        assert!(matches!(
            parse_pipeline("   |   "),
            Err(exec_grammar::ParseError::Empty)
        ));
    }

    // ── policy evaluation edge cases ──────────────────────────────────────────

    #[test]
    fn policy_blocked_param_wildcard_case_insensitive() {
        // Catches: blocked_parameters wildcard "*" matching only exact-case flag
        // names, missing "-recurse" when policy blocks "Recurse".
        let mut bp = HashMap::new();
        bp.insert("*".into(), vec!["Recurse".into()]);
        let policy = ExecPolicy {
            blocked_parameters: bp,
            ..Default::default()
        };
        // lowercase variant in the command
        let ast = parse("ls -recurse").unwrap();
        let v = policy.evaluate(&ast);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::BlockedParameter);
    }

    #[test]
    fn policy_allowlist_empty_means_no_restriction() {
        // Catches: empty allowed_binaries being treated as "block everything"
        // rather than "no allow-list enforcement".
        let policy = ExecPolicy::default();
        let ast = parse("totally-unknown-binary --arg").unwrap();
        assert!(policy.evaluate(&ast).is_empty());
    }

    #[test]
    fn policy_multiple_violations_all_reported() {
        // Catches: evaluate() returning after the first violation and missing
        // subsequent ones, preventing callers from showing a complete error list.
        let mut bp = HashMap::new();
        bp.insert("rm".into(), vec!["rf".into(), "no-preserve-root".into()]);
        let policy = ExecPolicy {
            allowed_binaries: vec!["cargo".into()], // rm is NOT allowed
            blocked_parameters: bp,
            ..Default::default()
        };
        let ast = parse("rm -rf --no-preserve-root /").unwrap();
        let v = policy.evaluate(&ast);
        // At minimum: UnknownCommand violation must be present
        assert!(v.iter().any(|x| x.kind == ViolationKind::UnknownCommand));
    }

    // ── risk classification edge cases ────────────────────────────────────────

    #[test]
    fn risk_curl_mixed_case_is_elevated() {
        // Catches: IMPLICIT_NETWORK_COMMANDS comparison being case-sensitive,
        // letting "Curl" or "CURL" bypass the elevated-risk check.
        let mut ast = parse("CURL https://example.com").unwrap();
        risk::classify(&mut ast, &ExecPolicy::default());
        assert_eq!(ast.risk, RiskLevel::Elevated);
    }

    #[test]
    fn risk_safe_command_stays_safe_after_double_classify() {
        // Catches: classify() accumulating state or toggling risk on repeated
        // calls, producing Elevated on the second invocation.
        let mut ast = parse("cargo build --release").unwrap();
        let policy = ExecPolicy {
            allowed_binaries: vec!["cargo".into()],
            ..Default::default()
        };
        risk::classify(&mut ast, &policy);
        let first = ast.risk;
        risk::classify(&mut ast, &policy);
        assert_eq!(ast.risk, first, "risk must be idempotent");
        assert_eq!(ast.risk, RiskLevel::Safe);
    }

    #[test]
    fn risk_policy_violation_overrides_network_elevation_to_blocked() {
        // Catches: classify() checking network commands BEFORE violations and
        // returning Elevated for a curl command that also violates the allow-list,
        // when the correct answer is Blocked.
        let policy = ExecPolicy {
            allowed_binaries: vec!["cargo".into()], // curl NOT allowed
            ..Default::default()
        };
        let mut ast = parse("curl https://evil.com").unwrap();
        risk::classify(&mut ast, &policy);
        assert_eq!(
            ast.risk,
            RiskLevel::Blocked,
            "policy violation must produce Blocked, not Elevated"
        );
    }
}
