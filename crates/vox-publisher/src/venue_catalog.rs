//! Venue catalog: whitelisted publication venues + journal-fit recommender.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VenueType {
    Conference,
    Journal,
    WebNative,
    LivingReview,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewModel {
    DoubleBlind,
    Open,
    Editorial,
    Internal,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VenueEntry {
    pub id: String,
    pub name: String,
    pub shortname: String,
    #[serde(rename = "type")]
    pub venue_type: VenueType,
    pub primary: bool,
    pub focus: Vec<String>,
    pub review_model: ReviewModel,
    pub typical_deadline: Option<String>,
    pub min_pages: Option<u32>,
    pub max_pages: Option<u32>,
    pub target_atlas: Option<String>,
    #[serde(default)]
    pub living_review: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VenueCatalog {
    pub schema_version: u32,
    pub venues: Vec<VenueEntry>,
}

impl VenueCatalog {
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    pub fn primary_venues(&self) -> impl Iterator<Item = &VenueEntry> {
        self.venues.iter().filter(|v| v.primary)
    }

    pub fn find_by_id(&self, id: &str) -> Option<&VenueEntry> {
        self.venues.iter().find(|v| v.id == id)
    }

    pub fn whitelisted_venue_ids(&self) -> Vec<String> {
        self.venues.iter().map(|v| v.id.clone()).collect()
    }
}

/// Score of a single venue for a given set of finding topics.
#[derive(Debug, Clone, Serialize)]
pub struct VenueFitScore {
    pub venue_id: String,
    pub score: f64,
    pub reasons: Vec<String>,
}

/// Rank venues by topic overlap with a finding's focus areas.
pub struct JournalFitRecommender<'a> {
    catalog: &'a VenueCatalog,
}

impl<'a> JournalFitRecommender<'a> {
    pub fn new(catalog: &'a VenueCatalog) -> Self {
        Self { catalog }
    }

    /// Score each venue by overlap between `finding_topics` and `venue.focus`.
    /// Returns venues sorted descending by score (highest fit first).
    pub fn rank_venues(&self, finding_topics: &[&str]) -> Vec<VenueFitScore> {
        let mut scores: Vec<VenueFitScore> = self
            .catalog
            .venues
            .iter()
            .map(|v| {
                let matches: Vec<String> = finding_topics
                    .iter()
                    .filter(|t| v.focus.iter().any(|f| f == *t))
                    .map(|t| t.to_string())
                    .collect();
                let score = if v.focus.is_empty() {
                    0.0
                } else {
                    matches.len() as f64 / v.focus.len() as f64
                };
                VenueFitScore {
                    venue_id: v.id.clone(),
                    score,
                    reasons: matches,
                }
            })
            .collect();
        scores.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores
    }

    /// Return the best-fit venue entry, or None if no venues score > 0.
    pub fn top_venue(&self, finding_topics: &[&str]) -> Option<&'a VenueEntry> {
        let scores = self.rank_venues(finding_topics);
        scores
            .into_iter()
            .find(|s| s.score > 0.0)
            .and_then(|s| self.catalog.find_by_id(&s.venue_id))
    }

    /// True iff `venue_id` appears in the catalog whitelist.
    pub fn is_venue_whitelisted(&self, venue_id: &str) -> bool {
        self.catalog.find_by_id(venue_id).is_some()
    }

    /// Phase E wiring — rank venues with per-`FindingClass` boosting.
    ///
    /// Venues that appear in `defaults.policy_for(class).recommended_venues`
    /// get a fixed `+1.0` score boost and a `"class_recommended"` reason
    /// added to their `VenueFitScore`. The base topic-overlap score is
    /// still computed and combined, so a venue that matches the candidate
    /// class AND the topic list ranks highest.
    ///
    /// Atlas classes
    /// (`ModelCapabilityAtlas`, `ProviderReliabilityAtlas`) inherit the
    /// existing IMC/MLSys-shaped ranking; non-Atlas classes get the
    /// micro-track routing from `vox-class-routing`.
    pub fn rank_venues_for_class(
        &self,
        finding_topics: &[&str],
        class: vox_scientia::class_routing::FindingClass,
        defaults: &vox_scientia::class_routing::ClassDefaults,
    ) -> Vec<VenueFitScore> {
        const CLASS_BOOST: f64 = 1.0;
        let class_recommended: std::collections::HashSet<&str> =
            vox_scientia::class_routing::recommended_venues_for(defaults, class)
                .iter()
                .map(String::as_str)
                .collect();
        let mut scores = self.rank_venues(finding_topics);
        for s in &mut scores {
            if class_recommended.contains(s.venue_id.as_str()) {
                s.score += CLASS_BOOST;
                s.reasons.push("class_recommended".to_string());
            }
        }
        scores.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores
    }

    /// Phase E wiring — best-fit venue for a given class.
    pub fn top_venue_for_class(
        &self,
        finding_topics: &[&str],
        class: vox_scientia::class_routing::FindingClass,
        defaults: &vox_scientia::class_routing::ClassDefaults,
    ) -> Option<&'a VenueEntry> {
        self.rank_venues_for_class(finding_topics, class, defaults)
            .into_iter()
            .find(|s| s.score > 0.0)
            .and_then(|s| self.catalog.find_by_id(&s.venue_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = r#"
schema_version: 1
venues:
  - id: imc
    name: "Internet Measurement Conference"
    shortname: "IMC"
    type: conference
    primary: true
    focus: ["measurement", "provider-behavior"]
    review_model: double-blind
    typical_deadline: "June"
    min_pages: 12
    max_pages: 14
    target_atlas: provider-atlas
  - id: tmlr
    name: "Transactions on Machine Learning Research"
    shortname: "TMLR"
    type: journal
    primary: true
    focus: ["machine-learning", "empirical"]
    review_model: open
    typical_deadline: "rolling"
    min_pages: ~
    max_pages: ~
    target_atlas: ~
  - id: distill
    name: "Distill"
    shortname: "Distill"
    type: web-native
    primary: false
    focus: ["explanation"]
    review_model: editorial
    typical_deadline: ~
    min_pages: ~
    max_pages: ~
    target_atlas: ~
"#;

    #[test]
    fn parse_venue_catalog() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        assert_eq!(cat.schema_version, 1);
        assert_eq!(cat.venues.len(), 3);
        let imc = cat.find_by_id("imc").expect("imc");
        assert_eq!(imc.shortname, "IMC");
        assert_eq!(imc.venue_type, VenueType::Conference);
        assert!(imc.primary);
        assert_eq!(imc.min_pages, Some(12));
    }

    #[test]
    fn primary_venues_filter() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let primary: Vec<_> = cat.primary_venues().collect();
        assert_eq!(primary.len(), 2);
    }

    #[test]
    fn journal_fit_recommender_ranks_by_topic_overlap() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let rec = JournalFitRecommender::new(&cat);
        let scores = rec.rank_venues(&["provider-behavior", "measurement"]);
        assert!(!scores.is_empty());
        assert_eq!(scores[0].venue_id, "imc");
        assert!(scores[0].score > 0.0);
    }

    #[test]
    fn whitelist_check() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let rec = JournalFitRecommender::new(&cat);
        assert!(rec.is_venue_whitelisted("imc"));
        assert!(rec.is_venue_whitelisted("tmlr"));
        assert!(!rec.is_venue_whitelisted("predatory-journal-xyz"));
    }

    // ── Phase E wiring — class-aware ranking ─────────────────────────────

    /// Build a `ClassDefaults` map where `algorithmic_improvement` has
    /// `imc` as its recommended venue. Verifies class-boost logic.
    fn defaults_recommending(class: vox_scientia::class_routing::FindingClass, venue_id: &str)
        -> vox_scientia::class_routing::ClassDefaults
    {
        let mut d = vox_scientia::class_routing::builtin_class_defaults();
        d.by_class
            .entry(class.as_str().to_string())
            .and_modify(|p| p.recommended_venues = vec![venue_id.to_string()])
            .or_insert(vox_scientia::class_routing::ClassPolicy {
                reply_window_days: 7,
                negative_result_quota: 0,
                critic_allowed: true,
                recommended_venues: vec![venue_id.to_string()],
            });
        d
    }

    #[test]
    fn class_aware_rank_boosts_class_recommended_venue() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let rec = JournalFitRecommender::new(&cat);
        let defaults = defaults_recommending(
            vox_scientia::class_routing::FindingClass::AlgorithmicImprovement,
            "imc",
        );
        let scores = rec.rank_venues_for_class(
            &["measurement"],
            vox_scientia::class_routing::FindingClass::AlgorithmicImprovement,
            &defaults,
        );
        let imc = scores.iter().find(|s| s.venue_id == "imc").unwrap();
        assert!(
            imc.reasons.iter().any(|r| r == "class_recommended"),
            "imc should carry class_recommended reason; got {:?}",
            imc.reasons
        );
        // imc's score includes the +1.0 boost on top of its topic overlap
        // (1 match / 2 focus tags = 0.5 → 1.5 with boost). So it ranks
        // above tmlr (which only matches `machine-learning` of its 2 tags
        // when the topic is `measurement` → 0.0).
        assert!(imc.score >= 1.0);
        assert_eq!(scores.first().map(|s| s.venue_id.as_str()), Some("imc"));
    }

    #[test]
    fn class_aware_rank_falls_back_to_topic_only_when_class_has_no_recommendations() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let rec = JournalFitRecommender::new(&cat);
        // `Other` class has no recommended_venues in builtin defaults.
        let defaults = vox_scientia::class_routing::builtin_class_defaults();
        let scores = rec.rank_venues_for_class(
            &["machine-learning"],
            vox_scientia::class_routing::FindingClass::Other,
            &defaults,
        );
        // No venue should have `class_recommended` reason.
        for s in &scores {
            assert!(
                !s.reasons.iter().any(|r| r == "class_recommended"),
                "no class_recommended boost for `Other`; got {:?}",
                s
            );
        }
        // tmlr matches the topic, so it should rank first.
        assert_eq!(scores.first().map(|s| s.venue_id.as_str()), Some("tmlr"));
    }

    #[test]
    fn class_aware_top_venue_returns_class_recommended_when_available() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let rec = JournalFitRecommender::new(&cat);
        let defaults = defaults_recommending(
            vox_scientia::class_routing::FindingClass::ReproducibilityInfra,
            "tmlr",
        );
        // No topic match for `unrelated-topic`, but class recommendation
        // still surfaces tmlr.
        let top = rec.top_venue_for_class(
            &["unrelated-topic"],
            vox_scientia::class_routing::FindingClass::ReproducibilityInfra,
            &defaults,
        );
        assert!(top.is_some(), "class-recommended venue should surface");
        assert_eq!(top.unwrap().id, "tmlr");
    }

    #[test]
    fn class_aware_top_venue_returns_none_when_neither_match() {
        let cat = VenueCatalog::from_yaml(SAMPLE_YAML).expect("parse");
        let rec = JournalFitRecommender::new(&cat);
        // Empty defaults map for a class with no class-recommendations,
        // and no topic match.
        let defaults = vox_scientia::class_routing::ClassDefaults {
            by_class: Default::default(),
        };
        let top = rec.top_venue_for_class(
            &["unrelated-topic"],
            vox_scientia::class_routing::FindingClass::Other,
            &defaults,
        );
        assert!(top.is_none());
    }
}
