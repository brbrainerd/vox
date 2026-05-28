//! End-to-end integration tests for the Vox compiler pipeline.
//! These tests lex → parse → typecheck → codegen the chatbot example
//! and verify the output is correct.
#![allow(unsafe_code)] // `std::env::{set_var,remove_var}` for opt-in Express codegen tests

use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;

use vox_bounded_fs::read_utf8_path_capped;
use vox_codegen::codegen_ts::{CodegenOptions, generate, generate_with_options};
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_module;

/// Serializes all tests that read or write `VOX_EMIT_EXPRESS_SERVER`.
/// Without this, parallel test runners observe the env-var mid-mutation,
/// causing `assert!(!server.ts)` tests to see a stale `=1` value.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Sets `VOX_EMIT_EXPRESS_SERVER=1` for the duration of `f`, then restores the prior value.
/// Holds [`ENV_MUTEX`] for the entire call so parallel tests see a stable env.
fn with_express_server_enabled<R>(f: impl FnOnce() -> R) -> R {
    let _env_guard = ENV_MUTEX.lock().expect("ENV_MUTEX poisoned");
    const KEY: &str = "VOX_EMIT_EXPRESS_SERVER";
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
        std::env::set_var(KEY, "1");
    }
    let _guard = Guard { prev };
    f()
}

/// Sets `VOX_WEBIR_VALIDATE=0` to skip the Web IR validate gate (default is on), then restores the prior value.
/// Uses [`ENV_MUTEX`] so tests do not race codegen.
fn with_web_ir_validate_cleared<R>(f: impl FnOnce() -> R) -> R {
    let _env_guard = ENV_MUTEX.lock().expect("ENV_MUTEX poisoned");
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
        std::env::set_var(KEY, "0");
    }
    let _guard = Guard { prev };
    f()
}

/// Sets `VOX_WEBIR_VALIDATE=1` for the duration of `f`, then restores the prior value.
fn with_web_ir_validate_enabled<R>(f: impl FnOnce() -> R) -> R {
    let _env_guard = ENV_MUTEX.lock().expect("ENV_MUTEX poisoned");
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
        std::env::set_var(KEY, "1");
    }
    let _guard = Guard { prev };
    f()
}

/// Call `generate()` while holding [`ENV_MUTEX`], ensuring the env-var is NOT set.
/// Prevents `codegen_server_has_express_route_with_await` from racing past "without express" tests.
macro_rules! generate_without_express {
    ($module:expr) => {{
        let _env_guard = ENV_MUTEX.lock().expect("ENV_MUTEX poisoned");
        let hir = vox_compiler::hir::lower_module($module);
        generate(&hir).expect("Should generate without errors")
    }};
}

const CHATBOT_SRC: &str = r#"import react.use_state

type ChatResult =
    | Ok(text: str)
    | Error(message: str)

component Chat() {
    let (messages, set_messages) = use_state([{role: "bot", text: ""}])
    let (input, set_input) = use_state("")
    let send = fn(_e) set_messages(messages.append({role: "user", text: input}))
    view: column(raw_class="chat-container") {
        column(raw_class="chat-title") { "Vox Chatbot" }
        button(raw_class="send-btn", on_click=send) { "Send" }
    }
}

@server fn claude_send(msg: str) to ChatResult {
    Ok("ok")
}

@mutation fn api_chat(prompt: str) to ChatResult {
    return Ok("ok")
}
"#;

include!("pipeline/includes/include_01.rs");
include!("pipeline/includes/include_02.rs");
include!("pipeline/includes/include_03.rs");
include!("pipeline/includes/blueprint_op_s_batch.rs");
include!("pipeline/includes/include_04.rs");
