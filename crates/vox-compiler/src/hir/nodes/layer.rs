//! HIR types for VUV's layered layout discipline (GA-26).
//!
//! See [`docs/src/architecture/vuv-layered-layout-discipline-2026.md`](../../../../docs/src/architecture/vuv-layered-layout-discipline-2026.md)
//! for design rationale. Adopts `wlr-layer-shell`'s closed-enum tier model
//! plus i3/Sway's typed-jump-target convention.

use crate::ast::span::Span;

/// The seven canonical Z-tiers for VUV view trees.
///
/// Ordering is total and fixed. Within-tier ordering is **deliberately
/// unspecified** — designs that depend on it are explicitly broken (per the
/// `wlr-layer-shell` precedent).
///
/// New tiers cannot be added by user code. The closed enum is part of the
/// language contract; opening it would be a P0 / C4 regression.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum LayerTier {
    /// Wallpapers, backdrops, decorative non-interactive elements.
    Background = 0,
    /// The main view tree. Default for component renders.
    Content = 1,
    /// App shell: nav rail, status bar, tab strip — persistent chrome above
    /// content but subordinate to overlays.
    Chrome = 2,
    /// Non-modal overlays anchored to a target: `Tooltip`, `Menu`, `ComboboxList`.
    Popover = 3,
    /// Interaction-blocking dialogs: `Dialog`, `AlertDialog`, `ConfirmDialog`.
    Modal = 4,
    /// Transient self-dismissing notifications: `Snackbar`, `Banner`.
    Toast = 5,
    /// Reserved escape hatch: debug overlays, accessibility cursor, focus ring.
    /// User code cannot construct surfaces at this tier.
    SystemOverlay = 6,
}

impl LayerTier {
    /// Return the canonical short name used in CSS portal selectors and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            LayerTier::Background => "background",
            LayerTier::Content => "content",
            LayerTier::Chrome => "chrome",
            LayerTier::Popover => "popover",
            LayerTier::Modal => "modal",
            LayerTier::Toast => "toast",
            LayerTier::SystemOverlay => "system-overlay",
        }
    }

    /// Parse a tier from its short name (used by the parser when lowering
    /// `@layer(tier: …)`). Returns `None` for unknown names.
    ///
    /// Suppress `clippy::should_implement_trait`: this returns `Option<Self>`,
    /// not `Result<Self, _>`, so it does not fit the `FromStr` shape; the
    /// `from_str` name is kept for parity with the surrounding parser surface.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "background" | "Background" => Some(LayerTier::Background),
            "content" | "Content" => Some(LayerTier::Content),
            "chrome" | "Chrome" => Some(LayerTier::Chrome),
            "popover" | "Popover" => Some(LayerTier::Popover),
            "modal" | "Modal" => Some(LayerTier::Modal),
            "toast" | "Toast" => Some(LayerTier::Toast),
            "system-overlay" | "system_overlay" | "SystemOverlay" => Some(LayerTier::SystemOverlay),
            _ => None,
        }
    }

    /// Return `true` if a surface at `child_tier` can legally be rendered as
    /// a child of a surface at `self`.
    ///
    /// Rule: a child's tier must be `<=` the parent's tier. A *stronger*
    /// surface (higher tier) cannot be the child of a *weaker* one — e.g., a
    /// `Modal` inside a `Tooltip` is structurally wrong because the Tooltip
    /// will dismiss itself with its target, orphaning the Modal it parents.
    /// The Tooltip belongs *above* the Modal it annotates, not the other way
    /// around — and the only way to get that ordering is to declare them as
    /// siblings, not parent/child.
    pub fn allows_child(self, child_tier: LayerTier) -> bool {
        child_tier <= self
    }

    /// Default tier for a typed UI primitive.
    ///
    /// Centralised here so primitive emitters and the parser converge on the
    /// same tier without re-deriving the rule.
    pub fn default_for_primitive(name: &str) -> Self {
        match name {
            "Tooltip" | "Menu" | "ComboboxList" | "PopoverContent" => LayerTier::Popover,
            "Dialog" | "AlertDialog" | "ConfirmDialog" | "Modal" => LayerTier::Modal,
            "Toast" | "Snackbar" | "Banner" => LayerTier::Toast,
            "AppShell" | "NavRail" | "StatusBar" | "TabStrip" => LayerTier::Chrome,
            "Wallpaper" | "Backdrop" => LayerTier::Background,
            _ => LayerTier::Content,
        }
    }
}

