#![no_main]

use libfuzzer_sys::fuzz_target;

/// Feed arbitrary bytes into the declaration parser entry point; must not panic.
fuzz_target!(|data: &[u8]| {
    vox_compiler::parser::fuzz_parse_decl_bytes(data);
});
