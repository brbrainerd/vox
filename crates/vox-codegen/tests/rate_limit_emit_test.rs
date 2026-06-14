/// Task 4D: @rate_limit by:user_id and by:api_key Rust codegen
///
/// Previously only RateLimitBy::Ip was handled in wrap_method_router.
/// This test verifies the enum variants are accessible and the crate compiles
/// with them referenced. The emit functions are private; compile success
/// (all three match arms present in http.rs) is the primary regression guard.
use vox_compiler::hir::http_ergonomics::RateLimitBy;

#[test]
fn rate_limit_by_variants_are_exhaustive() {
    // Ensure the three discriminants exist and are distinguishable.
    let ip = RateLimitBy::Ip;
    let uid = RateLimitBy::UserId;
    let key = RateLimitBy::ApiKey;
    assert_ne!(ip, uid);
    assert_ne!(ip, key);
    assert_ne!(uid, key);
}
