#![allow(unsafe_code)]

use std::ffi::OsString;
use std::sync::Mutex;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

/// Serializes `VOX_WEBIR_VALIDATE` emitter tests — env is process-global.
static WEBIR_VALIDATE_EMITTER_LOCK: Mutex<()> = Mutex::new(());

/// OP-S026 / OP-S025: `generate_with_options` runs WebIR validate by default (`VOX_WEBIR_VALIDATE` unset).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn codegen_emitter_honors_vox_webir_validate_success_path() {
    let _lock = WEBIR_VALIDATE_EMITTER_LOCK
        .lock()
        .expect("WEBIR_VALIDATE_EMITTER_LOCK poisoned");
    const KEY: &str = "VOX_WEBIR_VALIDATE";
    struct Guard {
        prev: Option<OsString>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(KEY, v) },
                None => unsafe { std::env::remove_var(KEY) },
            }
        }
    }
    let prev = std::env::var_os(KEY);
    unsafe {
        std::env::remove_var(KEY);
    }
    let _guard = Guard { prev };

    let source = r#"
component Home() {
    state n: int = 0
    view: text()
}
routes {
    "/" to Home
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("codegen");
    assert!(
        out.files.iter().any(|(n, _)| n == "Home.tsx"),
        "{:?}",
        out.files.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
}

/// OP-S028: validate-on path rejects literal CSS color values in `style {}` blocks (TASK-5.1).
/// `routes { }` blocks are silently dropped (Path B decommission, TASK-2.1); the validation gate
/// is tested via the literal-color-value error instead.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn codegen_emitter_vox_webir_validate_fails_on_literal_style_color() {
    let _lock = WEBIR_VALIDATE_EMITTER_LOCK
        .lock()
        .expect("WEBIR_VALIDATE_EMITTER_LOCK poisoned");
    const KEY: &str = "VOX_WEBIR_VALIDATE";
    struct Guard {
        prev: Option<OsString>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(KEY, v) },
                None => unsafe { std::env::remove_var(KEY) },
            }
        }
    }
    let prev = std::env::var_os(KEY);
    unsafe { std::env::set_var(KEY, "1") };
    let _guard = Guard { prev };

    let source = r#"
component A() {
    view: column(raw_class="x") { "hello" }
}
style {
    .x { color: "red" }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let err = match generate_with_options(&hir, CodegenOptions::default()) {
        Ok(_) => panic!("expected WebIR validate failure"),
        Err(e) => e,
    };
    assert!(
        err.contains("VOX_WEBIR_VALIDATE") && err.contains("web_ir_validate."),
        "{err}"
    );
}
