# SPEC.md — ThreatDeck TUI

## 1. Overview

ThreatDeck is a terminal-based threat intelligence monitoring and alerting platform designed for SOCs, security researchers, and threat intelligence analysts. It aggregates intelligence from multiple feed sources (API, RSS, Website, Onion), matches content against configurable keywords with criticality levels, generates alerts, and delivers notifications via multiple channels.

**Crate name**: `ThreatDeck`  
**Binary name**: `ThreatDeck`  
**Edition**: 2021  
**License**: MIT

---

## 2. Dependencies

```toml
[dependencies]
ratatui = { version = "0.30", features = ["crossterm"] }
crossterm = "0.29"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
quick-xml = { version = "0.39", features = ["serialize"] }
clap = { version = "4", features = ["derive"] }
directories = "5"
rusqlite = { version = "0.32", features = ["bundled", "chrono"] }
ureq = { version = "2", features = ["json", "charset"] }
serde_json = "1"
jsonpath_lib = "0.3"
regex = "1"
sha2 = "0.10"
hex = "0.4"
lettre = { version = "0.11", features = ["smtp-tokio-rustls", "builder"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["rt-multi-thread", "time", "sync"] }
scraper = "0.22"
atom_syndication = "0.12"
rss = "2"
```

---

## 3. Data Models

### 3.1 Enums

```rust
// src/types.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedType {
    Api,
    Rss,
    Website,
    Onion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedStatus {
    Healthy,   // Enabled + 0 consecutive failures
    Warning,   // Enabled + 1-2 consecutive failures
    Error,     // Enabled + 3+ consecutive failures
    Disabled,  // Manually disabled
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Criticality {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeywordMatchType {
    Simple,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email,
    Webhook,
    Discord,
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
```

### 3.2 Structs

```rust
// Feed — stored in SQLite
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
    pub custom_headers: Option<String>, // JSON string
    pub tor_proxy: Option<String>,      // e.g. "127.0.0.1:9050"
}

// ApiTemplate — stored in SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTemplate {
    pub id: i64,
    pub name: String,
    pub jsonpath_title: String,
    pub jsonpath_description: String,
    pub jsonpath_date: String,
    pub jsonpath_url: String,
    pub jsonpath_source: String,
    pub pagination_config: Option<String>, // JSON string
    pub created_at: DateTime<Utc>,
}

// Keyword — stored in SQLite
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

// Alert — stored in SQLite
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

// Tag — stored in SQLite
#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String, // hex color e.g. "#FF6B6B"
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

// NotificationConfig — stored in SQLite
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

// FeedHealthLog — stored in SQLite (last 100 entries per feed, rolling)
#[derive(Debug, Clone)]
pub struct FeedHealthLog {
    pub id: i64,
    pub feed_id: i64,
    pub status: FeedStatus,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

// AlertWithMeta — used in UI
#[derive(Debug, Clone)]
pub struct AlertWithMeta {
    pub alert: Alert,
    pub feed_name: String,
    pub keyword_pattern: String,
    pub tags: Vec<Tag>,
}

// FeedWithTags — used in UI
#[derive(Debug, Clone)]
pub struct FeedWithTags {
    pub feed: Feed,
    pub tags: Vec<Tag>,
    pub status: FeedStatus,
}
```

---

## 4. SQLite Schema

