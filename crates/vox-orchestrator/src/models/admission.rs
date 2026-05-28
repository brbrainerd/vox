use crate::models::spec::{ModelSpec, PricingSource};
use vox_db::VoxDb;

/// Evaluates if models with PricingSource::Unknown have gathered enough telemetry
/// to be promoted to PricingSource::Telemetry.
pub struct ModelAdmissionFilter;

impl ModelAdmissionFilter {
    /// Checks the database telemetry to see if any unknown models can be promoted.
    /// Returns the number of models that were promoted.
    pub async fn promote_calibrated_models(db: &VoxDb, models: &mut Vec<ModelSpec>) -> anyhow::Result<usize> {
        let pricing = db.get_pricing_catalog().await?;
        let mut promoted_count = 0;

        for m in models.iter_mut() {
            if m.pricing_source == PricingSource::Unknown {
                if let Some(row) = pricing.iter().find(|r| r.model_id == m.id) {
                    if row.confidence == "high" || row.confidence == "medium" || row.n_provider_reported >= 10 {
                        m.pricing_source = PricingSource::Telemetry;
                        if let Some(blended) = row.observed_blended_per_1k {
                            m.cost_per_1k = blended;
                        }
                        promoted_count += 1;
                    }
                }
            }
        }
        Ok(promoted_count)
    }
}
