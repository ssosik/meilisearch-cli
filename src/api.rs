use crate::document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "q")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub sort: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "facetsDistribution")]
    pub facets_distribution: Option<Vec<String>>,
    #[serde(default)]
    pub limit: u32,
}

impl ApiQuery {
    pub fn new() -> Self {
        let mut q = ApiQuery {
            ..Default::default()
        };

        q.limit = 10000;

        q
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiResponse {
    pub hits: Vec<document::Document>,
    #[serde(rename = "nbHits")]
    pub num_hits: u32,
    #[serde(rename = "exhaustiveNbHits")]
    pub exhaustive_num_hits: bool,
    pub query: String,
    pub limit: u16,
    pub offset: u32,
    #[serde(rename = "processingTimeMs")]
    pub processing_time_ms: u32,
}