```sql
-- Feeds
CREATE TABLE IF NOT EXISTS feeds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    feed_type TEXT NOT NULL CHECK(feed_type IN ('Api','Rss','Website','Onion')),
    enabled INTEGER NOT NULL DEFAULT 1,
    interval_secs INTEGER NOT NULL DEFAULT 300,
    last_fetch_at TIMESTAMP,
    last_error TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    api_template_id INTEGER,
    api_key TEXT,
    custom_headers TEXT,
    tor_proxy TEXT,
    FOREIGN KEY (api_template_id) REFERENCES api_templates(id)
);

-- API Templates
CREATE TABLE IF NOT EXISTS api_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    jsonpath_title TEXT NOT NULL DEFAULT '$.title',
    jsonpath_description TEXT NOT NULL DEFAULT '$.description',
    jsonpath_date TEXT NOT NULL DEFAULT '$.date',
    jsonpath_url TEXT NOT NULL DEFAULT '$.url',
    jsonpath_source TEXT,
    pagination_config TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Keywords
CREATE TABLE IF NOT EXISTS keywords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern TEXT NOT NULL,
    is_regex INTEGER NOT NULL DEFAULT 0,
    case_sensitive INTEGER NOT NULL DEFAULT 0,
    criticality TEXT NOT NULL CHECK(criticality IN ('Low','Medium','High','Critical')),
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Alerts
CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    keyword_id INTEGER NOT NULL,
    title TEXT,
    content_snippet TEXT NOT NULL,
    criticality TEXT NOT NULL CHECK(criticality IN ('Low','Medium','High','Critical')),
    read INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT NOT NULL,
    detected_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    metadata_json TEXT,
    FOREIGN KEY (feed_id) REFERENCES feeds(id),
    FOREIGN KEY (keyword_id) REFERENCES keywords(id)
);
CREATE INDEX IF NOT EXISTS idx_alerts_feed ON alerts(feed_id);
CREATE INDEX IF NOT EXISTS idx_alerts_keyword ON alerts(keyword_id);
CREATE INDEX IF NOT EXISTS idx_alerts_detected ON alerts(detected_at);
CREATE INDEX IF NOT EXISTS idx_alerts_read ON alerts(read);

-- Tags
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#64B5F6',
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Many-to-many: Feed <-> Tag
CREATE TABLE IF NOT EXISTS feed_tags (
    feed_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (feed_id, tag_id),
    FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- Many-to-many: Keyword <-> Tag
CREATE TABLE IF NOT EXISTS keyword_tags (
    keyword_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (keyword_id, tag_id),
    FOREIGN KEY (keyword_id) REFERENCES keywords(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- Many-to-many: Alert <-> Tag
CREATE TABLE IF NOT EXISTS alert_tags (
    alert_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (alert_id, tag_id),
    FOREIGN KEY (alert_id) REFERENCES alerts(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- Notification Configurations
CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    channel TEXT NOT NULL CHECK(channel IN ('Email','Webhook','Discord')),
    config_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    min_criticality TEXT NOT NULL DEFAULT 'Low' CHECK(min_criticality IN ('Low','Medium','High','Critical')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Feed Health Logs (rolling, keep last 100 per feed)
CREATE TABLE IF NOT EXISTS feed_health_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('Healthy','Warning','Error','Disabled')),
    error_message TEXT,
    checked_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_health_logs_feed ON feed_health_logs(feed_id, checked_at);
```

**Default Data Inserted on First Run**:
```sql
-- Default tags
INSERT INTO tags (name, color, description) VALUES 
    ('X', '#1DA1F2', 'X (Twitter) feeds'),
    ('Ransomware Gang', '#FF6B6B', 'Dark web ransomware sources'),
    ('API', '#4CAF50', 'REST API feeds'),
    ('News', '#FF9800', 'General security news');

-- Default API templates
INSERT INTO api_templates (name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source) VALUES
    ('Ransomfeed.it', '$.post_title', '$.description', '$.discovered', '$.source', '$.group'),
    ('RansomLook', '$.name', '$.description', '$.published', '$.url', '$.group_name');
```

---

## 5. Module Specifications

### 5.1 `db` — Database Layer

**File**: `src/db.rs`

