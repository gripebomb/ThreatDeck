# ThreatDeck TUI — Build Plan

A terminal-based threat intelligence monitoring and alerting platform for SOCs, security researchers, and threat intel analysts.

## Reference Architecture
Based on `matthart1983/netscan` patterns:
- **ratatui 0.30 + crossterm 0.29** for TUI framework
- **rusqlite** (bundled) for local SQLite persistence
- **serde + toml** for configuration files
- **ureq** for HTTP requests (API feeds, RSS, webhooks)
- **quick-xml** for RSS/Atom XML parsing
- **clap** for CLI argument parsing
- **anyhow** for error handling
- **jsonpath_lib** for JSONPath extraction from API responses
- **regex** for pattern matching
- **sha2** for content hashing
- **lettre** for SMTP email notifications

## Stage Breakdown

### Stage 1 — SPEC & Foundation
**Goal**: Write SPEC.md, initialize repo, create database schema, core types, config system, CLI.
**Owner**: Orchestrator (main agent)
**Output**: `SPEC.md`, `project/` git repo with foundation modules

Tasks:
1. Write comprehensive SPEC.md (data models, schema, module boundaries, screen layouts, event flow)
2. Initialize Rust project with Cargo.toml
3. Create database schema and migrations
4. Define core types/enums in `types.rs`
5. Build config system (`config.rs`) for TOML-based app config
6. CLI argument parsing (`main.rs` skeleton)
7. SQLite DB layer (`db.rs`) with connection pooling and CRUD helpers

### Stage 2 — Feed Engine
**Goal**: Multi-source feed fetching with templates, scheduling, content hashing.
**Mode**: Parallel subagents after Stage 1 completes
**Agents**: 
- `Feed_Fetcher`: API, RSS, Website, Onion fetchers
- `Feed_Scheduler`: Background scheduler with intervals, health tracking

Key deliverables:
- `feed/` module with fetchers for all 4 feed types
- `template/` module with JSONPath-based API template system
- `scheduler/` module with tick-based job scheduling
- SHA256 content hash change detection
- Consecutive failure tracking
- Feed health status computation

### Stage 3 — Alert Engine
**Goal**: Keyword matching, alert generation, deduplication, content extraction.
**Mode**: Parallel with Stage 2 (depends on types/db from Stage 1)
**Agent**: `Alert_Engineer`

Key deliverables:
- `keyword/` module with regex/simple text matching
- `alert/` module with alert generation pipeline
- Deduplication via content hash (1-hour window)
- Content snippet extraction with keyword highlighting
- Smart detection (keyword creation time vs feed last check)
- Historical back-check on keyword creation

### Stage 4 — Tag & Notify
**Goal**: Tagging system and notification channels.
**Mode**: Parallel with Stage 2/3
**Agents**:
- `Tag_Developer`: Many-to-many tagging with colors, CRUD
- `Notify_Developer`: Email (SMTP), Webhook, Discord notifications

Key deliverables:
- `tag/` module with tag CRUD and many-to-many relationships
- `notify/` module with SMTP, generic webhook, Discord webhook
- Notification delivery on new alerts
- Configurable per-destination enable/disable

### Stage 5 — TUI Screens
**Goal**: All terminal UI screens with navigation, tables, forms, and real-time updates.
**Mode**: Parallel subagents after Stage 2/3/4 (depends on data models)
**Agents**:
- `UI_Dashboard`: Dashboard screen with stats, pie chart, recent alerts
- `UI_Feeds`: Feeds management screen with table, forms, health indicators
- `UI_Alerts`: Alerts screen with filtering, read/unread, bulk ops
- `UI_Keywords_Tags`: Keywords and Tags screens
- `UI_Logs_Settings`: Logs and Settings screens

### Stage 6 — Integration & Final Polish
**Goal**: Event loop, screen navigation, demo data, final testing.
**Owner**: Orchestrator
**Tasks**:
1. Wire all screens into `app.rs` state machine
2. Main event loop with keyboard input and 30s dashboard refresh / 250ms tick
3. Demo seed data script
4. Integration testing (build, clippy, basic runtime)
5. README.md with usage instructions
6. Final merge and cleanup

## File Layout
```
project/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point, CLI, event loop
│   ├── app.rs           # App state machine, screen management
│   ├── types.rs         # Core data structures and enums
│   ├── config.rs        # TOML configuration
│   ├── db.rs            # SQLite database layer
│   ├── theme.rs         # Color themes (5 built-in)
│   ├── ui/
│   │   ├── mod.rs       # UI dispatcher, shared widgets
│   │   ├── dashboard.rs # Dashboard screen
│   │   ├── feeds.rs     # Feeds screen
│   │   ├── alerts.rs    # Alerts screen
│   │   ├── keywords.rs  # Keywords screen
│   │   ├── tags.rs      # Tags screen
│   │   ├── logs.rs      # Feed health logs screen
│   │   └── settings.rs  # Settings screen
│   ├── feed/
│   │   ├── mod.rs       # Feed manager, orchestration
│   │   ├── api.rs       # API feed fetcher
│   │   ├── rss.rs       # RSS/Atom feed parser
│   │   ├── web.rs       # Website scraper
│   │   └── onion.rs     # Tor SOCKS5 proxy fetcher
│   ├── template.rs      # API template system (JSONPath)
│   ├── scheduler.rs     # Background feed scheduler
│   ├── keyword.rs       # Keyword matching engine
│   ├── alert.rs         # Alert generation engine
│   ├── tag.rs           # Tag management
│   └── notify.rs        # Notification channels
├── demo-seed.sh         # Demo data generator
└── README.md
```

## Progression Rules
- Stage 1 must complete before any other stage (SPEC is single source of truth)
- Stages 2, 3, 4 can run in parallel after Stage 1 (they depend on types/db schema but not on each other)
- Stage 5 can start after Stage 2/3/4 complete (needs data operations)
- Stage 6 runs after Stage 5 completes
- Interface contracts from SPEC.md are sacred — no unilateral changes
