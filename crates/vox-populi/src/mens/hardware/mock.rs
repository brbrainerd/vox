use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use crate::mens::hardware::types::HardwareSummary;
use async_trait::async_trait;

pub(crate) struct MockProbe {
    pub name: &'static str,
    pub applicable: bool,
    pub result: Result<Option<HardwareSummary>, ProbeError>,
}

#[async_trait]
impl HardwareProbe for MockProbe {
    fn name(&self) -> &'static str {
        self.name
    }
    fn applicable(&self) -> bool {
        self.applicable
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        self.result.clone()
    }
}
