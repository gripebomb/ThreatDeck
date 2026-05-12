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

CREATE TABLE IF NOT EXISTS keywords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern TEXT NOT NULL,
    is_regex INTEGER NOT NULL DEFAULT 0,
    case_sensitive INTEGER NOT NULL DEFAULT 0,
    criticality TEXT NOT NULL CHECK(criticality IN ('Low','Medium','High','Critical')),
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

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
    status TEXT NOT NULL DEFAULT 'New',
    disposition TEXT NOT NULL DEFAULT 'Unknown',
    severity_override TEXT,
    confidence_score INTEGER,
    owner TEXT,
    triage_notes TEXT,
    acknowledged_at TEXT,
    investigating_at TEXT,
    escalated_at TEXT,
    closed_at TEXT,
    closed_reason TEXT,
    FOREIGN KEY (feed_id) REFERENCES feeds(id),
    FOREIGN KEY (keyword_id) REFERENCES keywords(id)
);
CREATE INDEX IF NOT EXISTS idx_alerts_feed ON alerts(feed_id);
CREATE INDEX IF NOT EXISTS idx_alerts_keyword ON alerts(keyword_id);
CREATE INDEX IF NOT EXISTS idx_alerts_detected ON alerts(detected_at);
CREATE INDEX IF NOT EXISTS idx_alerts_read ON alerts(read);
CREATE INDEX IF NOT EXISTS idx_alerts_hash ON alerts(content_hash);
CREATE INDEX IF NOT EXISTS idx_alerts_status ON alerts(status);
CREATE INDEX IF NOT EXISTS idx_alerts_disposition ON alerts(disposition);
CREATE INDEX IF NOT EXISTS idx_alerts_owner ON alerts(owner);
CREATE INDEX IF NOT EXISTS idx_alerts_closed_at ON alerts(closed_at);

CREATE TABLE IF NOT EXISTS indicators (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    indicator_type TEXT NOT NULL,
    value TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    first_seen_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sighting_count INTEGER NOT NULL DEFAULT 1,
    confidence_score INTEGER,
    risk_score INTEGER,
    metadata_json TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(indicator_type, normalized_value)
);
CREATE INDEX IF NOT EXISTS idx_indicators_type ON indicators(indicator_type);
CREATE INDEX IF NOT EXISTS idx_indicators_normalized ON indicators(normalized_value);
CREATE INDEX IF NOT EXISTS idx_indicators_last_seen ON indicators(last_seen_at DESC);

CREATE TABLE IF NOT EXISTS indicator_occurrences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    indicator_id INTEGER NOT NULL,
    content_item_id INTEGER,
    alert_id INTEGER,
    feed_id INTEGER,
    source_field TEXT,
    start_offset INTEGER,
    end_offset INTEGER,
    surrounding_text TEXT,
    detected_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(indicator_id) REFERENCES indicators(id) ON DELETE CASCADE,
    FOREIGN KEY(content_item_id) REFERENCES feed_items(id) ON DELETE CASCADE,
    FOREIGN KEY(alert_id) REFERENCES alerts(id) ON DELETE CASCADE,
    FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_indicator_occurrences_indicator ON indicator_occurrences(indicator_id);
CREATE INDEX IF NOT EXISTS idx_indicator_occurrences_alert ON indicator_occurrences(alert_id);
CREATE INDEX IF NOT EXISTS idx_indicator_occurrences_content_item ON indicator_occurrences(content_item_id);
CREATE INDEX IF NOT EXISTS idx_indicator_occurrences_feed ON indicator_occurrences(feed_id);

