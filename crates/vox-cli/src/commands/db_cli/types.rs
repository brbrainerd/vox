//! Shared CLI argument types for `vox db` publication flows.

/// Shared `publication-prepare` / `publication-prepare-validated` fields (no `content_type`).
pub use vox_cli_core::db_types::{
    ArxivHandoffStageCli, DbPreflightProfileCli, DiscoveryIntakeGateCli, PublicationPrepareBodyCli,
    ScholarlyVenueCli,
};

impl DbPreflightProfileCli {
    pub fn to_profile(self) -> vox_publisher::publication_preflight::PreflightProfile {
        match self {
            DbPreflightProfileCli::Default => {
                vox_publisher::publication_preflight::PreflightProfile::Default
            }
            DbPreflightProfileCli::DoubleBlind => {
                vox_publisher::publication_preflight::PreflightProfile::DoubleBlind
            }
            DbPreflightProfileCli::MetadataComplete => {
                vox_publisher::publication_preflight::PreflightProfile::MetadataComplete
            }
            DbPreflightProfileCli::ArxivAssist => {
                vox_publisher::publication_preflight::PreflightProfile::ArxivAssist
            }
        }
    }
}

impl DiscoveryIntakeGateCli {
    pub fn to_gate(self) -> vox_publisher::scientia_discovery::DiscoveryIntakeGate {
        match self {
            DiscoveryIntakeGateCli::None => {
                vox_publisher::scientia_discovery::DiscoveryIntakeGate::None
            }
            DiscoveryIntakeGateCli::StrongSignalsOnly => {
                vox_publisher::scientia_discovery::DiscoveryIntakeGate::StrongSignalsOnly
            }
            DiscoveryIntakeGateCli::AllowReviewSuggested => {
                vox_publisher::scientia_discovery::DiscoveryIntakeGate::AllowReviewSuggested
            }
        }
    }
}

impl ScholarlyVenueCli {
    pub fn to_venue(self) -> vox_publisher::submission::ScholarlyVenue {
        match self {
            ScholarlyVenueCli::Zenodo => vox_publisher::submission::ScholarlyVenue::Zenodo,
            ScholarlyVenueCli::OpenReview => vox_publisher::submission::ScholarlyVenue::OpenReview,
            ScholarlyVenueCli::ArxivAssist => {
                vox_publisher::submission::ScholarlyVenue::ArxivAssist
            }
        }
    }
}

impl ArxivHandoffStageCli {
    pub fn slug(self) -> &'static str {
        match self {
            Self::StagingExported => "staging_exported",
            Self::OperatorAck => "operator_ack",
            Self::BundleValidated => "bundle_validated",
            Self::Submitted => "submitted",
            Self::Published => "published",
        }
    }
}