Public API:
```rust
pub struct Db {
    conn: rusqlite::Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn init_schema(&self) -> Result<()>;
    
    // Feeds
    pub fn create_feed(&self, feed: &FeedCreate) -> Result<i64>;
    pub fn get_feed(&self, id: i64) -> Result<Option<Feed>>;
    pub fn list_feeds(&self, filter: FeedFilter) -> Result<Vec<Feed>>;
    pub fn update_feed(&self, id: i64, feed: &FeedUpdate) -> Result<()>;
    pub fn delete_feed(&self, id: i64) -> Result<()>;
    pub fn update_feed_health(&self, id: i64, success: bool, error: Option<&str>, content_hash: Option<&str>) -> Result<()>;
    pub fn reset_feed_failures(&self, id: i64) -> Result<()>;
    
    // Templates
    pub fn create_template(&self, tmpl: &ApiTemplateCreate) -> Result<i64>;
    pub fn get_template(&self, id: i64) -> Result<Option<ApiTemplate>>;
    pub fn list_templates(&self) -> Result<Vec<ApiTemplate>>;
    
    // Keywords
    pub fn create_keyword(&self, kw: &KeywordCreate) -> Result<i64>;
    pub fn get_keyword(&self, id: i64) -> Result<Option<Keyword>>;
    pub fn list_keywords(&self, filter: KeywordFilter) -> Result<Vec<Keyword>>;
    pub fn update_keyword(&self, id: i64, kw: &KeywordUpdate) -> Result<()>;
    pub fn delete_keyword(&self, id: i64) -> Result<()>;
    
    // Alerts
    pub fn create_alert(&self, alert: &AlertCreate) -> Result<i64>;
    pub fn get_alert(&self, id: i64) -> Result<Option<Alert>>;
    pub fn list_alerts(&self, filter: AlertFilter) -> Result<Vec<AlertWithMeta>>;
    pub fn mark_alert_read(&self, id: i64, read: bool) -> Result<()>;
    pub fn mark_all_alerts_read(&self, read: bool) -> Result<()>;
    pub fn delete_old_alerts(&self, before: DateTime<Utc>) -> Result<u64>;
    pub fn get_alert_count(&self, filter: AlertFilter) -> Result<i64>;
    pub fn get_unread_alert_count(&self) -> Result<i64>;
    pub fn alert_exists_by_hash_window(&self, hash: &str, window: Duration) -> Result<bool>;
    pub fn get_criticality_distribution(&self) -> Result<Vec<(Criticality, i64)>>;
    pub fn get_top_keywords(&self, limit: usize) -> Result<Vec<(String, i64)>>;
    pub fn get_alert_trend(&self, days: u32) -> Result<Vec<(String, i64)>>; // date -> count
    
    // Tags
    pub fn create_tag(&self, tag: &TagCreate) -> Result<i64>;
    pub fn get_tag(&self, id: i64) -> Result<Option<Tag>>;
    pub fn list_tags(&self) -> Result<Vec<Tag>>;
    pub fn update_tag(&self, id: i64, tag: &TagUpdate) -> Result<()>;
    pub fn delete_tag(&self, id: i64) -> Result<()>;
    pub fn get_feed_tags(&self, feed_id: i64) -> Result<Vec<Tag>>;
    pub fn get_keyword_tags(&self, keyword_id: i64) -> Result<Vec<Tag>>;
    pub fn get_alert_tags(&self, alert_id: i64) -> Result<Vec<Tag>>;
    pub fn assign_tag_to_feed(&self, feed_id: i64, tag_id: i64) -> Result<()>;
    pub fn remove_tag_from_feed(&self, feed_id: i64, tag_id: i64) -> Result<()>;
    pub fn assign_tag_to_keyword(&self, keyword_id: i64, tag_id: i64) -> Result<()>;
    pub fn remove_tag_from_keyword(&self, keyword_id: i64, tag_id: i64) -> Result<()>;
    pub fn assign_tag_to_alert(&self, alert_id: i64, tag_id: i64) -> Result<()>;
    pub fn remove_tag_from_alert(&self, alert_id: i64, tag_id: i64) -> Result<()>;
    
    // Notifications
    pub fn create_notification(&self, cfg: &NotificationCreate) -> Result<i64>;
    pub fn list_notifications(&self) -> Result<Vec<NotificationConfig>>;
    pub fn update_notification(&self, id: i64, cfg: &NotificationUpdate) -> Result<()>;
    pub fn delete_notification(&self, id: i64) -> Result<()>;
    
    // Health Logs
    pub fn add_health_log(&self, feed_id: i64, status: FeedStatus, error: Option<&str>) -> Result<()>;
    pub fn get_health_logs(&self, feed_id: Option<i64>, limit: usize) -> Result<Vec<FeedHealthLog>>;
    pub fn prune_health_logs(&self, feed_id: i64, keep: usize) -> Result<()>;
    
    // Stats
    pub fn get_stats(&self) -> Result<Stats>;
    pub fn get_feed_health_ratio(&self) -> Result<f64>;
}
```