CREATE TABLE IF NOT EXISTS alert_indicators (
    alert_id INTEGER NOT NULL,
    indicator_id INTEGER NOT NULL,
    relationship TEXT NOT NULL DEFAULT 'observed',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(alert_id, indicator_id),
    FOREIGN KEY(alert_id) REFERENCES alerts(id) ON DELETE CASCADE,
    FOREIGN KEY(indicator_id) REFERENCES indicators(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_alert_indicators_indicator ON alert_indicators(indicator_id);

CREATE TABLE IF NOT EXISTS enrichment_providers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    provider_type TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    config_json TEXT,
    secret_ref TEXT,
    rate_limit_per_minute INTEGER,
    supports_types_json TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_enrichment_providers_enabled ON enrichment_providers(enabled);

CREATE TABLE IF NOT EXISTS enrichment_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    indicator_id INTEGER NOT NULL,
    provider_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_attempt_at TIMESTAMP,
    error_message TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(indicator_id) REFERENCES indicators(id) ON DELETE CASCADE,
    FOREIGN KEY(provider_id) REFERENCES enrichment_providers(id) ON DELETE CASCADE,
    UNIQUE(indicator_id, provider_id)
);
CREATE INDEX IF NOT EXISTS idx_enrichment_jobs_status_next ON enrichment_jobs(status, next_attempt_at, priority);
CREATE INDEX IF NOT EXISTS idx_enrichment_jobs_indicator ON enrichment_jobs(indicator_id);
CREATE INDEX IF NOT EXISTS idx_enrichment_jobs_provider ON enrichment_jobs(provider_id);

CREATE TABLE IF NOT EXISTS enrichment_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    indicator_id INTEGER NOT NULL,
    provider_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    reputation TEXT,
    score INTEGER,
    verdict TEXT,
    summary TEXT,
    raw_json TEXT,
    fetched_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(indicator_id) REFERENCES indicators(id) ON DELETE CASCADE,
    FOREIGN KEY(provider_id) REFERENCES enrichment_providers(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_enrichment_results_indicator ON enrichment_results(indicator_id, fetched_at DESC);
CREATE INDEX IF NOT EXISTS idx_enrichment_results_provider ON enrichment_results(provider_id);

CREATE TABLE IF NOT EXISTS feed_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    url TEXT,
    author TEXT,
    summary TEXT,
    content TEXT,
    published_at TIMESTAMP,
    fetched_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    content_hash TEXT NOT NULL UNIQUE,
    read INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT,
    FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_feed_items_feed ON feed_items(feed_id, published_at);
CREATE INDEX IF NOT EXISTS idx_feed_items_published ON feed_items(published_at DESC, fetched_at DESC);
CREATE INDEX IF NOT EXISTS idx_feed_items_read ON feed_items(read);
CREATE INDEX IF NOT EXISTS idx_feed_items_hash ON feed_items(content_hash);

CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#64B5F6',
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS feed_tags (
    feed_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (feed_id, tag_id),
    FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS keyword_tags (
    keyword_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (keyword_id, tag_id),
    FOREIGN KEY (keyword_id) REFERENCES keywords(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS alert_tags (
    alert_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (alert_id, tag_id),
    FOREIGN KEY (alert_id) REFERENCES alerts(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    channel TEXT NOT NULL CHECK(channel IN ('Email','Webhook','Discord')),
    config_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    min_criticality TEXT NOT NULL DEFAULT 'Low' CHECK(min_criticality IN ('Low','Medium','High','Critical')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS feed_health_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('Healthy','Warning','Error','Disabled')),
    error_message TEXT,
    checked_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_health_logs_feed ON feed_health_logs(feed_id, checked_at);

CREATE TABLE IF NOT EXISTS alert_triage_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_id INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    note TEXT,
    actor TEXT NOT NULL DEFAULT 'local',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(alert_id) REFERENCES alerts(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_triage_events_alert ON alert_triage_events(alert_id, created_at);

CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Default data
INSERT OR IGNORE INTO tags (name, color, description) VALUES 
    ('X', '#1DA1F2', 'X (Twitter) feeds'),
    ('Ransomware Gang', '#FF6B6B', 'Dark web ransomware sources'),
    ('API', '#4CAF50', 'REST API feeds'),
    ('News', '#FF9800', 'General security news');

INSERT OR IGNORE INTO api_templates (name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source) VALUES
    ('Ransomfeed.it', '$.post_title', '$.description', '$.discovered', '$.source', '$.group'),
    ('RansomLook', '$.name', '$.description', '$.published', '$.url', '$.group_name');
