use crate::ignore::IgnoreList;
use crate::normalize;
use crate::patterns;
use crate::types::{ExtractedIndicator, ExtractionField, ExtractionInput, IndicatorType};
use regex::Match;
use std::collections::HashSet;

const SURROUNDING_TEXT_CHARS: usize = 120;

#[derive(Debug, Default)]
pub struct IocExtractor {
    ignore_list: IgnoreList,
}

impl IocExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ignore_list(ignore_list: IgnoreList) -> Self {
        Self { ignore_list }
    }

    pub fn extract(&self, input: &ExtractionInput<'_>) -> Vec<ExtractedIndicator> {
        let mut indicators = Vec::new();
        for field in &input.fields {
            self.extract_field(field, &mut indicators);
        }
        indicators
            .into_iter()
            .filter(|indicator| !self.ignore_list.is_ignored(indicator))
            .collect()
    }

    fn extract_field(&self, field: &ExtractionField<'_>, indicators: &mut Vec<ExtractedIndicator>) {
        let mut seen_spans = HashSet::new();

        self.scan_matches(
            field,
            patterns::url().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_url(value).map(|(normalized, indicator_type)| {
                    (
                        indicator_type,
                        normalized,
                        Some(if indicator_type == IndicatorType::OnionUrl {
                            90
                        } else {
                            80
                        }),
                    )
                })
            },
        );

        self.scan_matches(
            field,
            patterns::email().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_email(value)
                    .map(|normalized| (IndicatorType::Email, normalized, Some(75)))
            },
        );

        self.scan_matches(
            field,
            patterns::onion_domain().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_onion_domain(value)
                    .map(|normalized| (IndicatorType::OnionDomain, normalized, Some(90)))
            },
        );

        self.scan_matches(
            field,
            patterns::sha256().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_hash(value, 64)
                    .map(|normalized| (IndicatorType::Sha256, normalized, Some(85)))
            },
        );

        self.scan_matches(
            field,
            patterns::sha1().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_hash(value, 40)
                    .map(|normalized| (IndicatorType::Sha1, normalized, Some(80)))
            },
        );

        self.scan_matches(
            field,
            patterns::md5().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_hash(value, 32)
                    .map(|normalized| (IndicatorType::Md5, normalized, Some(75)))
            },
        );

        self.scan_matches(
            field,
            patterns::cve().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_cve(value)
                    .map(|normalized| (IndicatorType::Cve, normalized, Some(90)))
            },
        );

        self.scan_matches(
            field,
            patterns::mitre_technique().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_mitre_technique(value)
                    .map(|normalized| (IndicatorType::MitreAttackTechnique, normalized, Some(80)))
            },
        );

        self.scan_matches(
            field,
            patterns::ipv4().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_ipv4(value)
                    .map(|normalized| (IndicatorType::Ipv4, normalized, Some(80)))
            },
        );

        self.scan_matches(
            field,
            patterns::ipv6().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_ipv6(value)
                    .map(|normalized| (IndicatorType::Ipv6, normalized, Some(80)))
            },
        );

        self.scan_matches(
            field,
            patterns::domain().find_iter(field.text),
            &mut seen_spans,
            indicators,
            |value| {
                normalize::normalize_domain(value)
                    .map(|normalized| (IndicatorType::Domain, normalized, Some(65)))
            },
        );
    }

    fn scan_matches<'a, I, F>(
        &self,
        field: &ExtractionField<'_>,
        matches: I,
        seen_spans: &mut HashSet<(usize, usize)>,
        indicators: &mut Vec<ExtractedIndicator>,
        normalize_match: F,
    ) where
        I: Iterator<Item = Match<'a>>,
        F: Fn(&str) -> Option<(IndicatorType, String, Option<u8>)>,
    {
        for matched in matches {
            if overlaps_seen(matched.start(), matched.end(), seen_spans) {
                continue;
            }

            let raw = normalize::trim_indicator(matched.as_str());
            let Some((indicator_type, normalized_value, confidence_hint)) = normalize_match(raw)
            else {
                continue;
            };

            let start_offset = matched.start();
            let end_offset = matched.start() + raw.len();
            seen_spans.insert((start_offset, end_offset));
            indicators.push(ExtractedIndicator {
                indicator_type,
                value: raw.to_string(),
                normalized_value,
                source_field: field.name.to_string(),
                start_offset,
                end_offset,
                surrounding_text: surrounding_text(field.text, start_offset, end_offset),
                confidence_hint,
            });
        }
    }
}

pub fn extract_indicators(input: &ExtractionInput<'_>) -> Vec<ExtractedIndicator> {
    IocExtractor::new().extract(input)
}

fn overlaps_seen(start: usize, end: usize, seen_spans: &HashSet<(usize, usize)>) -> bool {
    seen_spans
        .iter()
        .any(|(seen_start, seen_end)| start < *seen_end && end > *seen_start)
}

fn surrounding_text(text: &str, start: usize, end: usize) -> String {
    let context_start = floor_char_boundary(text, start.saturating_sub(SURROUNDING_TEXT_CHARS));
    let context_end = ceil_char_boundary(text, (end + SURROUNDING_TEXT_CHARS).min(text.len()));
    text[context_start..context_end].to_string()
}

