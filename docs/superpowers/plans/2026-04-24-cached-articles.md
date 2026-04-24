# Cached Articles Feed Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a cached Articles tab that stores normalized RSS/API feed items and lets users browse and read them in the TUI.

**Architecture:** Add persistent `feed_items` storage below the existing feed and alert model, then expose it through `Db` query methods and a new `Articles` screen. The first article reader renders cleaned plain text in-terminal and keeps keyword alerting separate from full-feed article storage.

**Tech Stack:** Rust, rusqlite, ratatui, crossterm, chrono, serde_json, existing ThreatDeck TUI patterns.

---

## File Structure

- Modify `src/schema.sql`: create `feed_items` table and indexes.
- Modify `src/types.rs`: add persisted article structs and `Screen::Articles`.
- Modify `src/db.rs`: add insert/list/read methods and tests.
- Create `src/ui/articles.rs`: article list and reader UI.
- Modify `src/ui/mod.rs`: register Articles module, tab labels, selected tab, help text.
- Modify `src/app.rs`: add article state, refresh methods, navigation, filtering, and key dispatch.
- Modify feed-fetching code where RSS items are normalized: save `feed_items` before alert matching. If the current scheduler/fetcher is scaffolded, add the storage call to the closest normalization boundary and test at `Db` level.
- Modify `README.md` or `TUI_DESIGN.md`: document the new tab briefly if existing docs list tabs.

---

## Chunk 1: Database And Types

### Task 1: Add Article Storage Schema

**Files:**
- Modify: `src/schema.sql`
- Modify: `src/types.rs`
- Modify: `src/db.rs`

- [ ] **Step 1: Write failing DB tests**

Add tests in `src/db.rs`:

```rust
#[test]
fn feed_items_insert_idempotently_and_list_newest_first() {
    let db = temp_db();
    db.init_schema().unwrap();
    let feed_id = db.add_feed(NewFeed {
        name: "Example".into(),
        url: "https://example.com/feed.xml".into(),
        feed_type: FeedType::Rss,
        enabled: true,
        interval_secs: 300,
        api_template_id: None,
        api_key: None,
        custom_headers: None,
        tor_proxy: None,
    }).unwrap();

    let item = NewFeedItem {
        feed_id,
        title: "First item".into(),
        url: Some("https://example.com/1".into()),
        author: None,
        summary: Some("<p>Hello&nbsp;world</p>".into()),
        content: None,
        published_at: None,
        content_hash: "hash-1".into(),
        metadata_json: None,
    };

    db.upsert_feed_item(&item).unwrap();
    db.upsert_feed_item(&item).unwrap();

    let items = db.list_feed_items(&FeedItemFilter::default()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item.title, "First item");
    assert_eq!(items[0].feed_name, "Example");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `RUSTFLAGS="-Awarnings" cargo test feed_items_insert_idempotently_and_list_newest_first`

Expected: FAIL because `NewFeedItem`, `FeedItemFilter`, and DB methods do not exist.

- [ ] **Step 3: Add schema and structs**

Add to `src/schema.sql`:

```sql
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
```

Add article structs to `src/types.rs`:

```rust
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
    pub content_hash: String,
    pub read: bool,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FeedItemWithFeed {
    pub item: FeedItem,
    pub feed_name: String,
}

