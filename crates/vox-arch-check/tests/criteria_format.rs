//! CR-META lint integration tests — exercises the public
//! `vox_arch_check::criteria_format::check_criteria_format` API and asserts the
//! live `v1-release-criteria.md` self-passes.

use vox_arch_check::criteria_format::check_criteria_format;

#[test]
fn flags_block_missing_if_failing() {
    let doc = "\
**[CR-X] Some criterion.** Foo.
- `verify_cmd`: `cargo run -p vox-audit -- foo`
- `artifact_path`: `contracts/reports/foo/<UTC>.json`
";
    let errs = check_criteria_format(doc).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| e.contains("CR-X") && e.contains("if_failing")),
        "expected a CR-X / if_failing error; got {errs:?}"
    );
}

#[test]
fn well_formed_block_passes() {
    let doc = "\
**[CR-Y] Good.** Bar.
- `verify_cmd`: `cargo run -p vox-audit -- bar`
- `artifact_path`: `contracts/reports/bar/<UTC>.json`
- `if_failing`: do the thing.
";
    assert!(
        check_criteria_format(doc).is_ok(),
        "{:?}",
        check_criteria_format(doc)
    );
}

#[test]
fn real_criteria_doc_is_well_formed() {
    let root = env!("CARGO_MANIFEST_DIR");
    let path =
        std::path::Path::new(root).join("../../docs/src/architecture/v1-release-criteria.md");
    let doc = std::fs::read_to_string(&path).expect("read criteria doc");
    let res = check_criteria_format(&doc);
    assert!(
        res.is_ok(),
        "live criteria doc must self-pass CR-META: {res:?}"
    );
}
