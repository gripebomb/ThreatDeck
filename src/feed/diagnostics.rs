use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchFailurePhase {
    UrlValidation,
    Dns,
    TcpConnect,
    TlsHandshake,
    HttpRequest,
    HttpStatus,
    Redirect,
    BodyDownload,
    ContentDecode,
    FeedParse,
    DatabaseWrite,
    Unknown,
}

impl FetchFailurePhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::UrlValidation => "URL validation",
            Self::Dns => "DNS lookup",
            Self::TcpConnect => "TCP connect",
            Self::TlsHandshake => "TLS handshake",
            Self::HttpRequest => "HTTP request",
            Self::HttpStatus => "HTTP status",
            Self::Redirect => "redirect",
            Self::BodyDownload => "body download",
            Self::ContentDecode => "content decode",
            Self::FeedParse => "feed parse",
            Self::DatabaseWrite => "database write",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value {
            "URL validation" => Self::UrlValidation,
            "DNS lookup" => Self::Dns,
            "TCP connect" => Self::TcpConnect,
            "TLS handshake" => Self::TlsHandshake,
            "HTTP request" => Self::HttpRequest,
            "HTTP status" => Self::HttpStatus,
            "redirect" => Self::Redirect,
            "body download" => Self::BodyDownload,
            "content decode" => Self::ContentDecode,
            "feed parse" => Self::FeedParse,
            "database write" => Self::DatabaseWrite,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchFailureKind {
    InvalidUrl,
    DnsResolutionFailed,
    ConnectionRefused,
    ConnectionTimeout,
    TlsCertificateInvalid,
    TlsHandshakeFailed,
    HttpStatusClientError,
    HttpStatusServerError,
    TooManyRedirects,
    BodyReadFailed,
    InvalidXml,
    InvalidJson,
    InvalidRss,
    DatabaseError,
    Unknown,
}

impl FetchFailureKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::InvalidUrl => "invalid URL",
            Self::DnsResolutionFailed => "DNS resolution failed",
            Self::ConnectionRefused => "connection refused",
            Self::ConnectionTimeout => "connection timeout",
            Self::TlsCertificateInvalid => "TLS certificate invalid",
            Self::TlsHandshakeFailed => "TLS handshake failed",
            Self::HttpStatusClientError => "HTTP client error",
            Self::HttpStatusServerError => "HTTP server error",
            Self::TooManyRedirects => "too many redirects",
            Self::BodyReadFailed => "body read failed",
            Self::InvalidXml => "invalid XML",
            Self::InvalidJson => "invalid JSON",
            Self::InvalidRss => "invalid RSS",
            Self::DatabaseError => "database error",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value {
            "invalid URL" => Self::InvalidUrl,
            "DNS resolution failed" => Self::DnsResolutionFailed,
            "connection refused" => Self::ConnectionRefused,
            "connection timeout" => Self::ConnectionTimeout,
            "TLS certificate invalid" => Self::TlsCertificateInvalid,
            "TLS handshake failed" => Self::TlsHandshakeFailed,
            "HTTP client error" => Self::HttpStatusClientError,
            "HTTP server error" => Self::HttpStatusServerError,
            "too many redirects" => Self::TooManyRedirects,
            "body read failed" => Self::BodyReadFailed,
            "invalid XML" => Self::InvalidXml,
            "invalid JSON" => Self::InvalidJson,
            "invalid RSS" => Self::InvalidRss,
            "database error" => Self::DatabaseError,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchDiagnostic {
    pub phase: FetchFailurePhase,
    pub kind: FetchFailureKind,
    pub summary: String,
    pub detail: Option<String>,
    pub http_status: Option<u16>,
    pub url: String,
    pub final_url: Option<String>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone)]
pub struct FetchAttempt {
    pub id: Option<i64>,
    pub feed_id: Option<i64>,
    pub attempted_at: Option<DateTime<Utc>>,
    pub success: bool,
    pub url: String,
    pub final_url: Option<String>,
    pub http_status: Option<u16>,
    pub elapsed_ms: u128,
    pub diagnostic: Option<FetchDiagnostic>,
    pub items_seen: Option<usize>,
    pub items_new: Option<usize>,
}

pub fn classify_http_status(url: &str, status: u16, elapsed_ms: u128) -> FetchDiagnostic {
    let kind = if (400..500).contains(&status) {
        FetchFailureKind::HttpStatusClientError
    } else if (500..600).contains(&status) {
        FetchFailureKind::HttpStatusServerError
    } else {
        FetchFailureKind::Unknown
    };

    FetchDiagnostic {
        phase: FetchFailurePhase::HttpStatus,
        kind,
        summary: format!("Feed returned HTTP {status}"),
        detail: None,
        http_status: Some(status),
        url: url.to_string(),
        final_url: None,
        elapsed_ms,
    }
}

pub fn classify_transport_error(url: &str, error: &str, elapsed_ms: u128) -> FetchDiagnostic {
    let lower = error.to_lowercase();
    let (phase, kind, summary) = if lower.contains("certificate")
        || lower.contains("unknownissuer")
        || lower.contains("invalid peer certificate")
    {
        (
            FetchFailurePhase::TlsHandshake,
            FetchFailureKind::TlsCertificateInvalid,
            "TLS certificate validation failed",
        )
    } else if lower.contains("tls") || lower.contains("ssl") {
        (
            FetchFailurePhase::TlsHandshake,
            FetchFailureKind::TlsHandshakeFailed,
            "TLS handshake failed",
        )
    } else if lower.contains("dns")
        || lower.contains("lookup")
        || lower.contains("name or service not known")
        || lower.contains("temporary failure in name resolution")
    {
        (
            FetchFailurePhase::Dns,
            FetchFailureKind::DnsResolutionFailed,
            "DNS resolution failed",
        )
    } else if lower.contains("timed out") || lower.contains("timeout") {
        (
            FetchFailurePhase::TcpConnect,
            FetchFailureKind::ConnectionTimeout,
            "Connection timed out while fetching feed",
        )
    } else if lower.contains("connection refused") {
        (
            FetchFailurePhase::TcpConnect,
            FetchFailureKind::ConnectionRefused,
            "Connection refused by remote host",
        )
    } else if lower.contains("redirect") {
        (
            FetchFailurePhase::Redirect,
            FetchFailureKind::TooManyRedirects,
            "Too many redirects while fetching feed",
        )
    } else {
        (
            FetchFailurePhase::HttpRequest,
            FetchFailureKind::Unknown,
            "Feed request failed",
        )
    };

    FetchDiagnostic {
        phase,
        kind,
        summary: summary.to_string(),
        detail: Some(error.to_string()),
        http_status: None,
        url: url.to_string(),
        final_url: None,
        elapsed_ms,
    }
}

pub fn classify_body_error(url: &str, error: &str, elapsed_ms: u128) -> FetchDiagnostic {
    FetchDiagnostic {
        phase: FetchFailurePhase::BodyDownload,
        kind: FetchFailureKind::BodyReadFailed,
        summary: "Feed response body could not be read".to_string(),
        detail: Some(error.to_string()),
        http_status: None,
        url: url.to_string(),
        final_url: None,
        elapsed_ms,
    }
}

pub fn classify_parse_error(
    url: &str,
    context: &str,
    error: &str,
    elapsed_ms: u128,
) -> FetchDiagnostic {
    let lower = context.to_lowercase();
    let (kind, summary) = if lower.contains("json") {
        (
            FetchFailureKind::InvalidJson,
            "API response could not be parsed as JSON",
        )
    } else if lower.contains("rss") {
        (
            FetchFailureKind::InvalidRss,
            "Feed response could not be parsed as RSS",
        )
    } else {
        (
            FetchFailureKind::InvalidXml,
            "Feed response could not be parsed as XML",
        )
    };

    FetchDiagnostic {
        phase: FetchFailurePhase::FeedParse,
        kind,
        summary: summary.to_string(),
        detail: Some(error.to_string()),
        http_status: None,
        url: url.to_string(),
        final_url: None,
        elapsed_ms,
    }
}

pub fn classify_anyhow_error(
    url: &str,
    error: &anyhow::Error,
    elapsed_ms: u128,
) -> FetchDiagnostic {
    let chain = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(": ");
    let lower = chain.to_lowercase();

    if let Some(status) = extract_http_status(&lower) {
        let mut diagnostic = classify_http_status(url, status, elapsed_ms);
        diagnostic.detail = Some(chain);
        return diagnostic;
    }

    if lower.contains("reading rss body")
        || lower.contains("reading api response body")
        || lower.contains("reading website body")
        || lower.contains("reading onion site body")
    {
        return classify_body_error(url, &chain, elapsed_ms);
    }

    if lower.contains("parsing rss feed") {
        return classify_parse_error(url, "parsing RSS feed", &chain, elapsed_ms);
    }

    if lower.contains("parsing api json") {
        return classify_parse_error(url, "parsing API JSON", &chain, elapsed_ms);
    }

    classify_transport_error(url, &chain, elapsed_ms)
}

fn extract_http_status(error: &str) -> Option<u16> {
    for token in error.split(|c: char| !c.is_ascii_digit()) {
        if token.len() == 3 {
            if let Ok(status) = token.parse::<u16>() {
                if (100..600).contains(&status) {
                    return Some(status);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_phase_display_is_operator_readable() {
        assert_eq!(FetchFailurePhase::TlsHandshake.label(), "TLS handshake");
        assert_eq!(FetchFailurePhase::FeedParse.label(), "feed parse");
    }

    #[test]
    fn failure_kind_display_is_operator_readable() {
        assert_eq!(
            FetchFailureKind::TlsCertificateInvalid.label(),
            "TLS certificate invalid"
        );
        assert_eq!(FetchFailureKind::InvalidRss.label(), "invalid RSS");
    }

    #[test]
    fn classify_http_status_client_error() {
        let diagnostic = classify_http_status("https://example.test/feed.xml", 404, 12);
        assert_eq!(diagnostic.phase, FetchFailurePhase::HttpStatus);
        assert_eq!(diagnostic.kind, FetchFailureKind::HttpStatusClientError);
        assert_eq!(diagnostic.summary, "Feed returned HTTP 404");
        assert_eq!(diagnostic.http_status, Some(404));
    }

    #[test]
    fn classify_transport_tls_certificate_text() {
        let diagnostic = classify_transport_error(
            "https://example.test/feed.xml",
            "invalid peer certificate: UnknownIssuer",
            20,
        );
        assert_eq!(diagnostic.phase, FetchFailurePhase::TlsHandshake);
        assert_eq!(diagnostic.kind, FetchFailureKind::TlsCertificateInvalid);
    }

    #[test]
    fn classify_transport_dns_text() {
        let diagnostic = classify_transport_error(
            "https://missing.invalid/feed.xml",
            "failed to lookup address information: Name or service not known",
            20,
        );
        assert_eq!(diagnostic.phase, FetchFailurePhase::Dns);
        assert_eq!(diagnostic.kind, FetchFailureKind::DnsResolutionFailed);
    }

    #[test]
    fn classify_parse_errors() {
        let rss = classify_parse_error(
            "https://example.test/feed.xml",
            "parsing RSS feed",
            "invalid XML",
            5,
        );
        assert_eq!(rss.phase, FetchFailurePhase::FeedParse);
        assert_eq!(rss.kind, FetchFailureKind::InvalidRss);

        let json = classify_parse_error(
            "https://example.test/api",
            "parsing API JSON",
            "expected value",
            5,
        );
        assert_eq!(json.kind, FetchFailureKind::InvalidJson);
    }
}
