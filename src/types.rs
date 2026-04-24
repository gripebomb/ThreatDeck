use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Enums ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FeedType {
    #[default]
    Api,
    Rss,
    Website,
    Onion,
}

impl std::fmt::Display for FeedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedType::Api => write!(f, "API"),
            FeedType::Rss => write!(f, "RSS"),
            FeedType::Website => write!(f, "Website"),
            FeedType::Onion => write!(f, "Onion"),
        }
    }
}

impl From<&str> for FeedType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "api" => FeedType::Api,
            "rss" => FeedType::Rss,
            "website" => FeedType::Website,
            "onion" => FeedType::Onion,
            _ => FeedType::Api,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedStatus {
    Healthy,
    Warning,
    Error,
    Disabled,
}

impl FeedStatus {
    pub fn label(self) -> &'static str {
        match self {
            FeedStatus::Healthy => "Healthy",
            FeedStatus::Warning => "Warning",
            FeedStatus::Error => "Error",
            FeedStatus::Disabled => "Disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Hash, Default)]
pub enum Criticality {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Criticality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Criticality::Low => write!(f, "Low"),
            Criticality::Medium => write!(f, "Medium"),
            Criticality::High => write!(f, "High"),
            Criticality::Critical => write!(f, "Critical"),
        }
    }
}

impl From<&str> for Criticality {
    fn from(s: &str) -> Self {
        match s {
            "Low" => Criticality::Low,
            "Medium" => Criticality::Medium,
            "High" => Criticality::High,
            "Critical" => Criticality::Critical,
            _ => Criticality::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeywordMatchType {
    Simple,
    Regex,
}

impl std::fmt::Display for KeywordMatchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeywordMatchType::Simple => write!(f, "Simple"),
            KeywordMatchType::Regex => write!(f, "Regex"),
        }
    }
}

impl From<&str> for KeywordMatchType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "simple" => KeywordMatchType::Simple,
            "regex" => KeywordMatchType::Regex,
            _ => KeywordMatchType::Simple,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NotificationChannel {
    #[default]
    Email,
    Webhook,
    Discord,
}

impl std::fmt::Display for NotificationChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationChannel::Email => write!(f, "Email"),
            NotificationChannel::Webhook => write!(f, "Webhook"),
            NotificationChannel::Discord => write!(f, "Discord"),
        }
    }
}

impl From<&str> for NotificationChannel {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "email" => NotificationChannel::Email,
            "webhook" => NotificationChannel::Webhook,
            "discord" => NotificationChannel::Discord,
            _ => NotificationChannel::Webhook,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    Dashboard,
    Feeds,
    Alerts,
    Keywords,
    Tags,
    Logs,
    Settings,
}

impl std::fmt::Display for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Screen::Dashboard => write!(f, "Dashboard"),
            Screen::Feeds => write!(f, "Feeds"),
            Screen::Alerts => write!(f, "Alerts"),
            Screen::Keywords => write!(f, "Keywords"),
            Screen::Tags => write!(f, "Tags"),
            Screen::Logs => write!(f, "Logs"),
            Screen::Settings => write!(f, "Settings"),
        }
    }
}

// ── Core Structs ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Feed {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub feed_type: FeedType,
    pub enabled: bool,
    pub interval_secs: u64,
    pub last_fetch_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub content_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub api_template_id: Option<i64>,
    pub api_key: Option<String>,
    pub custom_headers: Option<String>,
    pub tor_proxy: Option<String>,
}

impl Feed {
    pub fn health_status(&self) -> FeedStatus {
        if !self.enabled {
            FeedStatus::Disabled
        } else if self.consecutive_failures >= 3 {
            FeedStatus::Error
        } else if self.consecutive_failures >= 1 {
            FeedStatus::Warning
        } else {
            FeedStatus::Healthy
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTemplate {
    pub id: i64,
    pub name: String,
    pub jsonpath_title: String,
    pub jsonpath_description: String,
    pub jsonpath_date: String,
    pub jsonpath_url: String,
    pub jsonpath_source: String,
    pub pagination_config: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Keyword {
    pub id: i64,
    pub pattern: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub criticality: Criticality,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl Keyword {
    pub fn match_type(&self) -> KeywordMatchType {
        if self.is_regex {
            KeywordMatchType::Regex
        } else {
            KeywordMatchType::Simple
        }
    }
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: i64,
    pub feed_id: i64,
    pub keyword_id: i64,
    pub title: Option<String>,
    pub content_snippet: String,
    pub criticality: Criticality,
    pub read: bool,
    pub content_hash: String,
    pub detected_at: DateTime<Utc>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub id: i64,
    pub name: String,
    pub channel: NotificationChannel,
    pub config_json: String,
    pub enabled: bool,
    pub min_criticality: Criticality,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FeedHealthLog {
    pub id: i64,
    pub feed_id: i64,
    pub status: FeedStatus,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

// ── Composite / UI Structs ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AlertWithMeta {
    pub alert: Alert,
    pub feed_name: String,
    pub keyword_pattern: String,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone)]
pub struct FeedWithTags {
    pub feed: Feed,
    pub tags: Vec<Tag>,
    pub status: FeedStatus,
}

// ── Feed Engine Structs ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FeedResult {
    pub content_hash: String,
    pub items: Vec<FeedItem>,
    pub raw_content: String,
}

#[derive(Debug, Clone, Default)]
pub struct FeedItem {
    pub title: Option<String>,
    pub description: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub source: Option<String>,
    pub raw_json: Option<String>,
}

// ── Keyword Engine Structs ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub keyword_id: i64,
    pub pattern: String,
    pub criticality: Criticality,
    pub matched_text: String,
    pub position: (usize, usize),
}

// ── Notification Structs ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
}

#[derive(Debug, Clone)]
pub struct DiscordEmbed {
    pub title: String,
    pub description: String,
    pub color: u32,
    pub fields: Vec<(String, String, bool)>,
}

// ── Stats ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub total_feeds: i64,
    pub total_alerts: i64,
    pub total_keywords: i64,
    pub unread_alerts: i64,
    pub healthy_feeds: i64,
    pub warning_feeds: i64,
    pub error_feeds: i64,
    pub disabled_feeds: i64,
}

// ── Form / Filter Structs ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct FeedForm {
    pub name: String,
    pub url: String,
    pub feed_type: FeedType,
    pub interval_secs: u64,
    pub enabled: bool,
    pub api_template_id: Option<i64>,
    pub api_key: String,
    pub custom_headers: String,
    pub tor_proxy: String,
}

#[derive(Debug, Clone, Default)]
pub struct KeywordForm {
    pub pattern: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub criticality: Criticality,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TagForm {
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationForm {
    pub name: String,
    pub channel: NotificationChannel,
    pub config_json: String,
    pub enabled: bool,
    pub min_criticality: Criticality,
}

// ── App State Enums ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub enum ConfirmDialog {
    DeleteFeed { id: i64, name: String },
    DeleteKeyword { id: i64, pattern: String },
    DeleteTag { id: i64, name: String },
    DeleteAlert { id: i64 },
    DeleteOldAlerts { cutoff: DateTime<Utc>, count: u64 },
    DeleteNotification { id: i64, name: String },
    BulkDeleteAlerts { count: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Notifications,
}

#[derive(Debug, Clone)]
pub enum TagAssignmentTarget {
    Feed(i64),
    Keyword(i64),
    Alert(i64),
}
