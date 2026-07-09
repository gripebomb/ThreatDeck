//! TUI view models for the split-pane alert workbench.
//!
//! These types decouple rendering from raw database row shapes. This module
//! depends only on domain types ([`crate::types`]); it never imports storage
//! row types. Storage rows are converted to these view models in the app layer
//! (`crate::app`), which is the only place that touches storage. Rendering code
//! consumes these view models exclusively.
//!
//! See `docs/ARCHITECTURE.md` (Responsibility Boundaries) and
//! `tickets/02-workbench-view-models-and-data-bundle.md`.

use chrono::{DateTime, Utc};
use sentinel_ioc::IndicatorType;

use crate::types::{Alert, AlertDisposition, AlertStatus, AlertWithMeta, Criticality};

// ── Alert list ───────────────────────────────────────────────────────────────

/// A single row in the left alert-list pane. Rendered, never the raw DB shape.
#[derive(Debug, Clone)]
pub struct AlertListItem {
    pub id: i64,
    pub title: Option<String>,
    pub severity: Criticality,
    pub status: AlertStatus,
    pub disposition: AlertDisposition,
    pub feed_name: String,
    pub keyword_pattern: String,
    pub read: bool,
    pub detected_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

impl From<&AlertWithMeta> for AlertListItem {
    fn from(value: &AlertWithMeta) -> Self {
        Self {
            id: value.alert.id,
            title: value.alert.title.clone(),
            // Show the *effective* severity so an override is reflected in the list.
            severity: value.alert.effective_severity(),
            status: value.alert.status,
            disposition: value.alert.disposition,
            feed_name: value.feed_name.clone(),
            keyword_pattern: value.keyword_pattern.clone(),
            read: value.alert.read,
            detected_at: value.alert.detected_at,
            tags: value.tags.iter().map(|t| t.name.clone()).collect(),
        }
    }
}

// ── Alert details ────────────────────────────────────────────────────────────

/// Top-right selected-alert detail payload. Built by the app service from an
/// [`Alert`] plus its feed/keyword/tags context.
#[derive(Debug, Clone)]
pub struct AlertDetailViewModel {
    pub id: i64,
    pub title: Option<String>,
    pub feed_name: String,
    pub feed_url: Option<String>,
    pub keyword_pattern: String,
    /// Effective severity (honours a `severity_override`).
    pub severity: Criticality,
    /// Original keyword criticality, before any override.
    pub base_criticality: Criticality,
    pub status: AlertStatus,
    pub disposition: AlertDisposition,
    pub confidence: Option<i64>,
    pub owner: Option<String>,
    pub tags: Vec<String>,
    pub snippet: String,
    pub triage_notes: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub investigating_at: Option<DateTime<Utc>>,
    pub escalated_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_reason: Option<String>,
    /// Original source URL when resolvable (currently None: alerts have no
    /// direct link back to their originating feed item).
    pub source_url: Option<String>,
}

impl AlertDetailViewModel {
    /// Compose the detail view model from its constituent storage records.
    ///
    /// Kept as a constructor (rather than a `From`) because it joins several
    /// unrelated row types; only the app service should call this.
    pub fn from_parts(
        alert: &Alert,
        feed_name: String,
        feed_url: Option<String>,
        keyword_pattern: String,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: alert.id,
            title: alert.title.clone(),
            feed_name,
            feed_url,
            keyword_pattern,
            severity: alert.effective_severity(),
            base_criticality: alert.criticality,
            status: alert.status,
            disposition: alert.disposition,
            confidence: alert.confidence_score,
            owner: alert.owner.clone(),
            tags,
            snippet: alert.content_snippet.clone(),
            triage_notes: alert.triage_notes.clone(),
            detected_at: alert.detected_at,
            acknowledged_at: alert.acknowledged_at,
            investigating_at: alert.investigating_at,
            escalated_at: alert.escalated_at,
            closed_at: alert.closed_at,
            closed_reason: alert.closed_reason.clone(),
            source_url: None,
        }
    }
}

// ── Indicators / enrichment ──────────────────────────────────────────────────

/// One enrichment result attached to an indicator.
#[derive(Debug, Clone)]
pub struct EnrichmentViewModel {
    pub provider_id: i64,
    pub status: String,
    pub reputation: Option<String>,
    pub score: Option<i64>,
    pub verdict: Option<String>,
    pub summary: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

/// An extracted indicator (the "IOCs" tab) with its enrichment results nested.
#[derive(Debug, Clone)]
pub struct IndicatorViewModel {
    pub id: i64,
    pub indicator_type: IndicatorType,
    pub value: String,
    pub normalized_value: String,
    pub sighting_count: i64,
    pub confidence: Option<i64>,
    pub risk: Option<i64>,
    pub enrichment: Vec<EnrichmentViewModel>,
}

impl IndicatorViewModel {
    /// Human-readable type label (mirrors the Indicators screen labels).
    pub fn type_label(&self) -> &'static str {
        indicator_type_label(self.indicator_type)
    }
}

/// Shared indicator-type label. `IndicatorType` has no `Display` impl, so this
/// centralises the mapping used by both the workbench and the existing list UI.
pub fn indicator_type_label(indicator_type: IndicatorType) -> &'static str {
    match indicator_type {
        IndicatorType::Ipv4 => "IPv4",
        IndicatorType::Ipv6 => "IPv6",
        IndicatorType::Domain => "Domain",
        IndicatorType::Url => "URL",
        IndicatorType::Email => "Email",
        IndicatorType::Md5 => "MD5",
        IndicatorType::Sha1 => "SHA1",
        IndicatorType::Sha256 => "SHA256",
        IndicatorType::Cve => "CVE",
        IndicatorType::MitreAttackTechnique => "MITRE",
        IndicatorType::OnionDomain => "Onion",
        IndicatorType::OnionUrl => "Onion URL",
        IndicatorType::CryptoWallet => "Wallet",
        IndicatorType::CloudAccessKey => "Cloud Key",
        IndicatorType::Unknown => "Unknown",
    }
}

// ── Triage history ───────────────────────────────────────────────────────────

/// One triage audit event (the "Triage History" tab).
#[derive(Debug, Clone)]
pub struct TriageEventViewModel {
    pub id: i64,
    pub event_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub note: Option<String>,
    pub actor: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Bundle ───────────────────────────────────────────────────────────────────

/// Everything the split-pane view needs for one selected alert: details plus
/// the bottom-right context-tab data. Loaded once per selection change by the
/// app service so rendering never issues SQL.
#[derive(Debug, Clone, Default)]
pub struct AlertWorkbenchBundle {
    pub detail: Option<AlertDetailViewModel>,
    pub indicators: Vec<IndicatorViewModel>,
    /// Raw metadata JSON blob; the Metadata tab pretty-prints it.
    pub metadata_json: Option<String>,
    pub triage_history: Vec<TriageEventViewModel>,
    /// Original feed-item raw content if resolvable (currently always None).
    pub raw_content: Option<String>,
}

impl AlertWorkbenchBundle {
    pub fn is_empty(&self) -> bool {
        self.detail.is_none()
            && self.indicators.is_empty()
            && self.metadata_json.is_none()
            && self.triage_history.is_empty()
            && self.raw_content.is_none()
    }
}
