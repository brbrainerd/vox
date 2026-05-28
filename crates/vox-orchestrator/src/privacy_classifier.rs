//! Sensitivity classifier and privacy-level-aware routing (D8).
//!
//! `PrivacyClassifier` maps structured detection signals to a [`PrivacyLevel`].
//! Adds `route_for_level` on top of the existing [`crate::privacy_router::PrivacyRouter`]
//! so callers can route by level rather than raw PII bool.
//! All logic is pure: no async, no I/O.

use serde::{Deserialize, Serialize};

use crate::privacy_router::{PrivacyLevel, PrivacyRouter, PrivacyRoutingDecision};

/// Structured detection signals used to classify privacy level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationSignals {
    /// True when a PII scanner (name, email, phone, SSN …) flagged the content.
    pub pii_detected: bool,
    /// True when a regulatory-keyword scanner found HIPAA / GDPR / PCI markers.
    pub regulated_marker_detected: bool,
    /// True when the content is sourced from a public, non-authenticated origin.
    pub public_source: bool,
    /// True when the content is explicitly marked internal (e.g. internal wiki, internal doc).
    pub internal_marker: bool,
}

/// Thresholds; currently no numeric thresholds — classification is rule-based.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivacyClassifierConfig;

/// Pure privacy-level classifier.
pub struct PrivacyClassifier {
    #[allow(dead_code)]
    config: PrivacyClassifierConfig,
}

impl PrivacyClassifier {
    pub fn new(config: PrivacyClassifierConfig) -> Self {
        Self { config }
    }

    /// Classify signals into a [`PrivacyLevel`].
    ///
    /// Resolution order (first match wins):
    /// 1. `regulated_marker_detected` → `Regulated`
    /// 2. `pii_detected` → `Private`
    /// 3. `internal_marker` → `Internal`
    /// 4. `public_source` → `Public`
    /// 5. Default → `Internal` (safe fallback)
    #[must_use]
    #[inline]
    pub fn classify(&self, signals: &ClassificationSignals) -> PrivacyLevel {
        if signals.regulated_marker_detected {
            return PrivacyLevel::Regulated;
        }
        if signals.pii_detected {
            return PrivacyLevel::Private;
        }
        if signals.internal_marker {
            return PrivacyLevel::Internal;
        }
        if signals.public_source {
            return PrivacyLevel::Public;
        }
        PrivacyLevel::Internal
    }
}

/// Route by [`PrivacyLevel`], extending the existing [`PrivacyRouter`].
///
/// Separated from the existing `route(pii_detected: bool)` method to keep the
/// original behaviour stable while adding level-aware routing.
#[must_use]
pub fn route_for_level(router: &PrivacyRouter, level: PrivacyLevel) -> PrivacyRoutingDecision {
    match level {
        PrivacyLevel::Regulated => PrivacyRoutingDecision::LocalOnly,
        PrivacyLevel::Private => {
            if router.policy.force_local_for_private {
                PrivacyRoutingDecision::LocalOnly
            } else {
                PrivacyRoutingDecision::Redact
            }
        }
        PrivacyLevel::Internal => PrivacyRoutingDecision::Redact,
        PrivacyLevel::Public => PrivacyRoutingDecision::Allowed,
    }
}

/// Metric payload emitted when a privacy route decision is made.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct PrivacyRouteEvent {
    pub metric_type: &'static str,
    pub privacy_level: String,
    pub routing_decision: String,
    pub session_id: Option<String>,
}

impl PrivacyRouteEvent {
    pub fn new(
        level: PrivacyLevel,
        decision: PrivacyRoutingDecision,
        session_id: Option<String>,
    ) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_PRIVACY_ROUTE_DECISION,
            privacy_level: level.to_string(),
            routing_decision: decision.to_string(),
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_router::PrivacyRoutingPolicy;

    fn classifier() -> PrivacyClassifier {
        PrivacyClassifier::new(PrivacyClassifierConfig)
    }

    fn router_force_local() -> PrivacyRouter {
        PrivacyRouter::new(PrivacyRoutingPolicy {
            force_local_for_private: true,
            ..Default::default()
        })
    }

    fn router_no_force_local() -> PrivacyRouter {
        PrivacyRouter::new(PrivacyRoutingPolicy {
            force_local_for_private: false,
            ..Default::default()
        })
    }

    #[test]
    fn regulated_marker_classifies_regulated() {
        let c = classifier();
        let sig = ClassificationSignals {
            regulated_marker_detected: true,
            ..Default::default()
        };
        assert_eq!(c.classify(&sig), PrivacyLevel::Regulated);
    }

    #[test]
    fn pii_without_regulated_classifies_private() {
        let c = classifier();
        let sig = ClassificationSignals {
            pii_detected: true,
            ..Default::default()
        };
        assert_eq!(c.classify(&sig), PrivacyLevel::Private);
    }

    #[test]
    fn regulated_wins_over_pii() {
        let c = classifier();
        let sig = ClassificationSignals {
            pii_detected: true,
            regulated_marker_detected: true,
            ..Default::default()
        };
        assert_eq!(c.classify(&sig), PrivacyLevel::Regulated);
    }

    #[test]
    fn internal_marker_classifies_internal() {
        let c = classifier();
        let sig = ClassificationSignals {
            internal_marker: true,
            ..Default::default()
        };
        assert_eq!(c.classify(&sig), PrivacyLevel::Internal);
    }

    #[test]
    fn public_source_classifies_public() {
        let c = classifier();
        let sig = ClassificationSignals {
            public_source: true,
            ..Default::default()
        };
        assert_eq!(c.classify(&sig), PrivacyLevel::Public);
    }

    #[test]
    fn no_signals_defaults_to_internal() {
        let c = classifier();
        assert_eq!(
            c.classify(&ClassificationSignals::default()),
            PrivacyLevel::Internal
        );
    }

    #[test]
    fn regulated_routes_local_only() {
        let r = router_force_local();
        assert_eq!(
            route_for_level(&r, PrivacyLevel::Regulated),
            PrivacyRoutingDecision::LocalOnly
        );
    }

    #[test]
    fn private_with_force_local_routes_local_only() {
        let r = router_force_local();
        assert_eq!(
            route_for_level(&r, PrivacyLevel::Private),
            PrivacyRoutingDecision::LocalOnly
        );
    }

    #[test]
    fn private_without_force_local_routes_redact() {
        let r = router_no_force_local();
        assert_eq!(
            route_for_level(&r, PrivacyLevel::Private),
            PrivacyRoutingDecision::Redact
        );
    }

    #[test]
    fn internal_routes_redact() {
        let r = router_force_local();
        assert_eq!(
            route_for_level(&r, PrivacyLevel::Internal),
            PrivacyRoutingDecision::Redact
        );
    }

    #[test]
    fn public_routes_allowed() {
        let r = router_force_local();
        assert_eq!(
            route_for_level(&r, PrivacyLevel::Public),
            PrivacyRoutingDecision::Allowed
        );
    }

    #[test]
    fn privacy_route_event_has_correct_metric_type() {
        let ev =
            PrivacyRouteEvent::new(PrivacyLevel::Public, PrivacyRoutingDecision::Allowed, None);
        assert_eq!(ev.metric_type, "orch.privacy.route_decision");
    }
}
