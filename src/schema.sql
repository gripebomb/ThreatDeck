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
    FOREIGN KEY (feed_id) REFERENCES feeds(id),
    FOREIGN KEY (keyword_id) REFERENCES keywords(id)
);
CREATE INDEX IF NOT EXISTS idx_alerts_feed ON alerts(feed_id);
CREATE INDEX IF NOT EXISTS idx_alerts_keyword ON alerts(keyword_id);
CREATE INDEX IF NOT EXISTS idx_alerts_detected ON alerts(detected_at);
CREATE INDEX IF NOT EXISTS idx_alerts_read ON alerts(read);
CREATE INDEX IF NOT EXISTS idx_alerts_hash ON alerts(content_hash);

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
