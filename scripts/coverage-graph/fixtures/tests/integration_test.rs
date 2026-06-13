// Integration test fixture
use fixture_lib::add;
use fixture_lib::untested_function;

#[test]
fn integration_add_works() {
    assert_eq!(add(10, 20), 30);
}

#[test]
fn integration_references_untested() {
    // just references untested_function by calling it, no assertion on return
    let _ = untested_function();
}