### 5.2 `config` — Configuration

**File**: `src/config.rs`

Uses `directories` crate for platform-appropriate paths:
- Config dir: `~/.config/ThreatDeck/`
- Data dir: `~/.local/share/ThreatDeck/`

Files:
- `config.toml` — App settings (theme, retention days, tick rate)
- `feeds.toml` — Optional bulk feed import
- Database: `data_dir/ThreatDeck.db`

```rust
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: String,
    pub alert_retention_days: u32,
    pub dashboard_refresh_secs: u64,
    pub tick_rate_ms: u64,
    pub max_health_log_entries: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            alert_retention_days: 30,
            dashboard_refresh_secs: 30,
            tick_rate_ms: 250,
            max_health_log_entries: 100,
        }
    }
}
```

### 5.3 `feed` — Feed Engine

**Files**: `src/feed/mod.rs`, `src/feed/api.rs`, `src/feed/rss.rs`, `src/feed/web.rs`, `src/feed/onion.rs`

```rust
// feed/mod.rs
pub struct FeedResult {
    pub content_hash: String,
    pub items: Vec<FeedItem>,
    pub raw_content: String,
}

pub struct FeedItem {
    pub title: Option<String>,
    pub description: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub source: Option<String>,
    pub raw_json: Option<String>, // for API feeds
}

pub trait FeedFetcher {
    fn fetch(&self, feed: &Feed) -> Result<FeedResult>;
}

pub struct FeedManager;
impl FeedManager {
    pub fn fetch_feed(feed: &Feed, db: &Db) -> Result<FeedResult> {
        let fetcher: Box<dyn FeedFetcher> = match feed.feed_type {
            FeedType::Api => Box::new(ApiFetcher),
            FeedType::Rss => Box::new(RssFetcher),
            FeedType::Website => Box::new(WebFetcher),
            FeedType::Onion => Box::new(OnionFetcher),
        };
        fetcher.fetch(feed)
    }
}
```

**ApiFetcher**:
- Uses `ureq` for HTTP GET
- Applies `api_key` as custom header if present
- Applies `custom_headers` (parsed from JSON) 
- If `api_template_id` is set, uses JSONPath expressions to extract items from JSON array
- Returns array of `FeedItem`s
- For pagination, follow `pagination_config` rules

**RssFetcher**:
- Uses `ureq` to fetch XML
- Parses with `quick-xml` + `rss` crate for RSS 2.0
- Also supports Atom via `atom_syndication` crate
- Extracts title, description, pubDate, link for each item

**WebFetcher**:
- Uses `ureq` to fetch HTML
- Uses `scraper` (CSS selector engine) to extract text content
- Compares SHA256 hash of extracted text for change detection
- If changed, returns the new content as a single FeedItem

**OnionFetcher**:
- Uses `ureq` with proxy configured to `tor_proxy` (SOCKS5)
- Otherwise same as WebFetcher

### 5.4 `template` — API Template System

**File**: `src/template.rs`

```rust
pub struct TemplateEngine;

impl TemplateEngine {
    pub fn extract_items(
        json: &serde_json::Value,
        template: &ApiTemplate,
    ) -> Result<Vec<FeedItem>> {
        // Use jsonpath_lib to extract fields
        // Apply pagination if configured
    }
    
    pub fn validate_template(template: &ApiTemplate) -> Result<()> {
        // Ensure all JSONPath expressions are syntactically valid
    }
}
```

