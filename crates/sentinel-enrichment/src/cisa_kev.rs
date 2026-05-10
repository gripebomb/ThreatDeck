use crate::{
    EnrichmentError, EnrichmentProvider, EnrichmentRequest, EnrichmentResult, ProviderConfig,
    Reputation,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use sentinel_ioc::IndicatorType;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

const PROVIDER_NAME: &str = "cisa-kev";
const SUPPORTED_TYPES: &[IndicatorType] = &[IndicatorType::Cve];

#[derive(Debug, Clone)]
pub struct CisaKevProvider {
    entries: HashMap<String, KevEntry>,
}

impl CisaKevProvider {
    pub fn from_json_file(path: &Path) -> Result<Self, EnrichmentError> {
        let json = std::fs::read_to_string(path)
            .map_err(|err| EnrichmentError::Provider(err.to_string()))?;
        Self::from_json_str(&json)
    }

    pub fn from_json_str(json: &str) -> Result<Self, EnrichmentError> {
        let catalog: KevCatalog =
            serde_json::from_str(json).map_err(|err| EnrichmentError::Provider(err.to_string()))?;
        let entries = catalog
            .vulnerabilities
            .into_iter()
            .map(|entry| (entry.cve_id.to_ascii_uppercase(), entry))
            .collect();
        Ok(Self { entries })
    }

    fn unknown_result(&self, request: &EnrichmentRequest) -> EnrichmentResult {
        EnrichmentResult {
            provider_name: PROVIDER_NAME.to_string(),
            indicator_type: request.indicator_type,
            normalized_value: request.normalized_value.clone(),
            reputation: Reputation::Unknown,
            score: Some(0),
            verdict: Some("Not Listed".into()),
            summary: Some("CVE is not listed in the cached CISA KEV catalog.".into()),
            raw_json: serde_json::json!({
                "cveID": request.normalized_value,
                "listed": false
            }),
            expires_at: Some(Utc::now() + Duration::hours(24)),
        }
    }

    fn known_result(&self, request: &EnrichmentRequest, entry: &KevEntry) -> EnrichmentResult {
        let summary = format!(
            "{} {}: {}. Due date: {}.",
            entry.vendor_project.as_deref().unwrap_or("Unknown vendor"),
            entry.product.as_deref().unwrap_or("Unknown product"),
            entry
                .vulnerability_name
                .as_deref()
                .unwrap_or("Known exploited vulnerability"),
            entry.due_date.as_deref().unwrap_or("unspecified")
        );
        EnrichmentResult {
            provider_name: PROVIDER_NAME.to_string(),
            indicator_type: request.indicator_type,
            normalized_value: request.normalized_value.clone(),
            reputation: Reputation::Malicious,
            score: Some(90),
            verdict: Some("Known Exploited".into()),
            summary: Some(summary),
            raw_json: serde_json::to_value(entry).unwrap_or_else(|_| {
                serde_json::json!({
                    "cveID": request.normalized_value,
                    "listed": true
                })
            }),
            expires_at: Some(Utc::now() + Duration::hours(24)),
        }
    }
}

#[async_trait]
impl EnrichmentProvider for CisaKevProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn supported_types(&self) -> &[IndicatorType] {
        SUPPORTED_TYPES
    }

    async fn enrich(
        &self,
        request: &EnrichmentRequest,
        _config: &ProviderConfig,
    ) -> Result<EnrichmentResult, EnrichmentError> {
        if !self.supports_type(request.indicator_type) {
            return Err(EnrichmentError::UnsupportedIndicatorType {
                provider: PROVIDER_NAME.to_string(),
                indicator_type: request.indicator_type,
            });
        }

        let normalized = request.normalized_value.to_ascii_uppercase();
        Ok(self
            .entries
            .get(&normalized)
            .map(|entry| self.known_result(request, entry))
            .unwrap_or_else(|| self.unknown_result(request)))
    }
}

#[derive(Debug, Deserialize)]
struct KevCatalog {
    #[serde(default)]
    vulnerabilities: Vec<KevEntry>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct KevEntry {
    #[serde(rename = "cveID")]
    cve_id: String,
    vendor_project: Option<String>,
    product: Option<String>,
    vulnerability_name: Option<String>,
    date_added: Option<String>,
    due_date: Option<String>,
    required_action: Option<String>,
    notes: Option<String>,
}
