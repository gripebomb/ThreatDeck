use crate::config::TlsTrustStore;
use crate::db::Db;
use anyhow::{Context, Result};
use sentinel_enrichment::{
    CisaKevProvider, EnrichmentProvider, EnrichmentRequest, ProviderConfig, UrlHausProvider,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct EnrichmentWorker {
    providers: HashMap<String, Arc<dyn EnrichmentProvider>>,
}

pub async fn run_enrichment_once(
    db: &Db,
    cache_dir: &Path,
    limit: i64,
    tls_trust_store: TlsTrustStore,
) -> Result<usize> {
    let worker = EnrichmentWorker::from_cache_dir(cache_dir, tls_trust_store)?;
    worker.process_pending(db, limit).await
}

impl EnrichmentWorker {
    pub fn new(providers: Vec<Arc<dyn EnrichmentProvider>>) -> Self {
        let providers = providers
            .into_iter()
            .map(|provider| (provider.name().to_string(), provider))
            .collect();
        Self { providers }
    }

    pub fn from_cache_dir(cache_dir: &Path, tls_trust_store: TlsTrustStore) -> Result<Self> {
        let mut providers: Vec<Arc<dyn EnrichmentProvider>> = Vec::new();
        let cisa_kev_path = cache_dir.join("cisa-kev.json");
        if cisa_kev_path.exists() {
            providers.push(Arc::new(
                CisaKevProvider::from_json_file(&cisa_kev_path)
                    .with_context(|| format!("loading {}", cisa_kev_path.display()))?,
            ));
        }
        providers.push(Arc::new(UrlHausProvider::with_client(Arc::new(
            sentinel_enrichment::UreqUrlHausHttpClient::with_agent(crate::http::agent(
                tls_trust_store,
            )?),
        ))));
        Ok(Self::new(providers))
    }

    pub async fn process_pending(&self, db: &Db, limit: i64) -> Result<usize> {
        let jobs = db.claim_next_enrichment_jobs(limit)?;
        let mut succeeded = 0;
        let mut provider_request_counts: HashMap<i64, u32> = HashMap::new();

        for job in jobs {
            let Some(provider_record) = db.get_enrichment_provider(job.provider_id)? else {
                db.mark_enrichment_job_failed(job.id, "provider record not found", true)?;
                continue;
            };
            let Some(provider) = self.providers.get(&provider_record.name) else {
                db.mark_enrichment_job_failed(job.id, "provider not registered", true)?;
                continue;
            };
            let Some(indicator) = db.get_indicator(job.indicator_id)? else {
                db.mark_enrichment_job_failed(job.id, "indicator record not found", false)?;
                continue;
            };

            let config = provider_config_from_record(&provider_record);
            if let Some(rate_limit) = config.rate_limit_per_minute {
                let used = provider_request_counts
                    .entry(provider_record.id)
                    .or_insert(0);
                if *used >= rate_limit {
                    db.mark_enrichment_job_rate_limited(
                        job.id,
                        &format!(
                            "provider {} rate limit reached: {} request(s)/minute",
                            provider_record.name, rate_limit
                        ),
                    )?;
                    continue;
                }
                *used += 1;
            }

            let request = EnrichmentRequest {
                indicator_id: indicator.id,
                indicator_type: indicator.indicator_type,
                normalized_value: indicator.normalized_value,
                provider_name: provider_record.name.clone(),
            };

            match provider.enrich(&request, &config).await {
                Ok(result) => {
                    db.store_enrichment_result(indicator.id, provider_record.id, &result)
                        .with_context(|| format!("storing enrichment result for job {}", job.id))?;
                    db.mark_enrichment_job_succeeded(job.id)?;
                    succeeded += 1;
                }
                Err(err) => {
                    db.mark_enrichment_job_failed(job.id, &err.to_string(), true)?;
                }
            }
        }

        Ok(succeeded)
    }
}

fn provider_config_from_record(provider: &crate::db::EnrichmentProviderRecord) -> ProviderConfig {
    let mut config = provider
        .config_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<ProviderConfig>(json).ok())
        .unwrap_or_default();
    config.enabled = provider.enabled;
    config.secret_ref = provider.secret_ref.clone();
    config.rate_limit_per_minute = provider.rate_limit_per_minute.map(|value| value as u32);
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Db, EnrichmentProviderCreate};
    use sentinel_enrichment::{MockProvider, Reputation};
    use sentinel_ioc::{ExtractedIndicator, IndicatorType};
    use std::sync::Arc;

    fn memory_db() -> Db {
        Db::new_in_memory_for_tests()
    }

    fn cve_indicator() -> ExtractedIndicator {
        ExtractedIndicator {
            indicator_type: IndicatorType::Cve,
            value: "CVE-2025-12345".into(),
            normalized_value: "CVE-2025-12345".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 14,
            surrounding_text: "CVE-2025-12345".into(),
            confidence_hint: Some(90),
        }
    }

    fn cve_indicator_with_value(value: &str) -> ExtractedIndicator {
        ExtractedIndicator {
            value: value.into(),
            normalized_value: value.into(),
            ..cve_indicator()
        }
    }

    #[test]
    fn worker_processes_pending_job_with_mock_provider() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator_id = db.upsert_indicator(&cve_indicator()).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "mock-kev".into(),
                provider_type: "mock".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Cve],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        db.queue_enrichment_job(indicator_id, provider_id, 100)
            .unwrap();
        let worker = EnrichmentWorker::new(vec![Arc::new(
            MockProvider::new("mock-kev", vec![IndicatorType::Cve])
                .with_reputation(Reputation::Malicious)
                .with_score(88),
        )]);

        let processed = runtime.block_on(worker.process_pending(&db, 5)).unwrap();

        assert_eq!(processed, 1);
        let job = db.claim_next_enrichment_jobs(5).unwrap();
        assert!(job.is_empty());
        let results = db.get_latest_enrichment_results(indicator_id).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, Some(88));
        assert_eq!(results[0].reputation.as_deref(), Some("Malicious"));
    }

    #[test]
    fn worker_marks_job_retrying_when_provider_is_missing() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator_id = db.upsert_indicator(&cve_indicator()).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "missing-provider".into(),
                provider_type: "mock".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Cve],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        let job_id = db
            .queue_enrichment_job(indicator_id, provider_id, 100)
            .unwrap();
        let worker = EnrichmentWorker::new(Vec::new());

        let processed = runtime.block_on(worker.process_pending(&db, 5)).unwrap();

        assert_eq!(processed, 0);
        let job = db.get_enrichment_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, "retrying");
        assert_eq!(job.attempt_count, 1);
        assert!(job
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("provider not registered"));
    }

    #[test]
    fn worker_reschedules_jobs_over_provider_rate_limit() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let db = memory_db();
        db.init_schema().unwrap();
        let first_indicator_id = db
            .upsert_indicator(&cve_indicator_with_value("CVE-2025-11111"))
            .unwrap();
        let second_indicator_id = db
            .upsert_indicator(&cve_indicator_with_value("CVE-2025-22222"))
            .unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "mock-limited".into(),
                provider_type: "mock".into(),
                enabled: true,
                rate_limit_per_minute: Some(1),
                supports_types: vec![IndicatorType::Cve],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        db.queue_enrichment_job(first_indicator_id, provider_id, 100)
            .unwrap();
        let second_job_id = db
            .queue_enrichment_job(second_indicator_id, provider_id, 100)
            .unwrap();
        let worker = EnrichmentWorker::new(vec![Arc::new(
            MockProvider::new("mock-limited", vec![IndicatorType::Cve])
                .with_reputation(Reputation::Malicious),
        )]);

        let processed = runtime.block_on(worker.process_pending(&db, 5)).unwrap();

        assert_eq!(processed, 1);
        let second_job = db.get_enrichment_job(second_job_id).unwrap().unwrap();
        assert_eq!(second_job.status, "rate_limited");
        assert_eq!(second_job.attempt_count, 0);
        assert!(second_job
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("rate limit reached"));
    }

    #[test]
    fn worker_loads_cisa_kev_provider_from_cache_dir() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let cache_dir = std::env::temp_dir().join(format!(
            "threatdeck-enrichment-cache-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            cache_dir.join("cisa-kev.json"),
            r#"{
                "vulnerabilities": [
                    {
                        "cveID": "CVE-2025-12345",
                        "vendorProject": "CacheVendor",
                        "product": "CacheProduct",
                        "vulnerabilityName": "Cached KEV",
                        "dueDate": "2025-06-01"
                    }
                ]
            }"#,
        )
        .unwrap();
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator_id = db.upsert_indicator(&cve_indicator()).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "cisa-kev".into(),
                provider_type: "cisa_kev".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Cve],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        db.queue_enrichment_job(indicator_id, provider_id, 100)
            .unwrap();
        let worker = EnrichmentWorker::from_cache_dir(&cache_dir, TlsTrustStore::Bundled).unwrap();

        let processed = runtime.block_on(worker.process_pending(&db, 5)).unwrap();

        assert_eq!(processed, 1);
        let results = db.get_latest_enrichment_results(indicator_id).unwrap();
        assert_eq!(results[0].verdict.as_deref(), Some("Known Exploited"));
        assert!(results[0]
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("CacheVendor CacheProduct"));

        let _ = std::fs::remove_file(cache_dir.join("cisa-kev.json"));
        let _ = std::fs::remove_dir(cache_dir);
    }

    #[test]
    fn run_enrichment_once_processes_cached_cisa_kev_jobs() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let cache_dir =
            std::env::temp_dir().join(format!("threatdeck-enrichment-once-{}", std::process::id()));
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            cache_dir.join("cisa-kev.json"),
            r#"{
                "vulnerabilities": [
                    {
                        "cveID": "CVE-2025-12345",
                        "vendorProject": "OnceVendor",
                        "product": "OnceProduct",
                        "vulnerabilityName": "Once KEV",
                        "dueDate": "2025-06-01"
                    }
                ]
            }"#,
        )
        .unwrap();
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator_id = db.upsert_indicator(&cve_indicator()).unwrap();
        let providers = db.list_enabled_enrichment_providers().unwrap();
        let provider_id = providers
            .iter()
            .find(|provider| provider.name == "cisa-kev")
            .unwrap()
            .id;
        db.queue_enrichment_job(indicator_id, provider_id, 100)
            .unwrap();

        let processed = runtime
            .block_on(run_enrichment_once(
                &db,
                &cache_dir,
                5,
                TlsTrustStore::Bundled,
            ))
            .unwrap();

        assert_eq!(processed, 1);
        let results = db.get_latest_enrichment_results(indicator_id).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0]
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("OnceVendor OnceProduct"));

        let _ = std::fs::remove_file(cache_dir.join("cisa-kev.json"));
        let _ = std::fs::remove_dir(cache_dir);
    }
}