### 5.5 `scheduler` — Background Scheduler

**File**: `src/scheduler.rs`

Tick-based scheduler similar to netscan. Runs in a background thread managed by tokio runtime.

```rust
pub struct FeedScheduler {
    entries: Vec<ScheduleEntry>,
}

struct ScheduleEntry {
    feed_id: i64,
    interval_secs: u64,
    next_run: Instant,
}

impl FeedScheduler {
    pub fn new(feeds: &[Feed]) -> Self;
    pub fn tick(&mut self, now: Instant) -> Vec<i64>; // returns feed_ids to fetch
    pub fn update_feed(&mut self, feed: &Feed);
    pub fn add_feed(&mut self, feed: &Feed);
    pub fn remove_feed(&mut self, feed_id: i64);
}
```

### 5.6 `keyword` — Keyword Matching

**File**: `src/keyword.rs`

```rust
pub struct MatchResult {
    pub keyword_id: i64,
    pub pattern: String,
    pub criticality: Criticality,
    pub matched_text: String,
    pub position: (usize, usize),
}

pub struct KeywordEngine;

impl KeywordEngine {
    pub fn check_content(content: &str, keywords: &[Keyword]) -> Vec<MatchResult>;
    pub fn check_feed_item(item: &FeedItem, keywords: &[Keyword]) -> Vec<MatchResult>;
    pub fn compile_regex(pattern: &str, case_sensitive: bool) -> Result<Regex>;
}
```

Rules:
- If `enabled == false`, skip keyword
- If `is_regex == true`, compile regex and test
- If `is_regex == false`, do substring search
- Respect `case_sensitive` flag
- Return all matches, not just first

### 5.7 `alert` — Alert Generation

**File**: `src/alert.rs`

```rust
pub struct AlertEngine {
    db: Arc<Mutex<Db>>,
}

impl AlertEngine {
    pub fn process_feed_result(
        &self,
        feed: &Feed,
        result: &FeedResult,
    ) -> Result<Vec<Alert>> {
        // 1. Load all enabled keywords
        // 2. For each FeedItem, check against keywords
        // 3. For each match:
        //    a. Compute content_hash for dedup
        //    b. Check if alert_exists_by_hash_window(hash, 1 hour)
        //    c. If not duplicate, create alert with snippet
        //    d. Store metadata_json from API response if available
        // 4. Return created alerts
    }
    
    pub fn check_historical_for_keyword(&self, keyword: &Keyword) -> Result<Vec<Alert>> {
        // When a new keyword is created, scan all feeds that were checked
        // AFTER the keyword's created_at time
    }
}
```

### 5.8 `tag` — Tag Management

**File**: `src/tag.rs`

Thin wrapper around DB operations. No additional logic.

### 5.9 `notify` — Notifications

**File**: `src/notify.rs`

```rust
pub struct NotifyEngine {
    db: Arc<Mutex<Db>>,
}

pub enum NotificationPayload {
    Email { to: Vec<String>, subject: String, body: String },
    Webhook { url: String, headers: HashMap<String, String>, body: String },
    Discord { webhook_url: String, content: String, embeds: Vec<DiscordEmbed> },
}

impl NotifyEngine {
    pub fn send_for_alert(&self, alert: &Alert, feed: &Feed, keyword: &Keyword) -> Result<()> {
        // 1. Load all enabled notification configs where min_criticality <= alert.criticality
        // 2. For each config, build payload and send
    }
    
    pub fn send_email(&self, cfg: &EmailConfig, payload: &NotificationPayload) -> Result<()>;
    pub fn send_webhook(&self, cfg: &WebhookConfig, payload: &NotificationPayload) -> Result<()>;
    pub fn send_discord(&self, cfg: &DiscordConfig, payload: &NotificationPayload) -> Result<()>;
}
```

Config JSON schemas:
```rust
// Email
{ "smtp_server": "smtp.gmail.com", "smtp_port": 587, "username": "...", "password": "...", "from": "alerts@example.com", "to": ["soc@example.com"] }

// Webhook
{ "url": "https://hooks.example.com/threatintel", "headers": { "Authorization": "Bearer token" } }

// Discord
{ "webhook_url": "https://discord.com/api/webhooks/..." }
```

