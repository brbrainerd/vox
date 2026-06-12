#![no_main]

use libfuzzer_sys::fuzz_target;

/// libFuzzer harness: lex + parse arbitrary UTF-8 input; must not panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        let tokens = vox_compiler::lexer::lex(source);
        let _ = vox_compiler::parser::parse(tokens);
    }
});
