use regex::Regex;
use std::sync::OnceLock;

pub fn url() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)\bhttps?://[^\s<>"'`{}|\^]+"#).expect("valid URL regex"))
}

pub fn email() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,63}\b").expect("valid email regex")
    })
}

pub fn ipv4() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("valid IPv4 regex"))
}

pub fn ipv6() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[0-9A-Fa-f]{0,4}:[0-9A-Fa-f:]{2,}(?:%[0-9A-Za-z]+)?\b")
            .expect("valid IPv6 regex")
    })
}

pub fn sha256() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b[a-f0-9]{64}\b").expect("valid SHA256 regex"))
}

pub fn sha1() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b[a-f0-9]{40}\b").expect("valid SHA1 regex"))
}

pub fn md5() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b[a-f0-9]{32}\b").expect("valid MD5 regex"))
}

pub fn cve() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bCVE-\d{4}-\d{4,10}\b").expect("valid CVE regex"))
}

pub fn mitre_technique() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bT\d{4}(?:\.\d{3})?\b").expect("valid ATT&CK regex"))
}

pub fn onion_domain() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b[a-z2-7]{56}\.onion\b").expect("valid onion domain regex"))
}

pub fn domain() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}\b")
            .expect("valid domain regex")
    })
}