---

## 6. TUI Architecture

### 6.1 App State Machine

**File**: `src/app.rs`

```rust
pub struct App {
    pub screen: Screen,
    pub prev_screen: Option<Screen>,
    pub db: Arc<Mutex<Db>>,
    pub config: AppConfig,
    pub paths: Paths,
    pub theme: Theme,
    pub running: bool,
    pub last_tick: Instant,
    pub last_dashboard_refresh: Instant,
    pub notification: Option<(String, NotificationType)>, // toast message
    pub show_help: bool,
    pub show_confirm: Option<ConfirmDialog>,
    
    // Dashboard
    pub dashboard_stats: Option<Stats>,
    pub dashboard_recent_alerts: Vec<AlertWithMeta>,
    pub dashboard_criticality_data: Vec<(Criticality, i64)>,
    
    // Feeds screen
    pub feeds_list: Vec<FeedWithTags>,
    pub feeds_selected: usize,
    pub feeds_filter: String,
    pub feeds_show_form: bool,
    pub feeds_form: FeedForm,
    pub feeds_form_edit_id: Option<i64>,
    pub feeds_detail_view: bool,
    
    // Alerts screen
    pub alerts_list: Vec<AlertWithMeta>,
    pub alerts_selected: usize,
    pub alerts_filter: String,
    pub alerts_filter_criticality: Option<Criticality>,
    pub alerts_filter_unread_only: bool,
    pub alerts_detail_view: bool,
    pub alerts_bulk_mode: bool,
    pub alerts_selected_bulk: HashSet<i64>,
    
    // Keywords screen
    pub keywords_list: Vec<Keyword>,
    pub keywords_selected: usize,
    pub keywords_show_form: bool,
    pub keywords_form: KeywordForm,
    pub keywords_form_edit_id: Option<i64>,
    pub keywords_test_input: String,
    pub keywords_test_results: Vec<MatchResult>,
    
    // Tags screen
    pub tags_list: Vec<Tag>,
    pub tags_selected: usize,
    pub tags_show_form: bool,
    pub tags_form: TagForm,
    pub tags_form_edit_id: Option<i64>,
    pub tags_assignment_mode: bool,
    pub tags_assignment_target: TagAssignmentTarget,
    
    // Logs screen
    pub logs_list: Vec<FeedHealthLog>,
    pub logs_selected: usize,
    pub logs_filter_feed: Option<i64>,
    
    // Settings screen
    pub settings_tab: SettingsTab,
    pub settings_retention_days: u32,
    pub settings_theme_name: String,
    pub settings_notifications: Vec<NotificationConfig>,
    pub settings_notif_form: NotificationForm,
    pub settings_notif_form_edit_id: Option<i64>,
    pub settings_cleanup_preview: Option<DateTime<Utc>>,
}

pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
}

pub enum ConfirmDialog {
    DeleteFeed { id: i64, name: String },
    DeleteKeyword { id: i64, pattern: String },
    DeleteTag { id: i64, name: String },
    DeleteAlert { id: i64 },
    DeleteOldAlerts { cutoff: DateTime<Utc>, count: u64 },
    DeleteNotification { id: i64, name: String },
}

pub enum SettingsTab {
    General,
    Notifications,
}

pub enum TagAssignmentTarget {
    Feed(i64),
    Keyword(i64),
    Alert(i64),
}
```

### 6.2 Event Loop

**File**: `src/main.rs`

