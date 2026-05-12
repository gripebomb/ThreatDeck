# Auto-Fetch Background Thread — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire up a background thread that automatically fetches enabled feeds at a user-configurable interval (5–60 min, default 15), with on/off toggle and interval control in the Settings TUI.

**Architecture:** A dedicated `AutoFetcher` thread owns its own DB connection, sleeps for the configured interval, then fetches all enabled feeds sequentially and sends a summary back to the TUI via an `mpsc` channel. The main event loop polls the channel each tick.

**Tech Stack:** Rust, `std::thread`, `std::sync::mpsc`, rusqlite, ratatui, existing `FeedManager` + `AlertEngine`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/auto_fetch.rs` | NEW. `AutoFetchConfig`, `AutoFetchMessage`, `AutoFetcher` thread logic |
| `src/config.rs` | Add `[auto_fetch]` section to `AppConfig` |
| `src/app.rs` | Add fields, spawn/stop thread, handle messages in `on_tick` |
| `src/ui/settings.rs` | Add auto-fetch toggle + interval controls to General tab |
| `src/main.rs` | Wire thread spawn on start, graceful stop on exit |
| `src/db.rs` | Tests for auto-fetch integration |

---

## Chunk 1: Config + Data Structures

### Task 1: Add `AutoFetchConfig` to `src/config.rs`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add `AutoFetchConfig` struct**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoFetchConfig {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for AutoFetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 15,
        }
    }
}
```

- [ ] **Step 2: Add `auto_fetch` field to `AppConfig`**

Add to `AppConfig` struct:
```rust
pub auto_fetch: AutoFetchConfig,
```

Add to `AppConfig::default()`:
```rust
auto_fetch: AutoFetchConfig::default(),
```

- [ ] **Step 3: Write tests for config defaults and clamping**

```rust
#[test]
fn auto_fetch_config_defaults() {
    let config: AppConfig = toml::from_str("").unwrap();
    assert!(config.auto_fetch.enabled);
    assert_eq!(config.auto_fetch.interval_minutes, 15);
}

#[test]
fn auto_fetch_config_clamps_interval() {
    // The clamping logic will be in the UI handler, not config parsing.
    // This test just verifies the struct accepts the value as-is.
    let config: AppConfig = toml::from_str(r#"
[auto_fetch]
enabled = false
interval_minutes = 3
"#).unwrap();
    assert!(!config.auto_fetch.enabled);
    assert_eq!(config.auto_fetch.interval_minutes, 3);
}
```

- [ ] **Step 4: Run tests**

```bash
cd /home/dustin/github/ThreatDeck && cargo test config::tests::auto_fetch --bin ThreatDeck
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add AutoFetchConfig with enabled and interval_minutes"
```

---

## Chunk 2: Auto-Fetch Thread Module

### Task 2: Create `src/auto_fetch.rs`

**Files:**
- Create: `src/auto_fetch.rs`

- [ ] **Step 1: Create the module with messages and thread logic**

