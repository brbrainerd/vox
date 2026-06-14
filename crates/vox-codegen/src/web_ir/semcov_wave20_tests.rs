//! Adversarial semantic-coverage tests — wave 20 — targeting pure functions in
//! `web_ir`: ZTier z-value arithmetic and string round-trips.

#[cfg(test)]
mod semcov_wave20_tests {
    use crate::web_ir::ZTier;

    // -----------------------------------------------------------------------
    // ZTier — pure z-index arithmetic and parsing
    // -----------------------------------------------------------------------

    #[test]
    fn ztier_background_z_value_is_zero() {
        // Catches: off-by-one in the tier * 100 formula where Background has
        // discriminant 0 but some +1 shift increments it.
        assert_eq!(ZTier::Background.z_value(), 0);
    }

    #[test]
    fn ztier_z_values_are_strictly_monotone() {
        // Invariant: every tier must have a strictly higher z than the previous
        // one.  Equality between any two tiers would violate the layer discipline.
        let tiers = [
            ZTier::Background,
            ZTier::Content,
            ZTier::Chrome,
            ZTier::Popover,
            ZTier::Modal,
            ZTier::Toast,
            ZTier::SystemOverlay,
        ];
        for pair in tiers.windows(2) {
            assert!(
                pair[0].z_value() < pair[1].z_value(),
                "{:?} z={} must be < {:?} z={}",
                pair[0],
                pair[0].z_value(),
                pair[1],
                pair[1].z_value()
            );
        }
    }

    #[test]
    fn ztier_system_overlay_has_highest_z() {
        // Catches: any variant accidentally placed above SystemOverlay in the
        // enum definition (which would make its z_value dominate).
        let all = [
            ZTier::Background,
            ZTier::Content,
            ZTier::Chrome,
            ZTier::Popover,
            ZTier::Modal,
            ZTier::Toast,
            ZTier::SystemOverlay,
        ];
        let max = all.iter().map(|t| t.z_value()).max().unwrap();
        assert_eq!(ZTier::SystemOverlay.z_value(), max);
    }

    #[test]
    fn ztier_from_str_roundtrips_all_variants() {
        // Catches: any variant missing from the from_str match arm — a silent
        // None means the validator can never recover it from a string token.
        let cases = [
            ("background", ZTier::Background),
            ("content", ZTier::Content),
            ("chrome", ZTier::Chrome),
            ("popover", ZTier::Popover),
            ("modal", ZTier::Modal),
            ("toast", ZTier::Toast),
            ("system_overlay", ZTier::SystemOverlay),
        ];
        for (s, expected) in cases {
            assert_eq!(
                ZTier::from_str(s),
                Some(expected),
                "from_str(\"{s}\") returned None"
            );
        }
    }

    #[test]
    fn ztier_from_str_rejects_empty_string() {
        // Catches: from_str matching on an empty string and returning Some(…)
        // because a catch-all arm was added by mistake.
        assert_eq!(ZTier::from_str(""), None);
    }

    #[test]
    fn ztier_from_str_rejects_wrong_case() {
        // Catches: case-insensitive matching accidentally enabled — variant
        // names in the protocol are snake_case lowercase only.
        assert_eq!(ZTier::from_str("MODAL"), None);
        assert_eq!(ZTier::from_str("Modal"), None);
        assert_eq!(ZTier::from_str("System_Overlay"), None);
        assert_eq!(ZTier::from_str("SYSTEM_OVERLAY"), None);
    }

    #[test]
    fn ztier_from_str_rejects_space_separated_variant() {
        // Catches: "system overlay" (space instead of underscore) being accepted
        // due to a normalization step that should not exist.
        assert_eq!(ZTier::from_str("system overlay"), None);
    }

    #[test]
    fn ztier_to_str_roundtrips_from_str() {
        // Invariant: to_str() output must be accepted by from_str() for every
        // variant.  A mismatch breaks the data-vox-layer attribute cycle
        // (emit → parse → validate).
        let tiers = [
            ZTier::Background,
            ZTier::Content,
            ZTier::Chrome,
            ZTier::Popover,
            ZTier::Modal,
            ZTier::Toast,
            ZTier::SystemOverlay,
        ];
        for t in tiers {
            let s = t.to_str();
            assert_eq!(
                ZTier::from_str(s),
                Some(t),
                "from_str(to_str({t:?})) failed for \"{s}\""
            );
        }
    }

    #[test]
    fn ztier_z_value_increments_by_100_per_tier() {
        // Catches: the formula changing from `discriminant * 100` to something
        // non-uniform like a lookup table with a typo in one entry.
        let tiers = [
            ZTier::Background,
            ZTier::Content,
            ZTier::Chrome,
            ZTier::Popover,
            ZTier::Modal,
            ZTier::Toast,
            ZTier::SystemOverlay,
        ];
        for (i, t) in tiers.iter().enumerate() {
            assert_eq!(
                t.z_value(),
                (i as i32) * 100,
                "{t:?} should have z={} but got {}",
                i * 100,
                t.z_value()
            );
        }
    }
}
