use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IndicatorType {
    Ipv4,
    Ipv6,
    Domain,
    Url,
    Email,
    Md5,
    Sha1,
    Sha256,
    Cve,
    MitreAttackTechnique,
    OnionDomain,
    OnionUrl,
    CryptoWallet,
    CloudAccessKey,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct ExtractionField<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone)]
pub struct ExtractionInput<'a> {
    pub content_item_id: Option<i64>,
    pub alert_id: Option<i64>,
    pub feed_id: Option<i64>,
    pub fields: Vec<ExtractionField<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedIndicator {
    pub indicator_type: IndicatorType,
    pub value: String,
    pub normalized_value: String,
    pub source_field: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub surrounding_text: String,
    pub confidence_hint: Option<u8>,
}
