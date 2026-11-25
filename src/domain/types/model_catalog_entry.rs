// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCatalogEntry {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "contextLength")]
    pub context_length: usize,
    #[serde(rename = "inputPerMillion")]
    pub input_per_million: f64,
    #[serde(rename = "outputPerMillion")]
    pub output_per_million: f64,
    #[serde(rename = "cachedPerMillion")]
    pub cached_per_million: f64,
    #[serde(rename = "createdUnix")]
    pub created_unix: u64,
}
