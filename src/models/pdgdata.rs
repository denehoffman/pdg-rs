use crate::{LimitType, PdgId, ValueType};

#[derive(Clone, Debug)]
pub struct DataEntry {
    pub pdgid: PdgId,
    pub edition: String,
    pub value_type: ValueType,
    pub confidence_level: Option<f64>,
    pub limit_type: Option<LimitType>,
    pub comment: Option<String>,
    pub value: Option<f64>,
    pub error_positive: Option<f64>,
    pub error_negative: Option<f64>,
    pub scale_factor: Option<f64>,
    pub unit_text: String,
}
