use crate::types::IndicatorType;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

const TRIM_CHARS: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '`',
];

pub fn trim_indicator(value: &str) -> &str {
    value
        .trim_matches(char::is_whitespace)
        .trim_matches(TRIM_CHARS)
}

pub fn normalize_ipv4(value: &str) -> Option<String> {
    let value = trim_indicator(value);
    let mut octets = [0_u8; 4];
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 4 {
        return None;
    }

    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() || part.len() > 3 || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        octets[idx] = part.parse::<u8>().ok()?;
    }

    Some(Ipv4Addr::from(octets).to_string())
}

pub fn normalize_ipv6(value: &str) -> Option<String> {
    let value = trim_indicator(value);
    value.parse::<Ipv6Addr>().ok().map(|ip| ip.to_string())
}

pub fn normalize_domain(value: &str) -> Option<String> {
    let value = trim_indicator(value)
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if value.is_empty()
        || value == "localhost"
        || value == "example.com"
        || value.ends_with(".example.com")
        || value.contains("..")
        || !value.contains('.')
    {
        return None;
    }

    let labels: Vec<&str> = value.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|label| !valid_domain_label(label)) {
        return None;
    }

    let tld = labels.last()?;
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    Some(value)
}

pub fn normalize_onion_domain(value: &str) -> Option<String> {
    let value = trim_indicator(value)
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let host = value.strip_suffix(".onion")?;
    if host.len() == 56 && host.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')) {
        Some(format!("{host}.onion"))
    } else {
        None
    }
}

pub fn normalize_url(value: &str) -> Option<(String, IndicatorType)> {
    let value = trim_indicator(value);
    let parsed = Url::parse(value).ok()?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }

    let host = parsed.host_str()?.to_ascii_lowercase();
    let mut normalized = parsed;
    normalized.set_scheme(&scheme).ok()?;
    normalized.set_host(Some(&host)).ok()?;

    let indicator_type = if host.ends_with(".onion") {
        IndicatorType::OnionUrl
    } else {
        IndicatorType::Url
    };

    Some((normalized.to_string(), indicator_type))
}

pub fn normalize_email(value: &str) -> Option<String> {
    let value = trim_indicator(value).to_ascii_lowercase();
    let (_, domain) = value.rsplit_once('@')?;
    normalize_domain(domain)?;
    Some(value)
}

pub fn normalize_hash(value: &str, expected_len: usize) -> Option<String> {
    let value = trim_indicator(value);
    if value.len() == expected_len && value.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(value.to_ascii_lowercase())
    } else {
        None
    }
}

pub fn normalize_cve(value: &str) -> Option<String> {
    let value = trim_indicator(value).to_ascii_uppercase();
    let mut parts = value.split('-');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("CVE"), Some(year), Some(id), None)
            if year.len() == 4
                && year.chars().all(|c| c.is_ascii_digit())
                && (4..=10).contains(&id.len())
                && id.chars().all(|c| c.is_ascii_digit()) =>
        {
            Some(value)
        }
        _ => None,
    }
}

pub fn normalize_mitre_technique(value: &str) -> Option<String> {
    let value = trim_indicator(value).to_ascii_uppercase();
    let technique = value.strip_prefix('T')?;
    let (base, sub) = technique
        .split_once('.')
        .map_or((technique, None), |(base, sub)| (base, Some(sub)));

    if base.len() != 4 || !base.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(sub) = sub {
        if sub.len() != 3 || !sub.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }

    Some(value)
}

pub fn ip_flags(normalized_ip: &str) -> Option<IpFlags> {
    match normalized_ip.parse::<IpAddr>().ok()? {
        IpAddr::V4(ip) => Some(IpFlags {
            is_private: ip.is_private(),
            is_loopback: ip.is_loopback(),
            is_multicast: ip.is_multicast(),
            is_reserved: ip.is_documentation() || ip.is_unspecified() || ip.is_broadcast(),
        }),
        IpAddr::V6(ip) => Some(IpFlags {
            is_private: is_unique_local_ipv6(&ip),
            is_loopback: ip.is_loopback(),
            is_multicast: ip.is_multicast(),
            is_reserved: ip.is_unspecified(),
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpFlags {
    pub is_private: bool,
    pub is_reserved: bool,
    pub is_loopback: bool,
    pub is_multicast: bool,
}

fn valid_domain_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn is_unique_local_ipv6(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}