```rust
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(app.config.tick_rate_ms);
    let dashboard_refresh = Duration::from_secs(app.config.dashboard_refresh_secs);
    let mut last_tick = Instant::now();
    let mut last_dashboard_refresh = Instant::now();
    
    while app.running {
        // Draw
        terminal.draw(|f| ui::draw(f, app))?;
        
        // Handle events with timeout
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            match crossterm::event::read()? {
                Event::Key(key) => handle_key_event(key, app),
                Event::Mouse(mouse) => handle_mouse_event(mouse, app),
                _ => {}
            }
        }
        
        // Tick
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
        
        // Dashboard refresh
        if last_dashboard_refresh.elapsed() >= dashboard_refresh {
            app.refresh_dashboard();
            last_dashboard_refresh = Instant::now();
        }
    }
    Ok(())
}
```

### 6.3 Key Bindings (Global)

| Key | Action |
|-----|--------|
| `q` or `Ctrl+c` | Quit (if not in form) |
| `Tab` / `Shift+Tab` | Navigate forward/backward through fields |
| `1-8` | Jump to screen (Dashboard, Feeds, Alerts, Articles, Keywords, Tags, Logs, Settings) |
| `F1` or `?` | Toggle help overlay |
| `Esc` | Cancel form / Close popup / Go back |

**Screen-specific bindings** documented in respective UI modules.

### 6.4 Theme System

**File**: `src/theme.rs`

5 built-in themes: `dark`, `light`, `solarized`, `dracula`, `monokai`

```rust
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
    pub highlight: Color,
    pub muted: Color,
}
```

Feed status colors:
- Healthy → `theme.success`
- Warning → `theme.warning`
- Error → `theme.error`
- Disabled → `theme.muted`

Criticality colors:
- Low → Blue
- Medium → Yellow/Orange
- High → Orange/Red
- Critical → Bright Red

---

## 7. UI Screen Specifications

### 7.1 Dashboard (`ui/dashboard.rs`)

Layout: Single page with tiles.

**Top row — Statistics tiles** (4 tiles across):
1. **Total Feeds** — count from DB, with healthy ratio subtext
2. **Total Alerts** — count, with unread count highlighted
3. **Total Keywords** — count, with enabled count subtext
4. **Healthy Feeds** — percentage + count

**Middle row — Two columns**:
- Left: **Criticality Distribution** — Pie chart (block characters) showing Low/Medium/High/Critical counts. Clickable (navigates to Alerts with filter).
- Right: **Recent Alerts** — Scrollable list of last 5 alerts with criticality color strip, feed name, keyword, time ago.

**Bottom row — Activity sparkline** (optional): Alert counts over last 7 days.

**Interactions**:
- `r` — manual refresh
- `Enter` on recent alert → jump to Alerts screen with that alert selected
- `Enter` on pie slice → jump to Alerts filtered by criticality

### 7.2 Feeds (`ui/feeds.rs`)

Layout: Table on top (60%), detail/form on bottom (40%) when active.

**Table columns**:
| Status | Name | Type | Interval | Last Fetch | Failures | Tags |

Status is colored dot + text.

**Key bindings**:
- `a` / `n` — Add new feed (opens form)
- `e` — Edit selected feed
- `d` — Delete selected feed (with confirm)
- `Enter` — Toggle detail view / Toggle enable/disable
- `m` — Manual trigger fetch now
- `t` — Assign tags
- `/` — Filter by name
- `s` — Sort cycle (name/type/status/last_fetch)

**Feed Form fields**:
- Name (text)
- URL (text)
- Type (dropdown: API, RSS, Website, Onion)
- Interval seconds (number, min 60)
- Enabled (checkbox)
- API Template (dropdown, only for API type)
- API Key (password-style text, only for API type)
- Custom Headers (textarea JSON, only for API type)
- Tor Proxy (text, only for Onion type)

### 7.3 Alerts (`ui/alerts.rs`)

Layout: Table with filters at top.

**Table columns**:
| Read | Criticality | Feed | Keyword | Snippet | Detected | Tags |

Read status: `●` (unread) vs `○` (read)

**Filters bar** (above table):
- Text filter (name/keyword/content)
- Criticality dropdown filter
- Unread only toggle
- Tag filter dropdown

