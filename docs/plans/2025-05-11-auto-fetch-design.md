# Auto-Fetch Background Thread Design

## Goal
Wire up the existing `FeedScheduler` into a non-blocking background thread that automatically fetches enabled feeds at a user-configurable interval. Users can toggle auto-fetch on/off and adjust the interval (5–60 minutes) from the Settings TUI.

## Architecture

```
┌─────────────┐     mpsc::Sender     ┌─────────────┐
│  AutoFetcher │─────────────────────▶│   App (TUI)  │
│   (thread)   │  AutoFetchMessage    │  (main loop) │
└─────────────┘                      └─────────────┘
       │                                   │
       │ owns own DB conn                  │ reads from channel each tick
       │                                   │
       └── sleep(interval) ──▶ fetch feeds ──┘
```

The `AutoFetcher` thread opens its own SQLite connection, sleeps for the configured interval, then fetches all enabled feeds sequentially through `FeedManager` and `AlertEngine`. Results are sent back to the TUI as a summary message. The main event loop polls the channel each tick (every 250ms) and displays a toast notification.

## New Module: `src/auto_fetch.rs`

### Messages

```rust
#[derive(Debug, Clone)]
pub enum AutoFetchMessage {
    Completed {
        feeds_fetched: usize,
        alerts_created: usize,
        errors: Vec<String>,
    },
    Stopped,
}
```

### Thread Lifecycle

| Event | Action |
|-------|--------|
| App starts with `enabled=true` | Spawn thread immediately |
| User toggles to `enabled=false` in Settings | Send stop signal, set `auto_fetcher = None` |
| User toggles to `enabled=true` in Settings | Spawn new thread with current interval |
| User changes interval | Stop existing thread, spawn new thread with new interval |
| App exits (`q`) | Send stop, `join()` before shutdown |
| Config save (`s`) | Persist to `config.toml`, restart/stop thread if enabled state changed |

The thread uses `stop_rx.recv_timeout(interval)` each cycle. If a stop signal arrives, it exits cleanly.

## Config Changes

New `[auto_fetch]` section in `AppConfig`:

```toml
[auto_fetch]
enabled = true
interval_minutes = 15
```

- `enabled`: whether the background thread runs
- `interval_minutes`: 5–60, default 15

## App State Changes

New fields on `App`:
- `auto_fetcher: Option<AutoFetcher>` — holds thread handle + stop sender
- `auto_fetch_rx: mpsc::Receiver<AutoFetchMessage>` — receives from thread
- `settings_auto_fetch_enabled: bool` — mirror of config for TUI
- `settings_auto_fetch_interval: u32` — mirror of config for TUI

## Settings TUI Integration

General tab adds two new rows:

```
Auto fetch: enabled    | Fetch interval: 15 min
```

- **Space** — toggles `enabled`
- **+ / -** — adjusts interval (clamped 5–60)
- **s** — saves to `config.toml`, applies thread start/stop if enabled state changed

## Error Handling

- **Network failures** — logged to feed health log, accumulated in `errors` vec, sent in `Completed` message
- **Thread panic** — TUI detects `handle.is_finished()` unexpectedly, shows warning toast, does not auto-restart
- **Config write failure** — standard `save_app_config` error shown as toast

## Testing Plan

1. `auto_fetch_config_defaults` — default is enabled, 15 min
2. `auto_fetch_interval_clamped` — below 5 → 5, above 60 → 60
3. `auto_fetcher_sends_completed_message` — spawn with 1s interval, verify message arrives
4. `auto_fetcher_stops_on_signal` — send stop, verify thread exits within timeout
5. `app_spawns_thread_on_start_when_enabled` — integration test verifying thread spawned
6. `app_stops_thread_when_toggled_off` — toggle off, verify thread joined

## Files to Modify

- `src/auto_fetch.rs` — new module
- `src/config.rs` — add `AutoFetchConfig`
- `src/app.rs` — spawn/manage thread, handle messages, add settings fields
- `src/ui/settings.rs` — add toggle + interval controls in General tab
- `src/ui/mod.rs` — help text update (if needed)
- `src/main.rs` — wire up thread lifecycle on start/exit
- `Cargo.toml` — no new dependencies (uses only `std`)