/// A `@layer(tier: …)` decorator on a component declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirLayerDecl {
    pub tier: LayerTier,
    pub span: Span,
}

/// A typed cross-tree jump target — i3/Sway's `mark` ported to VUV.
///
/// Marks are unique within a view-tree scope. A `Mark<"checkout-button">`
/// is a compile-checked handle for cross-tree focus, scroll-anchor,
/// tooltip-target, and modal return-focus operations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirMark {
    /// The mark's literal label (string-typed, e.g., `"primary-action"`).
    pub label: String,
    /// Source span where the mark is declared.
    pub span: Span,
}

/// A reference to a previously-declared mark — used by `Tooltip::for(target: Mark<"…">)`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirMarkRef {
    /// The label being referenced.
    pub label: String,
    /// Span of the reference (not the original declaration).
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_ordering_is_total_and_stable() {
        assert!(LayerTier::Background < LayerTier::Content);
        assert!(LayerTier::Content < LayerTier::Chrome);
        assert!(LayerTier::Chrome < LayerTier::Popover);
        assert!(LayerTier::Popover < LayerTier::Modal);
        assert!(LayerTier::Modal < LayerTier::Toast);
        assert!(LayerTier::Toast < LayerTier::SystemOverlay);
    }

    #[test]
    fn modal_cannot_nest_in_tooltip() {
        // Popover (Tooltip) cannot host Modal — Modal is a stronger tier.
        assert!(!LayerTier::Popover.allows_child(LayerTier::Modal));
        // But a Modal inside a Modal is fine (same tier).
        assert!(LayerTier::Modal.allows_child(LayerTier::Modal));
        // A Tooltip (Popover) inside a Modal is fine — tooltips for modal
        // content are weaker than the modal they annotate.
        assert!(LayerTier::Modal.allows_child(LayerTier::Popover));
    }

    #[test]
    fn strong_parent_can_host_anything_at_or_below_its_tier() {
        // Toast (5) can host anything weaker.
        assert!(LayerTier::Toast.allows_child(LayerTier::Modal));
        assert!(LayerTier::Toast.allows_child(LayerTier::Content));
        assert!(LayerTier::Toast.allows_child(LayerTier::Background));
        // But not stronger (SystemOverlay).
        assert!(!LayerTier::Toast.allows_child(LayerTier::SystemOverlay));

        // Content (1) cannot host stronger surfaces — they must be portaled
        // up to their declared tier, not nested as children of Content.
        assert!(!LayerTier::Content.allows_child(LayerTier::Modal));
        assert!(!LayerTier::Content.allows_child(LayerTier::Popover));
    }

    #[test]
    fn from_str_round_trip() {
        for tier in [
            LayerTier::Background,
            LayerTier::Content,
            LayerTier::Chrome,
            LayerTier::Popover,
            LayerTier::Modal,
            LayerTier::Toast,
            LayerTier::SystemOverlay,
        ] {
            assert_eq!(LayerTier::from_str(tier.as_str()), Some(tier));
        }
    }

    #[test]
    fn default_for_primitive_maps_known_names() {
        assert_eq!(
            LayerTier::default_for_primitive("Tooltip"),
            LayerTier::Popover
        );
        assert_eq!(LayerTier::default_for_primitive("Dialog"), LayerTier::Modal);
        assert_eq!(LayerTier::default_for_primitive("Toast"), LayerTier::Toast);
        assert_eq!(
            LayerTier::default_for_primitive("MyCustomThing"),
            LayerTier::Content
        );
    }
}