```rust
use crate::alert::AlertEngine;
use crate::db::Db;
use crate::feed::FeedManager;
use crate::types::Feed;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum AutoFetchMessage {
    Completed {
        feeds_fetched: usize,
        alerts_created: usize,
        errors: Vec<String>,
    },
    Stopped,
}

pub struct AutoFetcher {
    pub handle: thread::JoinHandle<()>,
    pub stop_tx: mpsc::Sender<()>,
}

impl AutoFetcher {
    pub fn spawn(
        db_path: PathBuf,
        interval_minutes: u32,
        tx: mpsc::Sender<AutoFetchMessage>,
    ) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let interval = Duration::from_secs(interval_minutes as u64 * 60);

        let handle = thread::spawn(move || {
            let db = match Db::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    let _ = tx.send(AutoFetchMessage::Completed {
                        feeds_fetched: 0,
                        alerts_created: 0,
                        errors: vec![format!("Failed to open DB: {}", e)],
                    });
                    return;
                }
            };

            loop {
                match stop_rx.recv_timeout(interval) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        let _ = tx.send(AutoFetchMessage::Stopped);
                        return;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                let started = Instant::now();
                let feeds = match db.list_feeds(None) {
                    Ok(feeds) => feeds.into_iter().filter(|f| f.enabled).collect::<Vec<_>>(),
                    Err(e) => {
                        let _ = tx.send(AutoFetchMessage::Completed {
                            feeds_fetched: 0,
                            alerts_created: 0,
                            errors: vec![format!("Failed to list feeds: {}", e)],
                        });
                        continue;
                    }
                };

                let mut alerts_created = 0usize;
                let mut errors = Vec::new();

                for feed in feeds {
                    let template = feed
                        .api_template_id
                        .and_then(|id| db.get_template(id).ok().flatten());

                    match FeedManager::fetch_feed(&feed, template) {
                        Ok(result) => {
                            let keywords = db.list_keywords(true).unwrap_or_default();
                            match AlertEngine::process_feed_result(&db, &feed, &result, &keywords)
                            {
                                Ok(alerts) => {
                                    alerts_created += alerts.len();
                                }
                                Err(e) => {
                                    errors.push(format!(
                                        "Feed '{}' alert processing failed: {}",
                                        feed.name, e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            errors.push(format!("Feed '{}' fetch failed: {}", feed.name, e));
                        }
                    }
                }

                let _ = tx.send(AutoFetchMessage::Completed {
                    feeds_fetched: feeds.len(),
                    alerts_created,
                    errors,
                });
            }
        });

        AutoFetcher { handle, stop_tx }
    }

    pub fn stop(self) {
        let _ = self.stop_tx.send(());
        let _ = self.handle.join();
    }
}
```

Note: `Db::open` and `Db` need to be `Send`. Verify `Db` is `Send` (it holds a `Connection` which is `Send`).

- [ ] **Step 2: Add module declaration in `src/main.rs`**

Add near the top with the other mods:
```rust
mod auto_fetch;
```

- [ ] **Step 3: Verify it compiles**

```bash
cd /home/dustin/github/ThreatDeck && cargo check
```
Expected: no errors (may need to add `use crate::db::Db` and verify imports)

- [ ] **Step 4: Commit**

```bash
git add src/auto_fetch.rs src/main.rs
git commit -m "feat(auto-fetch): add AutoFetcher thread module"
```

---

## Chunk 3: App State and Thread Lifecycle

### Task 3: Add auto-fetch fields to `App` and manage thread

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add imports**

```rust
use crate::auto_fetch::{AutoFetchMessage, AutoFetcher};
use std::sync::mpsc;
```

- [ ] **Step 2: Add fields to `App` struct**

Add after existing fields (around line 140):
```rust
pub auto_fetcher: Option<AutoFetcher>,
pub auto_fetch_rx: Option<mpsc::Receiver<AutoFetchMessage>>,
pub settings_auto_fetch_enabled: bool,
pub settings_auto_fetch_interval: u32,
```

- [ ] **Step 3: Initialize fields in `App::new`**

After `app.refresh_settings();` (around line 240):
```rust
app.settings_auto_fetch_enabled = app.config.auto_fetch.enabled;
app.settings_auto_fetch_interval = app.config.auto_fetch.interval_minutes;

if app.config.auto_fetch.enabled {
    let (tx, rx) = mpsc::channel();
    let fetcher = AutoFetcher::spawn(
        app.paths.db_file.clone(),
        app.config.auto_fetch.interval_minutes,
        tx,
    );
    app.auto_fetcher = Some(fetcher);
    app.auto_fetch_rx = Some(rx);
}
```

- [ ] **Step 4: Handle messages in `on_tick`**

