# sentinel-ioc

Pure Rust IOC extraction and normalization logic used by ThreatDeck.

`sentinel-ioc` scans named text fields and returns structured indicators with their original value, normalized value, source field, byte offsets, surrounding context, and optional confidence hints. It is intentionally provider-free and network-free so it can be used safely during ingestion, alerting, tests, and offline analysis.

## Supported Indicator Types

- IPv4 and IPv6 addresses
- Domains and URLs
- Email addresses
- MD5, SHA1, and SHA256 hashes
- CVE identifiers
- MITRE ATT&CK technique IDs
- Onion domains and onion URLs

The `IndicatorType` enum also includes reserved variants for future extractor support, including crypto wallets, cloud access keys, and unknown indicators.

## Install

```toml
[dependencies]
sentinel-ioc = "0.1.0"
```

## Example

```rust
use sentinel_ioc::{extract_indicators, ExtractionField, ExtractionInput, IndicatorType};

let input = ExtractionInput {
    content_item_id: Some(42),
    alert_id: None,
    feed_id: Some(7),
    fields: vec![ExtractionField {
        name: "body",
        text: "Exploit observed for CVE-2025-12345 from https://evil.example/login",
    }],
};

let indicators = extract_indicators(&input);

assert!(indicators
    .iter()
    .any(|ioc| ioc.indicator_type == IndicatorType::Cve
        && ioc.normalized_value == "CVE-2025-12345"));
assert!(indicators
    .iter()
    .any(|ioc| ioc.indicator_type == IndicatorType::Url
        && ioc.normalized_value == "https://evil.example/login"));
```

## Core Types

- `ExtractionInput` groups the feed/content/alert context and a list of text fields to scan.
- `ExtractionField` names an individual text field.
- `ExtractedIndicator` contains the extracted value, normalized value, source offsets, context, and type.
- `IndicatorType` identifies the IOC category.
- `IgnoreList` supports filtering noisy or unwanted matches.
- `IocExtractor` provides a reusable extractor instance when you need custom ignore behavior.

## Design Notes

This crate does not perform enrichment, reputation checks, DNS lookups, HTTP requests, or persistence. Pair it with `sentinel-enrichment` or your own storage layer when you need provider lookups or database-backed indicator history.

## License

MIT
