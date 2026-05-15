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
}
