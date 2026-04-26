# ThreatDeck

<p align="center">
  <b>Terminal-based threat intelligence monitoring and alerting platform</b><br>
  for SOCs, security researchers, and threat intelligence analysts
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg?style=flat-square" alt="Rust 1.75+">
  <img src="https://img.shields.io/badge/platform-linux%20%7C%20macOS-lightgrey.svg?style=flat-square" alt="Platform">
</p>

---

## Dashboard Preview

<img width="1077" height="838" alt="image" src="https://github.com/user-attachments/assets/3702b588-3c64-42db-a20a-9d8643f74204" />

## Feeds Preview

<img width="1076" height="832" alt="image" src="https://github.com/user-attachments/assets/2408a487-780d-4a15-88bd-5514efc6a4bc" />

## Alerts Preview

<img width="1076" height="840" alt="image" src="https://github.com/user-attachments/assets/07564c80-b483-4f36-a74d-618a7d85f594" />

## Articles Preview

<img width="1078" height="839" alt="image" src="https://github.com/user-attachments/assets/f0e78eab-8e1f-4f01-9b5b-876a3be0d5f9" />

## Keywords Preview

<img width="1071" height="829" alt="image" src="https://github.com/user-attachments/assets/c8763b62-c846-456b-92a8-0394ff9a4cff" />

## Tags Preview

<img width="1079" height="837" alt="image" src="https://github.com/user-attachments/assets/fb372f6b-7faf-4ba6-8756-6232b365a3e1" />

## Features

- **Multi-source Feed Management** — Aggregate threat intelligence from APIs, RSS/Atom feeds, website scraping, and `.onion` sites via Tor
- **JSONPath API Templates** — Extract structured data from JSON APIs using configurable JSONPath expressions for title, description, date, URL, and source fields
- **Keyword Matching** — Simple text or regex-based keyword matching with 4 criticality levels (Low, Medium, High, Critical)
- **Alert Generation** — Automatic alert creation with deduplication (content hashing), snippet extraction, and metadata preservation
- **Cached Article Feed** — Browse every cached feed item across RSS/API sources and read cleaned article text directly in the terminal
- **Tagging System** — Organize feeds, keywords, and alerts with color-coded custom tags
- **Notifications** — Send alerts via Email (SMTP), Webhook, or Discord with per-channel minimum criticality thresholds
- **Dashboard Overview** — At-a-glance statistics, criticality distribution, recent alerts, and 7-day trend visualization
- **Feed Health Monitoring** — Track consecutive failures, health status (Healthy/Warning/Error/Disabled), and detailed health logs
- **Settings Management** — Alert retention policies, theme selection, notification channel configuration
- **5 Built-in Themes** — dark, light, solarized, dracula, monokai

## Installation

### Using Cargo

Requires Rust 1.75 or later.

```bash
cargo install ThreatDeck
```

The binary will be installed as `ThreatDeck` in your Cargo bin directory (usually `~/.cargo/bin/`).

### Build from Source

```bash
git clone https://github.com/gripebomb/ThreatDeck.git
cd ThreatDeck

# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Binary location
target/release/ThreatDeck
```

## Quick Start

### First Run

On first launch, `ThreatDeck` automatically creates its configuration and data directories:

```bash
ThreatDeck
```

Default paths:

- **Config**: `~/.config/ThreatDeck/config.toml`
- **Database**: `~/.local/share/ThreatDeck/ThreatDeck.db`

To view the exact paths on your system:

```bash
ThreatDeck --config-paths
```

### Adding Feeds

1. Launch the application: `ThreatDeck`
2. Press `2` to navigate to the **Feeds** screen
3. Press `a` to add a new feed
4. Fill in the feed details (Tab to cycle fields, Enter to save):
   - **Name**: Descriptive name for the feed
   - **URL**: Feed endpoint URL
   - **Type**: API, RSS, Website, or Onion
   - **Interval**: Polling interval in seconds (minimum 60)
   - For API feeds: select an API template and optionally provide an API key
   - For Onion feeds: configure Tor proxy (e.g., `socks5h://127.0.0.1:9050`)

### Creating Keywords

1. Press `4` to navigate to the **Keywords** screen
2. Press `a` to add a keyword
3. Configure:
   - **Pattern**: Text or regex pattern to match
   - **Type**: Simple text or Regex
   - **Case Sensitive**: Whether matching is case-sensitive
   - **Criticality**: Low, Medium, High, or Critical
4. Press `t` to test a pattern against sample input before saving

### Viewing Alerts

1. Press `3` to navigate to the **Alerts** screen
2. Browse alerts with `j`/`k` or arrow keys
3. Press `r` to toggle read/unread status
4. Press `R` to mark all alerts as read
5. Press `d` to delete an alert, `D` for bulk delete mode

## Configuration

The configuration file is stored at `~/.config/ThreatDeck/config.toml`:

```toml
theme = "dark"                    # dark, light, solarized, dracula, monokai
alert_retention_days = 30         # Auto-delete alerts older than N days
dashboard_refresh_secs = 30       # Dashboard data refresh interval
tick_rate_ms = 250                # UI tick rate (lower = more responsive)
max_health_log_entries = 100      # Maximum feed health log entries to retain
```

### Theme Settings

Change themes dynamically via the Settings screen (`7`) or edit `config.toml` directly:

| Theme     | Description                              |
|-----------|------------------------------------------|
| `dark`    | Default dark theme with muted palette    |
| `light`   | Clean light theme for bright terminals   |
| `solarized` | Ethan Schoonover's Solarized Dark      |
| `dracula` | Popular Dracula color scheme             |
| `monokai` | Classic Monokai syntax highlighting theme |

## Feed Types

### API (with JSONPath Templates)

For JSON REST APIs, use JSONPath expressions to extract fields. Two templates are built in:

| Template      | Title Path        | Description Path | Date Path       | URL Path   |
|---------------|-------------------|------------------|-----------------|------------|
| Ransomfeed.it | `$.post_title`    | `$.description`  | `$.discovered`  | `$.source` |
| RansomLook    | `$.name`          | `$.description`  | `$.published`   | `$.url`    |

Create custom templates via SQL or API. The `pagination_config` field supports offset/limit and page-based pagination strategies.

**Example API feed configuration:**

- URL: `https://api.ransomfeed.it/v1/posts`
- Type: `API`
- Template: `Ransomfeed.it`
- Interval: `300` (5 minutes)

### RSS / Atom

Standard RSS 2.0 and Atom 1.0 feeds are automatically parsed. Simply provide the feed URL and select type `RSS`.

**Example RSS feeds:**

- `https://feeds.securityweek.com/securityweek`
- `https://www.bleepingcomputer.com/feed/`

### Website Scraping

For HTML pages without structured feeds, the scraper extracts text content from the page body for keyword matching. Provide the target URL and select type `Website`.

**Example:**

- URL: `https://www.cisa.gov/news-events/cybersecurity-advisories`
- Type: `Website`
- Interval: `600` (10 minutes)

### Onion / Tor

Access `.onion` threat intelligence sources via a Tor SOCKS5 proxy. Configure the Tor proxy address (default: `socks5h://127.0.0.1:9050`) and provide an `.onion` URL.

**Requirements:** Tor must be running locally with a SOCKS5 listener.

**Example:**

- URL: `http://ransomxifxwc5ste.onion/posts`
- Type: `Onion`
- Tor Proxy: `socks5h://127.0.0.1:9050`

## Keyboard Shortcuts

### Global Keys

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `1`            | Dashboard screen                            |
| `2`            | Feeds screen                                |
| `3`            | Alerts screen                               |
| `4`            | Articles screen                             |
| `5`            | Keywords screen                             |
| `6`            | Tags screen                                 |
| `7`            | Logs screen                                 |
| `8`            | Settings screen                             |
| `q`            | Quit application                            |
| `?` / `F1`     | Toggle help overlay                         |
| `Esc`          | Cancel current action / Go back             |

### Dashboard

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `r`            | Refresh dashboard data                      |
| `1-8`          | Navigate to other screens                   |

### Feeds

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `↓`      | Move selection down                         |
| `k` / `↑`      | Move selection up                           |
| `a` / `n`      | Add new feed                                |
| `e`            | Edit selected feed                          |
| `d`            | Delete selected feed (with confirmation)    |
| `m`            | Trigger manual fetch                        |
| `t`            | Assign tags to feed                         |
| `Enter`        | View feed details                           |
| `Space`        | Toggle feed enabled/disabled                |
| `/`            | Filter feeds                                |
| `s`            | Cycle sort order                            |

### Alerts

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `↓`      | Move selection down                         |
| `k` / `↑`      | Move selection up                           |
| `r`            | Toggle read/unread status                   |
| `R`            | Mark all alerts as read                     |
| `d`            | Delete selected alert                       |
| `D`            | Enter bulk delete mode                      |
| `t`            | Assign tags to alert                        |
| `/`            | Filter alerts                               |

**Bulk Mode (after pressing `D`):**

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `Space`        | Toggle selection of current alert           |
| `a`            | Select all alerts                           |
| `d`            | Delete selected alerts                      |
| `Esc`          | Exit bulk mode                              |

### Articles

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `↓`      | Move selection down                         |
| `k` / `↑`      | Move selection up                           |
| `Enter`        | Fetch full text if needed, then open reader |
| `r`            | Toggle read/unread status                   |
| `u`            | Toggle unread-only filter                   |
| `/`            | Filter cached articles                      |
| `Esc`          | Close article reader                        |

