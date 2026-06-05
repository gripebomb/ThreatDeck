# sentinel-enrichment

Provider-neutral IOC enrichment types and local test providers used by ThreatDeck.

`sentinel-enrichment` defines a small async provider interface for turning normalized indicators into reputation, score, verdict, summary, raw provider data, and cache-expiration metadata. It is designed to sit behind a queue or outbox so ingestion can stay fast while enrichment happens separately.

## Install

```toml
[dependencies]
sentinel-enrichment = "0.1.1"
sentinel-ioc = "0.1.0"
```

## What It Provides

- `EnrichmentProvider`, an async trait for provider implementations.
- `EnrichmentRequest`, the normalized input passed to providers.
- `EnrichmentResult`, the normalized output stored or displayed by callers.
- `Reputation`, a provider-independent reputation enum.
- `ProviderConfig`, a serializable config structure for provider settings, rate limits, cache TTLs, and secret references.
- `MockProvider`, a deterministic provider for tests and local queue validation.
- `CisaKevProvider`, a local CISA Known Exploited Vulnerabilities catalog provider for CVE indicators.
- `UrlHausProvider`, an external URLHaus API provider for URL, domain, IPv4, MD5, and SHA256 indicators.

## Example: Mock Provider

```rust
use sentinel_enrichment::{
    EnrichmentProvider, EnrichmentRequest, MockProvider, ProviderConfig, Reputation,
};
use sentinel_ioc::IndicatorType;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let provider = MockProvider::new("mock-local", vec![IndicatorType::Cve])
    .with_reputation(Reputation::Malicious)
    .with_score(90)
    .with_verdict("Known bad");

let request = EnrichmentRequest {
    indicator_id: 1,
    indicator_type: IndicatorType::Cve,
    normalized_value: "CVE-2025-12345".to_string(),
    provider_name: "mock-local".to_string(),
};

let result = provider.enrich(&request, &ProviderConfig::default()).await?;

assert_eq!(result.reputation, Reputation::Malicious);
assert_eq!(result.score, Some(90));
# Ok(())
# }
```

## Example: CISA KEV Provider

```rust
use sentinel_enrichment::{
    CisaKevProvider, EnrichmentProvider, EnrichmentRequest, ProviderConfig, Reputation,
};
use sentinel_ioc::IndicatorType;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let catalog = r#"{
  "vulnerabilities": [
    {
      "cveID": "CVE-2025-12345",
      "vendorProject": "ExampleVendor",
      "product": "ExampleProduct",
      "vulnerabilityName": "Example exploited vulnerability",
      "dueDate": "2025-06-01"
    }
  ]
}"#;

let provider = CisaKevProvider::from_json_str(catalog)?;
let request = EnrichmentRequest {
    indicator_id: 1,
    indicator_type: IndicatorType::Cve,
    normalized_value: "CVE-2025-12345".to_string(),
    provider_name: "cisa-kev".to_string(),
};

let result = provider.enrich(&request, &ProviderConfig::default()).await?;

assert_eq!(result.reputation, Reputation::Malicious);
assert_eq!(result.verdict.as_deref(), Some("Known Exploited"));
# Ok(())
# }
```

## Provider Behavior

Providers declare their supported `IndicatorType` values and should return `EnrichmentError::UnsupportedIndicatorType` for unsupported requests. Callers are expected to manage persistence, queueing, retries, rate limits, and cache freshness around provider execution.

The included `CisaKevProvider` is local-file/local-string based. It does not fetch the CISA KEV catalog over the network.

`UrlHausProvider` calls the URLHaus API and requires an Auth-Key. Set `ProviderConfig.secret_ref` to `env:URLHAUS_AUTH_KEY` to read the key from the environment, or pass `auth_key` in `ProviderConfig.values` for controlled test/local use. The provider queries URL, host, and payload endpoints depending on the indicator type.

## License

MIT
