use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TopLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopComplianceReport {
    pub data_citation: TopLevel,
    pub data_transparency: TopLevel,
    pub analysis_code_transparency: TopLevel,
    pub overall_level: TopLevel, // min of the three
}

impl TopComplianceReport {
    /// Assess TOP compliance from available artifact indicators.
    ///
    /// - Level 0: nothing
    /// - Level 1: data cited (has_data_doi)
    /// - Level 2: data + code (has_data_doi && has_code_doi)
    /// - Level 3: data + code + preregistration
    ///
    /// `overall_level` = min(data_citation, data_transparency, analysis_code_transparency)
    pub fn assess(has_data_doi: bool, has_code_doi: bool, has_preregistration: bool) -> Self {
        let data_citation = if has_data_doi {
            if has_preregistration {
                TopLevel::Level3
            } else {
                TopLevel::Level2
            }
        } else {
            TopLevel::Level0
        };

        let data_transparency = if has_data_doi && has_code_doi {
            if has_preregistration {
                TopLevel::Level3
            } else {
                TopLevel::Level2
            }
        } else if has_data_doi {
            TopLevel::Level1
        } else {
            TopLevel::Level0
        };

        let analysis_code_transparency = if has_code_doi {
            if has_preregistration {
                TopLevel::Level3
            } else {
                TopLevel::Level2
            }
        } else {
            TopLevel::Level0
        };

        // overall = min of the three dimensions
        let overall_level = data_citation
            .clone()
            .min(data_transparency.clone())
            .min(analysis_code_transparency.clone());

        TopComplianceReport {
            data_citation,
            data_transparency,
            analysis_code_transparency,
            overall_level,
        }
    }

    pub fn is_level2_or_above(&self) -> bool {
        self.overall_level >= TopLevel::Level2
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmBadge {
    pub name: String,  // e.g. "Artifacts Available"
    pub url: String, // e.g. "https://www.acm.org/publications/policies/artifact-review-and-badging-current"
    pub awarded: bool, // true if Zenodo deposit exists
}

pub fn acm_artifacts_available_badge(zenodo_doi: Option<&str>) -> AcmBadge {
    AcmBadge {
        name: "Artifacts Available".to_string(),
        url: "https://www.acm.org/publications/policies/artifact-review-and-badging-current"
            .to_string(),
        awarded: zenodo_doi.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_assets_is_level0() {
        let report = TopComplianceReport::assess(false, false, false);
        assert_eq!(report.overall_level, TopLevel::Level0);
        assert!(!report.is_level2_or_above());
    }

    #[test]
    fn data_and_code_is_level2() {
        let report = TopComplianceReport::assess(true, true, false);
        assert!(report.is_level2_or_above());
    }

    #[test]
    fn all_three_is_level3() {
        let report = TopComplianceReport::assess(true, true, true);
        assert_eq!(report.overall_level, TopLevel::Level3);
    }

    #[test]
    fn acm_badge_awarded_when_zenodo_doi_present() {
        let badge = acm_artifacts_available_badge(Some("10.5281/zenodo.12345"));
        assert!(badge.awarded);
        let no_badge = acm_artifacts_available_badge(None);
        assert!(!no_badge.awarded);
    }
}