### Keywords

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `↓`      | Move selection down                         |
| `k` / `↑`      | Move selection up                           |
| `a` / `n`      | Add new keyword                             |
| `e`            | Edit selected keyword                       |
| `d`            | Delete selected keyword                     |
| `t`            | Test pattern against sample input           |
| `Enter`        | Toggle keyword enabled/disabled             |

### Tags

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `↓`      | Move selection down                         |
| `k` / `↑`      | Move selection up                           |
| `a` / `n`      | Add new tag                                 |
| `e`            | Edit selected tag                           |
| `d`            | Delete selected tag                         |

### Logs

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `↓`      | Move selection down                         |
| `k` / `↑`      | Move selection up                           |

### Settings

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `Tab`          | Switch between General/Notifications tabs   |

### Forms (Add/Edit)

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `Tab`          | Next field                                  |
| `Shift+Tab`    | Previous field                              |
| `Enter`        | Save                                        |
| `Esc`          | Cancel and close form                       |

## Architecture Overview

```
ThreatDeck/
├── src/
│   ├── main.rs           # CLI args, terminal setup, main event loop
│   ├── app.rs            # Application state, screen navigation, key routing
│   ├── config.rs         # Config file (TOML) loading and saving
│   ├── db.rs             # SQLite database operations (rusqlite)
│   ├── types.rs          # Core data structures and enums
│   ├── theme.rs          # Color theme definitions
│   ├── schema.sql        # Database schema with default data
│   ├── scheduler.rs      # Background feed polling scheduler
│   ├── notify.rs         # Notification channels (Email, Webhook, Discord)
│   ├── alert.rs          # Alert processing and deduplication logic
│   ├── keyword.rs        # Keyword matching engine (text + regex)
│   ├── tag.rs            # Tag management
│   ├── template.rs       # API template management
│   ├── ai.rs             # AI/ML analysis integration placeholder
│   ├── feed/             # Feed processing modules
│   │   ├── mod.rs        # Feed dispatcher and common types
│   │   ├── api.rs        # JSON API feed fetcher with JSONPath
│   │   ├── rss.rs        # RSS/Atom feed parser
│   │   ├── web.rs        # Website HTML scraper
│   │   ├── onion.rs      # Tor/onion site fetcher
│   │   └── utils.rs      # Feed utility functions
│   └── ui/               # TUI rendering modules
│       ├── mod.rs        # Main draw dispatcher, help, notifications
│       ├── dashboard.rs  # Stats, pie chart, recent alerts, trend
│       ├── feeds.rs      # Feed list, add/edit form
│       ├── alerts.rs     # Alert list, bulk operations
│       ├── keywords.rs   # Keyword list, test mode
│       ├── tags.rs       # Tag management screen
│       ├── logs.rs       # Feed health log viewer
│       ├── settings.rs   # General settings and notifications
│       └── utils.rs      # UI helper functions
├── Cargo.toml
└── README.md
```

### Key Design Decisions

- **SQLite (bundled)**: Self-contained database with no external dependencies; rusqlite with bundled feature ensures consistent builds across platforms
- **ratatui + crossterm**: Cross-platform terminal UI framework with async event handling
- **JSONPath for APIs**: Declarative data extraction without custom parsers per feed
- **Content Hashing**: SHA-256-based deduplication prevents duplicate alerts from the same content
- **Modular Feed Engine**: Each feed type implements a common interface, making it easy to add new sources

## Development

### Prerequisites

- Rust 1.75+ with Cargo
- sqlite3 CLI (for `demo-seed.sh`)

### Building

```bash
# Debug build
cargo build

# Release build with optimizations
cargo build --release

# Run with cargo
cargo run

# Run and print config paths
cargo run -- --config-paths
```

### Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

### Code Quality

```bash
# Format check
cargo fmt --check

# Apply formatting
cargo fmt

# Run Clippy lints
cargo clippy -- -D warnings

# Run Clippy with all features
cargo clippy --all-targets --all-features -- -D warnings
```

### Seeding Demo Data

A helper script is provided to populate the database with realistic demo data:

```bash
# Uses default database path
./demo-seed.sh

# Or specify a custom database path
./demo-seed.sh /path/to/custom.db
```

This creates 6 feeds, 8 keywords, 15 alerts, and tag assignments for testing and demonstration.

### Database Schema

The application uses the following SQLite schema (see `src/schema.sql`):

- **feeds** — Feed sources with health tracking
- **api_templates** — JSONPath extraction templates
- **keywords** — Matching patterns with criticality levels
- **alerts** — Generated alerts with deduplication hashes
- **tags** — Color-coded labels
- **feed_tags / keyword_tags / alert_tags** — Many-to-many tag assignments
- **notifications** — Notification channel configurations
- **feed_health_logs** — Per-feed health status history

## License

This project is licensed under the [MIT License](LICENSE).

---

<p align="center">
  Built with <a href="https://github.com/ratatui/ratatui">ratatui</a> and Rust.
</p>