Add to `on_tick` (after notification timeout check):
```rust
if let Some(rx) = &self.auto_fetch_rx {
    if let Ok(msg) = rx.try_recv() {
        match msg {
            AutoFetchMessage::Completed {
                feeds_fetched,
                alerts_created,
                errors,
            } => {
                self.refresh_dashboard();
                self.refresh_alerts();
                self.refresh_articles();
                self.refresh_indicators();
                self.refresh_feeds();
                self.refresh_logs();

                let mut msg_text = format!(
                    "Auto-fetched {} feed(s), created {} alert(s)",
                    feeds_fetched, alerts_created
                );
                let notif_type = if errors.is_empty() {
                    crate::types::NotificationType::Success
                } else {
                    msg_text.push_str(&format!(
                        " ({} error(s) — check logs)",
                        errors.len()
                    ));
                    crate::types::NotificationType::Warning
                };
                self.set_notification(msg_text, notif_type);
            }
            AutoFetchMessage::Stopped => {
                self.set_notification(
                    "Auto-fetch stopped".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
        }
    }
}
```

- [ ] **Step 5: Add helper methods for start/stop/restart**

Add after `refresh_settings`:
```rust
pub fn start_auto_fetch(&mut self) {
    if self.auto_fetcher.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let fetcher = AutoFetcher::spawn(
        self.paths.db_file.clone(),
        self.settings_auto_fetch_interval,
        tx,
    );
    self.auto_fetcher = Some(fetcher);
    self.auto_fetch_rx = Some(rx);
}

pub fn stop_auto_fetch(&mut self) {
    if let Some(fetcher) = self.auto_fetcher.take() {
        fetcher.stop();
    }
    self.auto_fetch_rx = None;
}

pub fn restart_auto_fetch(&mut self) {
    self.stop_auto_fetch();
    if self.settings_auto_fetch_enabled {
        self.start_auto_fetch();
    }
}
```

- [ ] **Step 6: Verify compilation**

```bash
cd /home/dustin/github/ThreatDeck && cargo check
```
Expected: no errors

