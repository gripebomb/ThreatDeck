use crate::{
    EnrichmentError, EnrichmentProvider, EnrichmentRequest, EnrichmentResult, ProviderConfig,
    Reputation,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use sentinel_ioc::IndicatorType;
use serde_json::Value;
use std::sync::Arc;

const PROVIDER_NAME: &str = "urlhaus";
const DEFAULT_BASE_URL: &str = "https://urlhaus-api.abuse.ch/v1";
const SUPPORTED_TYPES: &[IndicatorType] = &[
    IndicatorType::Url,
    IndicatorType::Domain,
    IndicatorType::Ipv4,
    IndicatorType::Md5,
    IndicatorType::Sha256,
];

pub trait UrlHausHttpClient: Send + Sync {
    fn post_form(
        &self,
        url: &str,
        auth_key: &str,
        form: &[(&str, &str)],
    ) -> Result<Value, EnrichmentError>;
}

#[derive(Debug, Clone)]
pub struct UreqUrlHausHttpClient {
    agent: ureq::Agent,
}

impl Default for UreqUrlHausHttpClient {
    fn default() -> Self {
        Self {
            agent: ureq::AgentBuilder::new().build(),
        }
    }
}

impl UreqUrlHausHttpClient {
    pub fn with_agent(agent: ureq::Agent) -> Self {
        Self { agent }
    }
}

impl UrlHausHttpClient for UreqUrlHausHttpClient {
    fn post_form(
        &self,
        url: &str,
        auth_key: &str,
        form: &[(&str, &str)],
    ) -> Result<Value, EnrichmentError> {
        self.agent
            .post(url)
            .set("Auth-Key", auth_key)
            .send_form(form)
            .map_err(|err| EnrichmentError::Provider(format!("URLHaus request failed: {err}")))?
            .into_json()
            .map_err(|err| {
                EnrichmentError::Provider(format!("URLHaus response was not JSON: {err}"))
            })
    }
}

#[derive(Clone)]
pub struct UrlHausProvider {
    client: Arc<dyn UrlHausHttpClient>,
    base_url: String,
}

impl Default for UrlHausProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl UrlHausProvider {
    pub fn new() -> Self {
        Self {
            client: Arc::new(UreqUrlHausHttpClient::default()),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_client(client: Arc<dyn UrlHausHttpClient>) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    fn auth_key(config: &ProviderConfig) -> Result<String, EnrichmentError> {
        if let Some(value) = config.values.get("auth_key").and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return Ok(value.trim().to_string());
            }
        }

        if let Some(secret_ref) = config.secret_ref.as_deref() {
            if let Some(env_name) = secret_ref.strip_prefix("env:") {
                let value = std::env::var(env_name).map_err(|_| {
                    EnrichmentError::Provider(format!(
                        "URLHaus Auth-Key environment variable {env_name} is not set"
                    ))
                })?;
                if !value.trim().is_empty() {
                    return Ok(value);
                }
                return Err(EnrichmentError::Provider(format!(
                    "URLHaus Auth-Key environment variable {env_name} is empty"
                )));
            }

            if !secret_ref.trim().is_empty() {
                return Ok(secret_ref.trim().to_string());
            }
        }

        Err(EnrichmentError::Provider(
            "URLHaus Auth-Key is required; set provider secret_ref to env:URLHAUS_AUTH_KEY or provide config value auth_key".into(),
        ))
    }

    fn request_parts(
        indicator_type: IndicatorType,
        value: &str,
    ) -> Option<(&'static str, Vec<(&'static str, String)>)> {
        match indicator_type {
            IndicatorType::Url => Some(("url", vec![("url", value.to_string())])),
            IndicatorType::Domain | IndicatorType::Ipv4 => {
                Some(("host", vec![("host", value.to_string())]))
            }
            IndicatorType::Md5 => Some(("payload", vec![("md5_hash", value.to_string())])),
            IndicatorType::Sha256 => Some(("payload", vec![("sha256_hash", value.to_string())])),
            _ => None,
        }
    }

    fn result_from_response(
        &self,
        request: &EnrichmentRequest,
        response: Value,
    ) -> EnrichmentResult {
        let status = response
            .get("query_status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        if status == "no_results" {
            return EnrichmentResult {
                provider_name: PROVIDER_NAME.to_string(),
                indicator_type: request.indicator_type,
                normalized_value: request.normalized_value.clone(),
                reputation: Reputation::Unknown,
                score: Some(0),
                verdict: Some("Not Listed".into()),
                summary: Some("Indicator is not listed in URLHaus.".into()),
                raw_json: response,
                expires_at: Some(Utc::now() + Duration::hours(24)),
            };
        }

        if status != "ok" {
            return EnrichmentResult {
                provider_name: PROVIDER_NAME.to_string(),
                indicator_type: request.indicator_type,
                normalized_value: request.normalized_value.clone(),
                reputation: Reputation::Unknown,
                score: None,
                verdict: Some(format!("URLHaus query status: {status}")),
                summary: Some("URLHaus did not return a definitive result.".into()),
                raw_json: response,
                expires_at: Some(Utc::now() + Duration::hours(6)),
            };
        }

        let online = response
            .get("url_status")
            .and_then(Value::as_str)
            .map(|status| status == "online")
            .unwrap_or_else(|| response_has_online_url(&response));
        let signature = response.get("signature").and_then(Value::as_str);
        let tags = response
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(Value::as_str)
                    .take(5)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|tags| !tags.is_empty());
        let url_count = response
            .get("url_count")
            .and_then(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| value.as_i64().map(|value| value.to_string()))
            })
            .or_else(|| {
                response
                    .get("urls")
                    .and_then(Value::as_array)
                    .map(|urls| urls.len().to_string())
            });

        let mut parts = Vec::new();
        if online {
            parts.push("active malware URL observed".to_string());
        } else {
            parts.push("malware distribution indicator observed".to_string());
        }
        if let Some(signature) = signature {
            parts.push(format!("signature: {signature}"));
        }
        if let Some(tags) = tags {
            parts.push(format!("tags: {tags}"));
        }
        if let Some(url_count) = url_count {
            parts.push(format!("associated URLs: {url_count}"));
        }

        EnrichmentResult {
            provider_name: PROVIDER_NAME.to_string(),
            indicator_type: request.indicator_type,
            normalized_value: request.normalized_value.clone(),
            reputation: Reputation::KnownMalware,
            score: Some(if online { 90 } else { 75 }),
            verdict: Some("URLHaus Match".into()),
            summary: Some(parts.join("; ")),
            raw_json: response,
            expires_at: Some(Utc::now() + Duration::hours(12)),
        }
    }
}

