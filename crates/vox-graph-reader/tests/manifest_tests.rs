// Tests for gui-content-manifest emission.
// The real emit function is implemented in Task G7; this file is the test fixture entry point.
// This smoke asserts only the function SIGNATURE (a compile/link gate). Until the G2 stub
// lands, this file fails to COMPILE (the symbol is undefined); once the stub exists, it
// compiles and passes. The real behavioral red→green is G7's golden test.

use std::path::Path;
use vox_graph_reader::manifest::emit_content_manifest;

#[test]
fn manifest_module_exists() {
    // Smoke: the module + signature are reachable (compile gate, not a behavioral assertion).
    let _ = emit_content_manifest
        as fn(&str, &str, &Path, &Path) -> Result<(), Box<dyn std::error::Error>>;
}