- [ ] **Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add auto-fetch thread lifecycle and message handling"
```

---

## Chunk 4: Settings TUI Integration

### Task 4: Add auto-fetch controls to Settings General tab

**Files:**
- Modify: `src/ui/settings.rs`

- [ ] **Step 1: Add display fields to `draw_general`**

After the enrichment row (around line 135), add:
```rust
let auto_fetch_text = format!(
    "Auto fetch: {} | Fetch interval: {} min",
    if app.settings_auto_fetch_enabled {
        "enabled"
    } else {
        "disabled"
    },
    app.settings_auto_fetch_interval
);
let auto_fetch_para = Paragraph::new(auto_fetch_text).block(
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border)),
);
f.render_widget(auto_fetch_para, chunks[5]);
```

Adjust the layout constraints from 6 to 7 rows:
```rust
.constraints([
    Constraint::Length(3), // theme
    Constraint::Length(3), // retention
    Constraint::Length(3), // preview
    Constraint::Length(3), // ioc
    Constraint::Length(3), // enrichment
    Constraint::Length(3), // auto fetch
    Constraint::Length(5), // help
])
```

And shift help to `chunks[6]`.

- [ ] **Step 2: Update help text in `draw_general`**

```rust
let help = Paragraph::new("Keys: [Left/Right/Space] Theme  [-/+] Retention  [i] IOC  [j] Raw JSON  [e] Enrichment  [o] Alert-only  [f] Auto-fetch  [+/-] Interval  [s] Save")
```

- [ ] **Step 3: Handle keys in `handle_key`**

In the General tab key handler, add after the `'o'` handler:

```rust
KeyCode::Char('f') if matches!(app.settings_tab, SettingsTab::General) => {
    app.settings_auto_fetch_enabled = !app.settings_auto_fetch_enabled;
    if app.settings_auto_fetch_enabled {
        app.start_auto_fetch();
        app.set_notification(
            "Auto-fetch enabled".to_string(),
            crate::types::NotificationType::Success,
        );
    } else {
        app.stop_auto_fetch();
        app.set_notification(
            "Auto-fetch disabled".to_string(),
            crate::types::NotificationType::Info,
        );
    }
}
KeyCode::Char('+') | KeyCode::Char('=') if matches!(app.settings_tab, SettingsTab::General) => {
    if app.settings_auto_fetch_interval < 60 {
        app.settings_auto_fetch_interval += 5;
        if app.settings_auto_fetch_enabled {
            app.restart_auto_fetch();
            app.set_notification(
                format!("Auto-fetch interval: {} min", app.settings_auto_fetch_interval),
                crate::types::NotificationType::Info,
            );
        }
    }
}
KeyCode::Char('-') if matches!(app.settings_tab, SettingsTab::General) => {
    if app.settings_auto_fetch_interval > 5 {
        app.settings_auto_fetch_interval -= 5;
        if app.settings_auto_fetch_enabled {
            app.restart_auto_fetch();
            app.set_notification(
                format!("Auto-fetch interval: {} min", app.settings_auto_fetch_interval),
                crate::types::NotificationType::Info,
            );
        }
    }
}
```

- [ ] **Step 4: Persist on save**

In the `'s'` handler (around line 547), add before `save_app_config`:
```rust
app.config.auto_fetch.enabled = app.settings_auto_fetch_enabled;
app.config.auto_fetch.interval_minutes = app.settings_auto_fetch_interval;
```

- [ ] **Step 5: Update status bar text**

The status bar text for General tab should mention auto-fetch keys. Update around line 65:
```rust
"-- NORMAL -- [1-9,0] Nav  [Tab] Tabs  [Left/Right] Theme  [-/+] Retention/Interval  [f] Auto-fetch  [i/j/e/o] Toggles  [p] Preview  [s] Save  [?] Help  [q] Quit"
```

- [ ] **Step 6: Verify compilation**

```bash
cd /home/dustin/github/ThreatDeck && cargo check
```

- [ ] **Step 7: Commit**

```bash
git add src/ui/settings.rs
git commit -m "feat(settings): add auto-fetch toggle and interval controls"
```

---

## Chunk 5: Main Exit Handling

### Task 5: Graceful shutdown on app exit

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Stop auto-fetcher before cleanup**

In `run_app`, before returning `res`, add:
```rust
app.stop_auto_fetch();
```

Or in `main`, after `run_app` returns, before the disable_raw_mode call:
```rust
app.stop_auto_fetch();
```

The better place is right after `res = run_app(...)`:
```rust
let res = run_app(&mut terminal, &mut app);
app.stop_auto_fetch();
```

- [ ] **Step 2: Verify compilation**

```bash
cd /home/dustin/github/ThreatDeck && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): gracefully stop auto-fetcher on app exit"
```

---

## Chunk 6: Tests

### Task 6: Add unit tests for auto-fetch module

**Files:**
- Modify: `src/auto_fetch.rs`

- [ ] **Step 1: Add tests at bottom of `src/auto_fetch.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn auto_fetcher_sends_completed_message() {
        let db = crate::db::Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let db_path = std::env::temp_dir().join(format!(
            "threatdeck-autofetch-test-{}.db",
            std::process::id()
        ));
        db.backup_to(&db_path).unwrap();

        let (tx, rx) = mpsc::channel();
        let fetcher = AutoFetcher::spawn(db_path.clone(), 1, tx);

        let msg = rx.recv_timeout(Duration::from_secs(10)).expect("message within 10s");
        fetcher.stop();

        match msg {
            AutoFetchMessage::Completed { .. } => {}
            other => panic!("Expected Completed, got {:?}", other),
        }

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn auto_fetcher_stops_on_signal() {
        let db = crate::db::Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let db_path = std::env::temp_dir().join(format!(
            "threatdeck-autofetch-stop-{}.db",
            std::process::id()
        ));
        db.backup_to(&db_path).unwrap();

        let (tx, rx) = mpsc::channel();
        let fetcher = AutoFetcher::spawn(db_path.clone(), 60, tx);
        fetcher.stop();

        let msg = rx.recv_timeout(Duration::from_secs(2)).expect("stopped message");
        assert!(matches!(msg, AutoFetchMessage::Stopped));

        let _ = std::fs::remove_file(&db_path);
    }
}
```

Note: `Db::backup_to` may not exist. If not, use the in-memory approach with `Connection::backup` or just create a temp file-based DB for the test.

Alternative approach for tests that avoids `backup_to`:
```rust
let db_path = std::env::temp_dir().join(format!("threatdeck-autofetch-test-{}.db", std::process::id()));
let _ = std::fs::remove_file(&db_path);
let db = crate::db::Db::open(&db_path).unwrap();
db.init_schema().unwrap();
// ... create a feed ...
let (tx, rx) = mpsc::channel();
let fetcher = AutoFetcher::spawn(db_path.clone(), 1, tx);
// ...
```

- [ ] **Step 2: Add config test to `src/config.rs` if not already done**

Verify `cargo test auto_fetch` passes.

- [ ] **Step 3: Run full test suite**

```bash
cd /home/dustin/github/ThreatDeck && cargo test
```
Expected: all tests pass (49 existing + new ones)

- [ ] **Step 4: Run release build**

```bash
cd /home/dustin/github/ThreatDeck && cargo build --release
```
Expected: builds cleanly

- [ ] **Step 5: Commit**

```bash
git add src/auto_fetch.rs
git commit -m "test(auto-fetch): add unit tests for AutoFetcher thread"
```

---

## Chunk 7: Final Integration & Manual Test

### Task 7: End-to-end verification

- [ ] **Step 1: Start the app**

```bash
cd /home/dustin/github/ThreatDeck && cargo run --release
```

- [ ] **Step 2: Verify Settings shows auto-fetch**

Press `0` (Settings), `Tab` to General tab. Should see:
```
Auto fetch: enabled | Fetch interval: 15 min
```

- [ ] **Step 3: Toggle off**

Press `f`. Should see toast "Auto-fetch disabled". Setting should show `disabled`.

- [ ] **Step 4: Toggle on**

Press `f` again. Toast "Auto-fetch enabled". Setting shows `enabled`.

- [ ] **Step 5: Adjust interval**

Press `-` twice. Should show `5 min`. Toast should appear.
Press `+` once. Should show `10 min`.

- [ ] **Step 6: Save config**

Press `s`. Toast "Config saved".

- [ ] **Step 7: Verify config file**

```bash
cat ~/.config/ThreatDeck/config.toml
```
Should contain:
```toml
[auto_fetch]
enabled = true
interval_minutes = 10
```

- [ ] **Step 8: Wait for auto-fetch (or set interval to 5 min and wait)**

After interval elapses, should see toast like:
```
Auto-fetched 83 feed(s), created 0 alert(s) (3 error(s) — check logs)
```

- [ ] **Step 9: Quit gracefully**

Press `q`. App exits without hanging.

- [ ] **Step 10: Commit final changes**

```bash
git add -A
git commit -m "feat(auto-fetch): wire up background thread with configurable interval"
```

---

## Notes & Edge Cases

1. **SQLite concurrency:** The background thread opens a separate `Db` connection. SQLite handles multiple readers fine. Writes (storing items, creating alerts) happen in the background thread only during fetch, so no write contention with the TUI thread.

2. **Feed fetch is synchronous:** Each feed is fetched sequentially in the background thread. With 83 feeds, this could take 1-3 minutes. The TUI is not blocked. Future enhancement: concurrent fetches with a semaphore.

3. **Error accumulation:** If all 83 feeds error, the toast shows "83 error(s) — check logs" but doesn't flood the UI with individual messages. Check the Logs screen (`9`) for per-feed health status.

4. **Thread panic detection:** If the background thread panics, `auto_fetcher.handle.is_finished()` will be true while `auto_fetch_rx` still exists. A future enhancement could detect this in `on_tick` and show a warning.

5. **Config migration:** Existing config files without `[auto_fetch]` will get the defaults thanks to `#[serde(default)]` on `AutoFetchConfig`.