#[async_trait]
impl EnrichmentProvider for UrlHausProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn supported_types(&self) -> &[IndicatorType] {
        SUPPORTED_TYPES
    }

    async fn enrich(
        &self,
        request: &EnrichmentRequest,
        config: &ProviderConfig,
    ) -> Result<EnrichmentResult, EnrichmentError> {
        let Some((endpoint, form)) =
            Self::request_parts(request.indicator_type, &request.normalized_value)
        else {
            return Err(EnrichmentError::UnsupportedIndicatorType {
                provider: PROVIDER_NAME.to_string(),
                indicator_type: request.indicator_type,
            });
        };

        let auth_key = Self::auth_key(config)?;
        let url = format!("{}/{}/", self.base_url, endpoint);
        let form_refs = form
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        let response = self.client.post_form(&url, &auth_key, &form_refs)?;

        Ok(self.result_from_response(request, response))
    }
}

fn response_has_online_url(response: &Value) -> bool {
    response
        .get("urls")
        .and_then(Value::as_array)
        .map(|urls| {
            urls.iter().any(|url| {
                url.get("url_status")
                    .and_then(Value::as_str)
                    .map(|status| status == "online")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    type RecordedCall = (String, String, Vec<(String, String)>);

    #[derive(Default)]
    struct RecordingClient {
        calls: Mutex<Vec<RecordedCall>>,
        response: Value,
    }

    impl RecordingClient {
        fn new(response: Value) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                response,
            }
        }
    }

    impl UrlHausHttpClient for RecordingClient {
        fn post_form(
            &self,
            url: &str,
            auth_key: &str,
            form: &[(&str, &str)],
        ) -> Result<Value, EnrichmentError> {
            self.calls.lock().unwrap().push((
                url.to_string(),
                auth_key.to_string(),
                form.iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            ));
            Ok(self.response.clone())
        }
    }

    fn request(indicator_type: IndicatorType, value: &str) -> EnrichmentRequest {
        EnrichmentRequest {
            indicator_id: 1,
            indicator_type,
            normalized_value: value.into(),
            provider_name: "urlhaus".into(),
        }
    }

    #[tokio::test]
    async fn urlhaus_queries_url_with_auth_key() {
        let client = Arc::new(RecordingClient::new(json!({
            "query_status": "ok",
            "url_status": "online",
            "signature": "Heodo",
            "tags": ["exe", "loader"]
        })));
        let provider =
            UrlHausProvider::with_client(client.clone()).with_base_url("https://unit.test/v1");
        let mut config = ProviderConfig::default();
        config.values.insert("auth_key".into(), json!("test-key"));

        let result = provider
            .enrich(
                &request(IndicatorType::Url, "http://bad.example/a"),
                &config,
            )
            .await
            .expect("URLHaus response maps");

        assert_eq!(result.reputation, Reputation::KnownMalware);
        assert_eq!(result.score, Some(90));
        assert_eq!(result.verdict.as_deref(), Some("URLHaus Match"));
        assert!(result.summary.unwrap().contains("Heodo"));

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls[0].0, "https://unit.test/v1/url/");
        assert_eq!(calls[0].1, "test-key");
        assert_eq!(
            calls[0].2,
            vec![("url".into(), "http://bad.example/a".into())]
        );
    }

    #[tokio::test]
    async fn urlhaus_maps_no_results_to_unknown() {
        let client = Arc::new(RecordingClient::new(json!({
            "query_status": "no_results"
        })));
        let provider = UrlHausProvider::with_client(client);
        let mut config = ProviderConfig::default();
        config.values.insert("auth_key".into(), json!("test-key"));

        let result = provider
            .enrich(&request(IndicatorType::Domain, "example.com"), &config)
            .await
            .expect("no results maps");

        assert_eq!(result.reputation, Reputation::Unknown);
        assert_eq!(result.score, Some(0));
        assert_eq!(result.verdict.as_deref(), Some("Not Listed"));
    }

    #[tokio::test]
    async fn urlhaus_uses_payload_hash_form() {
        let client = Arc::new(RecordingClient::new(json!({
            "query_status": "ok",
            "signature": "Lumma",
            "url_count": "3"
        })));
        let provider = UrlHausProvider::with_client(client.clone());
        let mut config = ProviderConfig::default();
        config.values.insert("auth_key".into(), json!("test-key"));

        provider
            .enrich(
                &request(
                    IndicatorType::Sha256,
                    "35e304d10d53834e3e41035d12122773c9a4d183a24e03f980ad3e6b2ecde7fa",
                ),
                &config,
            )
            .await
            .expect("hash maps");

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls[0].0, "https://urlhaus-api.abuse.ch/v1/payload/");
        assert_eq!(
            calls[0].2[0],
            (
                "sha256_hash".into(),
                "35e304d10d53834e3e41035d12122773c9a4d183a24e03f980ad3e6b2ecde7fa".into()
            )
        );
    }

    #[tokio::test]
    async fn urlhaus_requires_auth_key() {
        let provider = UrlHausProvider::with_client(Arc::new(RecordingClient::new(json!({}))));

        let err = provider
            .enrich(
                &request(IndicatorType::Url, "http://bad.example/a"),
                &ProviderConfig::default(),
            )
            .await
            .expect_err("missing auth key fails");

        assert!(err.to_string().contains("Auth-Key"));
    }
}