#[derive(Debug, Clone)]
pub struct NewFeedItem {
    pub feed_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub content_hash: String,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct FeedItemFilter {
    pub text: Option<String>,
    pub unread_only: bool,
    pub feed_id: Option<i64>,
    pub limit: Option<usize>,
}
```

- [ ] **Step 4: Add DB methods**

Implement in `src/db.rs`:

```rust
pub fn upsert_feed_item(&self, item: &NewFeedItem) -> Result<i64> {
    self.conn.execute(
        "INSERT OR IGNORE INTO feed_items
         (feed_id, title, url, author, summary, content, published_at, content_hash, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            item.feed_id,
            item.title,
            item.url,
            item.author,
            item.summary,
            item.content,
            item.published_at.map(|dt| dt.to_rfc3339()),
            item.content_hash,
            item.metadata_json,
        ],
    )?;
    self.conn.query_row(
        "SELECT id FROM feed_items WHERE content_hash = ?1",
        [&item.content_hash],
        |row| row.get(0),
    ).map_err(Into::into)
}
```

Also implement `list_feed_items`, `get_feed_item`, and `mark_feed_item_read`.

- [ ] **Step 5: Run tests**

Run: `RUSTFLAGS="-Awarnings" cargo test feed_items`

Expected: PASS.

---

## Chunk 2: Articles TUI

### Task 2: Add Articles Screen State And Navigation

**Files:**
- Modify: `src/types.rs`
- Modify: `src/app.rs`
- Modify: `src/ui/mod.rs`
- Create: `src/ui/articles.rs`

- [ ] **Step 1: Add screen enum and app state**

Add `Screen::Articles` between `Alerts` and `Keywords`. Add app fields:

```rust
pub articles_list: Vec<FeedItemWithFeed>,
pub articles_selected: usize,
pub articles_filter: String,
pub articles_unread_only: bool,
pub articles_reader: bool,
pub articles_scroll: u16,
```

- [ ] **Step 2: Wire navigation**

Update numeric navigation to:

- `1` Dashboard
- `2` Feeds
- `3` Alerts
- `4` Articles
- `5` Keywords
- `6` Tags
- `7` Logs
- `8` Settings

Update global help text and tab labels.

- [ ] **Step 3: Implement list UI**

Create `src/ui/articles.rs` with a table layout matching existing screens:

- title: `Articles | Filter: ... | Unread only`
- columns: status, published, feed, title
- footer: `[Enter] Read  [r] Toggle read  [u] Unread only  [/] Filter  [Esc] Back`

- [ ] **Step 4: Implement reader UI**

Render selected article in a full-screen panel. Use wrapped paragraph text with `articles_scroll`. Header includes feed, date, and URL.

- [ ] **Step 5: Implement key handling**

Normal list:

- movement via existing `move_selection`
- `Enter`: open reader, mark selected item read, reset scroll
- `r`: toggle read
- `u`: toggle unread filter and refresh

Reader:

- `Esc`: close reader
- `j/k`, arrows, PageUp/PageDown, Ctrl+d/u: scroll

- [ ] **Step 6: Run tests/check**

Run: `RUSTFLAGS="-Awarnings" cargo test`

Expected: PASS.

---

## Chunk 3: Article Text Cleanup And Fetch Integration

### Task 3: Render Clean Article Text

**Files:**
- Modify: `src/ui/articles.rs`
- Test: add unit tests in `src/ui/articles.rs`

- [ ] **Step 1: Write cleanup tests**

```rust
#[test]
fn clean_article_text_strips_html_and_entities() {
    assert_eq!(
        clean_article_text("<p>Hello&nbsp;<b>world</b>&amp; teams</p>"),
        "Hello world & teams"
    );
}
```

- [ ] **Step 2: Implement cleanup helper**

Implement simple cleanup:

- replace common block tags with newlines
- remove remaining tags
- decode common entities: `&nbsp;`, `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`
- collapse repeated whitespace while preserving paragraph breaks

- [ ] **Step 3: Run cleanup tests**

Run: `RUSTFLAGS="-Awarnings" cargo test clean_article_text`

Expected: PASS.

### Task 4: Save Normalized Items During Feed Processing

**Files:**
- Modify feed fetch or scheduler module that currently parses RSS/API items.
- Modify/add tests near that module if existing tests are present.

- [ ] **Step 1: Locate normalization boundary**

Run: `rg -n "FeedItem|rss|channel|items|insert_alert|add_alert|content_hash|fetch" src`

Use the place where remote feed entries are already normalized for keyword matching.

- [ ] **Step 2: Add storage call**

For each normalized item, build `NewFeedItem` and call `db.upsert_feed_item`. Use the existing item hash if available; otherwise hash `feed_id + url + title + published_at`.

- [ ] **Step 3: Keep alert behavior unchanged**

Do not require a keyword match to store the article. Continue creating alerts only when keywords match.

- [ ] **Step 4: Run verification**

Run:

```bash
RUSTFLAGS="-Awarnings" cargo test
cargo build --release
```

Expected: all tests pass and release build succeeds.

---

## Chunk 4: Docs And Final Verification

### Task 5: Update Docs

**Files:**
- Modify: `README.md`
- Modify: `TUI_DESIGN.md` if the tab list appears there

- [ ] **Step 1: Document Articles tab**

Add one concise section describing:

- full cached feed
- Enter opens reader
- `/` filters articles
- read/unread state

- [ ] **Step 2: Check old navigation references**

Run: `rg -n "\\[1-7\\]|1-7|Settings|Articles" README.md TUI_DESIGN.md SPEC.md src`

Update stale references to include `[8] Settings` where needed.

- [ ] **Step 3: Final verification**

Run:

```bash
RUSTFLAGS="-Awarnings" cargo test
cargo build --release
./target/release/ThreatDeck --config-paths
```

Expected: tests pass, release build succeeds, config paths show ThreatDeck paths.
