use crate::normalize;
use crate::types::{ExtractedIndicator, IndicatorType};
use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct IgnoreList {
    domains: HashSet<String>,
    emails: HashSet<String>,
    hashes: HashSet<String>,
    patterns: Vec<Regex>,
    pub ignore_reserved_ips: bool,
    pub ignore_private_ips: bool,
}

impl IgnoreList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_domain(mut self, domain: impl AsRef<str>) -> Self {
        if let Some(domain) = normalize::normalize_domain(domain.as_ref()) {
            self.domains.insert(domain);
        }
        self
    }

    pub fn with_email(mut self, email: impl AsRef<str>) -> Self {
        if let Some(email) = normalize::normalize_email(email.as_ref()) {
            self.emails.insert(email);
        }
        self
    }

    pub fn with_hash(mut self, hash: impl AsRef<str>) -> Self {
        self.hashes.insert(hash.as_ref().to_ascii_lowercase());
        self
    }

    pub fn with_pattern(mut self, pattern: &str) -> Result<Self, regex::Error> {
        self.patterns.push(Regex::new(pattern)?);
        Ok(self)
    }

    pub fn ignore_reserved_ips(mut self, ignore: bool) -> Self {
        self.ignore_reserved_ips = ignore;
        self
    }

    pub fn ignore_private_ips(mut self, ignore: bool) -> Self {
        self.ignore_private_ips = ignore;
        self
    }

    pub fn is_ignored(&self, indicator: &ExtractedIndicator) -> bool {
        if self.patterns.iter().any(|re| re.is_match(&indicator.value)) {
            return true;
        }

        match indicator.indicator_type {
            IndicatorType::Domain | IndicatorType::OnionDomain => {
                self.domains.contains(&indicator.normalized_value)
            }
            IndicatorType::Email => self.emails.contains(&indicator.normalized_value),
            IndicatorType::Md5 | IndicatorType::Sha1 | IndicatorType::Sha256 => {
                self.hashes.contains(&indicator.normalized_value)
            }
            IndicatorType::Ipv4 | IndicatorType::Ipv6 => {
                normalize::ip_flags(&indicator.normalized_value).is_some_and(|flags| {
                    (self.ignore_private_ips && flags.is_private)
                        || (self.ignore_reserved_ips && flags.is_reserved)
                })
            }
            _ => false,
        }
    }
}