fn floor_char_boundary(text: &str, mut idx: usize) -> usize {
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(text: &str, mut idx: usize) -> usize {
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> ExtractionInput<'_> {
        ExtractionInput {
            content_item_id: Some(1),
            alert_id: Some(2),
            feed_id: Some(3),
            fields: vec![ExtractionField { name: "body", text }],
        }
    }

    fn assert_contains(text: &str, indicator_type: IndicatorType, normalized: &str) {
        let indicators = extract_indicators(&input(text));
        assert!(
            indicators
                .iter()
                .any(|indicator| indicator.indicator_type == indicator_type
                    && indicator.normalized_value == normalized),
            "missing {indicator_type:?} {normalized}; got {indicators:#?}"
        );
    }

    #[test]
    fn extracts_ipv4_and_normalizes_octets() {
        assert_contains(
            "Callback 192.168.001.001 observed",
            IndicatorType::Ipv4,
            "192.168.1.1",
        );
    }

    #[test]
    fn rejects_invalid_ipv4() {
        let indicators = extract_indicators(&input("Invalid 999.1.1.1 version 1.2.3"));
        assert!(indicators
            .iter()
            .all(|i| i.indicator_type != IndicatorType::Ipv4));
    }

    #[test]
    fn extracts_ipv6() {
        assert_contains(
            "Host 2001:0db8:0000:0000:0000:ff00:0042:8329",
            IndicatorType::Ipv6,
            "2001:db8::ff00:42:8329",
        );
    }

    #[test]
    fn extracts_domain() {
        assert_contains(
            "Visit Evil.Example.NET.",
            IndicatorType::Domain,
            "evil.example.net",
        );
    }

    #[test]
    fn rejects_placeholder_domain() {
        let indicators = extract_indicators(&input("Ignore example.com and localhost"));
        assert!(indicators
            .iter()
            .all(|i| i.indicator_type != IndicatorType::Domain));
    }

    #[test]
    fn extracts_url_and_preserves_path_case() {
        assert_contains(
            "Open HTTP://Example.COM/Some/Path?q=One.",
            IndicatorType::Url,
            "http://example.com/Some/Path?q=One",
        );
    }

    #[test]
    fn extracts_email() {
        assert_contains(
            "Contact User@Example.ORG.",
            IndicatorType::Email,
            "user@example.org",
        );
    }

    #[test]
    fn extracts_hashes() {
        assert_contains(
            "md5 D41D8CD98F00B204E9800998ECF8427E",
            IndicatorType::Md5,
            "d41d8cd98f00b204e9800998ecf8427e",
        );
        assert_contains(
            "sha1 DA39A3EE5E6B4B0D3255BFEF95601890AFD80709",
            IndicatorType::Sha1,
            "da39a3ee5e6b4b0d3255bfef95601890afd80709",
        );
        assert_contains(
            "sha256 E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
            IndicatorType::Sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }

    #[test]
    fn extracts_cve() {
        assert_contains(
            "Patch cve-2025-12345 now",
            IndicatorType::Cve,
            "CVE-2025-12345",
        );
    }

    #[test]
    fn extracts_mitre_attack_technique() {
        assert_contains(
            "Observed t1059.001",
            IndicatorType::MitreAttackTechnique,
            "T1059.001",
        );
    }

    #[test]
    fn extracts_onion_domain_and_url() {
        let onion = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcd.onion";
        assert_contains(
            &format!("Leak at {onion}"),
            IndicatorType::OnionDomain,
            onion,
        );
        assert_contains(
            &format!("Leak at http://{onion}/post"),
            IndicatorType::OnionUrl,
            &format!("http://{onion}/post"),
        );
    }

    #[test]
    fn tracks_source_offsets_and_context() {
        let text = "prefix CVE-2025-12345 suffix";
        let indicators = extract_indicators(&input(text));
        let cve = indicators
            .iter()
            .find(|indicator| indicator.indicator_type == IndicatorType::Cve)
            .expect("CVE extracted");

        assert_eq!(cve.source_field, "body");
        assert_eq!(cve.start_offset, 7);
        assert_eq!(cve.end_offset, 21);
        assert_eq!(cve.surrounding_text, text);
    }

    #[test]
    fn ignore_list_filters_domains_and_private_ips() {
        let ignore_list = IgnoreList::new()
            .with_domain("evil.example.net")
            .ignore_private_ips(true);
        let extractor = IocExtractor::with_ignore_list(ignore_list);
        let indicators = extractor.extract(&input("evil.example.net 10.1.2.3 8.8.8.8"));

        assert!(!indicators
            .iter()
            .any(|indicator| indicator.normalized_value == "evil.example.net"));
        assert!(!indicators
            .iter()
            .any(|indicator| indicator.normalized_value == "10.1.2.3"));
        assert!(indicators
            .iter()
            .any(|indicator| indicator.normalized_value == "8.8.8.8"));
    }
}
