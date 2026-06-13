use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sentinel_ioc::IndicatorType;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use thiserror::Error;

mod cisa_kev;
mod urlhaus;

pub use cisa_kev::CisaKevProvider;
pub use urlhaus::{UreqUrlHausHttpClient, UrlHausProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reputation {
    Unknown,
    Benign,
    Suspicious,
    Malicious,
    KnownScanner,
    KnownPhishing,
    KnownMalware,
    KnownC2,
    KnownRansomware,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentRequest {
    pub indicator_id: i64,
    pub indicator_type: IndicatorType,
    pub normalized_value: String,
    pub provider_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnrichmentResult {
    pub provider_name: String,
    pub indicator_type: IndicatorType,
    pub normalized_value: String,
    pub reputation: Reputation,
    pub score: Option<i32>,
    pub verdict: Option<String>,
    pub summary: Option<String>,
    pub raw_json: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub secret_ref: Option<String>,
    pub rate_limit_per_minute: Option<u32>,
    pub cache_ttl_hours: Option<u32>,
    pub values: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Error)]
pub enum EnrichmentError {
    #[error("unsupported indicator type {indicator_type:?} for provider {provider}")]
    UnsupportedIndicatorType {
        provider: String,
        indicator_type: IndicatorType,
    },
    #[error("provider {provider} is disabled")]
    Disabled { provider: String },
    #[error("{0}")]
    Provider(String),
}

#[async_trait]
pub trait EnrichmentProvider: Send + Sync {
    fn name(&self) -> &str;

    fn supported_types(&self) -> &[IndicatorType];

    fn supports_type(&self, indicator_type: IndicatorType) -> bool {
        self.supported_types().contains(&indicator_type)
    }

    async fn enrich(
        &self,
        request: &EnrichmentRequest,
        config: &ProviderConfig,
    ) -> Result<EnrichmentResult, EnrichmentError>;
}

#[derive(Debug, Clone)]
pub struct MockProvider {
    name: String,
    supported_types: Vec<IndicatorType>,
    reputation: Reputation,
    score: Option<i32>,
    verdict: Option<String>,
    summary: Option<String>,
}

impl MockProvider {
    pub fn new(name: impl Into<String>, supported_types: Vec<IndicatorType>) -> Self {
        Self {
            name: name.into(),
            supported_types,
            reputation: Reputation::Unknown,
            score: None,
            verdict: None,
            summary: None,
        }
    }

    pub fn with_reputation(mut self, reputation: Reputation) -> Self {
        self.reputation = reputation;
        self
    }

    pub fn with_score(mut self, score: i32) -> Self {
        self.score = Some(score);
        self
    }

    pub fn with_verdict(mut self, verdict: impl Into<String>) -> Self {
        self.verdict = Some(verdict.into());
        self
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }
}

#[async_trait]
impl EnrichmentProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_types(&self) -> &[IndicatorType] {
        &self.supported_types
    }

    async fn enrich(
        &self,
        request: &EnrichmentRequest,
        _config: &ProviderConfig,
    ) -> Result<EnrichmentResult, EnrichmentError> {
        if !self.supports_type(request.indicator_type) {
            return Err(EnrichmentError::UnsupportedIndicatorType {
                provider: self.name.clone(),
                indicator_type: request.indicator_type,
            });
        }

        Ok(EnrichmentResult {
            provider_name: self.name.clone(),
            indicator_type: request.indicator_type,
            normalized_value: request.normalized_value.clone(),
            reputation: self.reputation,
            score: self.score,
            verdict: self.verdict.clone(),
            summary: self.summary.clone(),
            raw_json: json!({
                "provider": self.name,
                "indicator_id": request.indicator_id,
                "indicator": request.normalized_value,
                "mock": true
            }),
            expires_at: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CisaKevProvider, EnrichmentProvider, EnrichmentRequest, MockProvider, ProviderConfig,
        Reputation,
    };
    use sentinel_ioc::IndicatorType;

    #[tokio::test]
    async fn mock_provider_enriches_supported_indicator() {
        let provider = MockProvider::new("mock-local", vec![IndicatorType::Cve])
            .with_reputation(Reputation::Malicious)
            .with_score(90);
        let request = EnrichmentRequest {
            indicator_id: 42,
            indicator_type: IndicatorType::Cve,
            normalized_value: "CVE-2025-12345".into(),
            provider_name: "mock-local".into(),
        };

        let result = provider
            .enrich(&request, &ProviderConfig::default())
            .await
            .expect("mock enrichment succeeds");

        assert_eq!(provider.name(), "mock-local");
        assert!(provider.supports_type(IndicatorType::Cve));
        assert_eq!(result.provider_name, "mock-local");
        assert_eq!(result.indicator_type, IndicatorType::Cve);
        assert_eq!(result.normalized_value, "CVE-2025-12345");
        assert_eq!(result.reputation, Reputation::Malicious);
        assert_eq!(result.score, Some(90));
    }

    #[tokio::test]
    async fn mock_provider_rejects_unsupported_indicator_type() {
        let provider = MockProvider::new("mock-local", vec![IndicatorType::Cve]);
        let request = EnrichmentRequest {
            indicator_id: 7,
            indicator_type: IndicatorType::Domain,
            normalized_value: "bad.example.net".into(),
            provider_name: "mock-local".into(),
        };

        let err = provider
            .enrich(&request, &ProviderConfig::default())
            .await
            .expect_err("unsupported type is rejected");

        assert!(err.to_string().contains("unsupported"));
    }

    #[tokio::test]
    async fn cisa_kev_provider_matches_known_exploited_cve() {
        let provider = CisaKevProvider::from_json_str(
            r#"{
                "title": "CISA Known Exploited Vulnerabilities Catalog",
                "vulnerabilities": [
                    {
                        "cveID": "CVE-2025-12345",
                        "vendorProject": "ExampleVendor",
                        "product": "ExampleProduct",
                        "vulnerabilityName": "Example vulnerability",
                        "dateAdded": "2025-05-01",
                        "dueDate": "2025-06-01",
                        "requiredAction": "Apply mitigations",
                        "notes": "Known exploited in the wild"
                    }
                ]
            }"#,
        )
        .expect("fixture parses");
        let request = EnrichmentRequest {
            indicator_id: 1,
            indicator_type: IndicatorType::Cve,
            normalized_value: "CVE-2025-12345".into(),
            provider_name: "cisa-kev".into(),
        };

        let result = provider
            .enrich(&request, &ProviderConfig::default())
            .await
            .expect("known CVE enriches");

        assert_eq!(provider.name(), "cisa-kev");
        assert_eq!(provider.supported_types(), &[IndicatorType::Cve]);
        assert_eq!(result.reputation, Reputation::Malicious);
        assert_eq!(result.score, Some(90));
        assert_eq!(result.verdict.as_deref(), Some("Known Exploited"));
        assert!(result
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("ExampleVendor ExampleProduct"));
        assert_eq!(result.raw_json["vendorProject"], "ExampleVendor");
    }

    #[tokio::test]
    async fn cisa_kev_provider_returns_unknown_for_cve_not_in_catalog() {
        let provider =
            CisaKevProvider::from_json_str(r#"{"vulnerabilities":[]}"#).expect("catalog parses");
        let request = EnrichmentRequest {
            indicator_id: 1,
            indicator_type: IndicatorType::Cve,
            normalized_value: "CVE-2025-99999".into(),
            provider_name: "cisa-kev".into(),
        };

        let result = provider
            .enrich(&request, &ProviderConfig::default())
            .await
            .expect("unknown CVE enriches");

        assert_eq!(result.reputation, Reputation::Unknown);
        assert_eq!(result.score, Some(0));
        assert_eq!(result.verdict.as_deref(), Some("Not Listed"));
    }

    #[tokio::test]
    async fn cisa_kev_provider_loads_catalog_from_file() {
        let path =
            std::env::temp_dir().join(format!("threatdeck-cisa-kev-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{
                "vulnerabilities": [
                    {
                        "cveID": "CVE-2026-11111",
                        "vendorProject": "FileVendor",
                        "product": "FileProduct",
                        "vulnerabilityName": "File loaded vulnerability",
                        "dueDate": "2026-06-01"
                    }
                ]
            }"#,
        )
        .expect("write fixture");
        let provider = CisaKevProvider::from_json_file(&path).expect("file catalog loads");
        let request = EnrichmentRequest {
            indicator_id: 1,
            indicator_type: IndicatorType::Cve,
            normalized_value: "CVE-2026-11111".into(),
            provider_name: "cisa-kev".into(),
        };

        let result = provider
            .enrich(&request, &ProviderConfig::default())
            .await
            .expect("known CVE enriches");

        assert_eq!(result.verdict.as_deref(), Some("Known Exploited"));
        assert!(result
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("FileVendor FileProduct"));
        let _ = std::fs::remove_file(path);
    }
}