**Key bindings**:
- `r` — Toggle read/unread selected
- `R` — Mark all as read
- `d` — Delete selected
- `D` — Bulk delete mode
- `Space` — In bulk mode: toggle selection
- `a` — Select all in bulk mode
- `Enter` — View full alert detail
- `t` — Assign tags to alert
- `/` — Quick filter

**Alert detail view**:
- Full content snippet with syntax-highlighted matched keywords
- Metadata JSON pretty-printed (for API feeds)
- Source feed info
- Matched keyword info
- Timestamp

### 7.4 Keywords (`ui/keywords.rs`)

Layout: Table with test panel.

**Table columns**:
| Pattern | Type | Case | Criticality | Enabled | Tags |

**Key bindings**:
- `a` / `n` — Add keyword
- `e` — Edit
- `d` — Delete
- `t` — Test pattern against sample text
- `Enter` — Toggle enabled
- `s` — Sort cycle

**Test panel** (appears when `t` pressed):
- Textarea for sample input
- Results showing matches with positions
- Real-time as you type

**Keyword Form fields**:
- Pattern (text)
- Match Type (Simple / Regex)
- Case Sensitive (checkbox)
- Criticality (Low/Medium/High/Critical dropdown)
- Enabled (checkbox)

### 7.5 Tags (`ui/tags.rs`)

Layout: Table with assignment panel.

**Table columns**:
| Name | Color | Description | Usage Count |

**Key bindings**:
- `a` / `n` — Add tag
- `e` — Edit
- `d` — Delete
- `Enter` — View items with this tag

**Tag Form fields**:
- Name (text)
- Color (hex color picker via text input with preview block)
- Description (text)

### 7.6 Logs (`ui/logs.rs`)

Layout: Full-screen sortable DataGrid.

**Table columns**:
| Time | Feed | Status | Error Message |

**Key bindings**:
- `f` — Filter by feed
- `c` — Clear filter
- `s` — Sort cycle
- `r` — Refresh

### 7.7 Settings (`ui/settings.rs`)

Layout: Two tabs.

**General Tab**:
- Theme selector (dropdown)
- Dashboard refresh interval (seconds)
- Alert retention period (days)
- Cleanup old alerts button (with preview count)

**Notifications Tab**:
- Table of notification configs
- Add/Edit/Delete buttons
- Test button to send test notification

---

## 8. Feed Health Monitoring

Health status is computed dynamically from `consecutive_failures`:

```rust
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
```

On every fetch attempt:
1. If success: reset `consecutive_failures` to 0, clear `last_error`
2. If failure: increment `consecutive_failures`, set `last_error`
3. Always insert a `feed_health_logs` entry
4. After insert, prune old logs for that feed (keep last 100)

---

## 9. Alert Deduplication

Alerts are deduplicated by `content_hash` within a 1-hour window:

```rust
fn should_create_alert(db: &Db, content_hash: &str) -> Result<bool> {
    let exists = db.alert_exists_by_hash_window(content_hash, Duration::from_secs(3600))?;
    Ok(!exists)
}
```

`content_hash` is SHA256 of the normalized alert content (feed_id + keyword_id + title + snippet).

---

## 10. Demo Data

`demo-seed.sh` inserts sample data for testing:
- 6 feeds (2 API, 2 RSS, 1 Website, 1 Onion)
- 8 keywords (mixed simple/regex, all criticalities)
- 12 alerts with varied criticality
- All 4 default tags assigned appropriately

---

## 11. Error Handling Strategy

- All modules use `anyhow::Result` for error propagation
- DB errors are logged and surfaced as toast notifications
- Feed fetch failures are recorded in health logs, not fatal
- Notification delivery failures are logged but don't block alert creation
- UI forms validate before submit and show inline errors

---

## 12. Performance Considerations

- SQLite WAL mode enabled for concurrent reads
- Feed fetching uses tokio blocking threads (not async runtime)
- UI tick rate: 250ms (configurable)
- Dashboard refresh: 30s (configurable)
- Alert dedup window: 1 hour
- Health logs: rolling 100 per feed
- Alert retention: configurable (default 30 days)
- Content hashing: SHA256 on normalized strings
