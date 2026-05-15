# Feed Fetch Diagnostics MVP Design

Date: 2026-05-15

## Goal

Improve ThreatDeck feed observability so users can tell why a feed fetch failed, where it failed, and what to try next. The first implementation target is a focused MVP that keeps the existing `ureq` fetch stack and improves current health logs, feed details, and CLI diagnostics.

The MVP should answer operator questions like:

- Did the feed fail before reaching the server, during the HTTP request, while reading the body, while parsing RSS/JSON, or while storing results?
- Was the failure likely TLS, DNS, timeout, connection refused, HTTP status, body read, RSS/XML parse, JSON parse, or database related?
- What was the latest attempt, and what happened over the last few attempts?
- Can I run a single feed diagnostic without launching the TUI?

## Non-Goals

This MVP will not replace `ureq`, add a full `tracing` log pipeline, write JSON log files, disable TLS validation, support per-feed invalid certificate bypasses, or add custom CA bundle support. Those may be useful later, but they are intentionally outside this first pass.

## Current Context

ThreatDeck currently stores feed health through `feeds.last_error`, `feeds.consecutive_failures`, `feeds.last_fetch_at`, and `feed_health_logs`. The Feeds screen already has a compact list and an `Enter` detail modal. Manual fetch and auto-fetch both run the same conceptual flow, but they duplicate fetch, health update, and error handling behavior.

Fetchers live in `src/feed/` and use `ureq`:

- `rss.rs` fetches a URL, reads the body, and parses RSS.
- `api.rs` fetches a URL, reads the body, and parses JSON.
- `web.rs` fetches a URL, reads the body, and extracts page text.
- `onion.rs` fetches through a configured proxy and reads the body.

Failures currently collapse into `anyhow` strings, which is useful for raw detail but not enough for structured diagnostics.

## Architecture

Add a diagnostics layer beside the existing fetchers rather than replacing them. The new module should be `src/feed/diagnostics.rs`.

Core types:

```rust
pub enum FetchFailurePhase {
    UrlValidation,
    Dns,
    TcpConnect,
    TlsHandshake,
    HttpRequest,
    HttpStatus,
    Redirect,
    BodyDownload,
    ContentDecode,
    FeedParse,
    DatabaseWrite,
    Unknown,
}

pub enum FetchFailureKind {
    InvalidUrl,
    DnsResolutionFailed,
    ConnectionRefused,
    ConnectionTimeout,
    TlsCertificateInvalid,
    TlsHandshakeFailed,
    HttpStatusClientError,
    HttpStatusServerError,
    TooManyRedirects,
    BodyReadFailed,
    InvalidXml,
    InvalidJson,
    InvalidRss,
    DatabaseError,
    Unknown,
}

pub struct FetchDiagnostic {
    pub phase: FetchFailurePhase,
    pub kind: FetchFailureKind,
    pub summary: String,
    pub detail: Option<String>,
    pub http_status: Option<u16>,
    pub url: String,
    pub final_url: Option<String>,
    pub elapsed_ms: u128,
}

pub struct FetchAttempt {
    pub feed_id: Option<i64>,
    pub success: bool,
    pub url: String,
    pub final_url: Option<String>,
    pub http_status: Option<u16>,
    pub elapsed_ms: u128,
    pub diagnostic: Option<FetchDiagnostic>,
    pub items_seen: Option<usize>,
    pub items_new: Option<usize>,
}
```

Classification should use `ureq` information where it exists:

- `ureq::Error::Status` maps to `HttpStatus`.
- `ureq::Error::Transport` maps to network, TLS, DNS, redirect, or timeout categories using available error context and conservative string matching.
- RSS/XML parse errors map to `FeedParse/InvalidRss` or `FeedParse/InvalidXml`.
- JSON parse errors map to `FeedParse/InvalidJson`.
- Body read failures map to `BodyDownload/BodyReadFailed`.

Avoid one large brittle classifier when local classification is clearer. Fetchers should classify errors at the boundary where context is known: request, body read, parse, or proxy setup.

## Data Model

Add a dedicated fetch attempt table:

```sql
CREATE TABLE IF NOT EXISTS feed_fetch_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL,
    attempted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success INTEGER NOT NULL,
    url TEXT NOT NULL,
    final_url TEXT,
    http_status INTEGER,
    elapsed_ms INTEGER NOT NULL,
    failure_phase TEXT,
    failure_kind TEXT,
    error_summary TEXT,
    error_detail TEXT,
    items_seen INTEGER,
    items_new INTEGER,
    FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_feed_fetch_attempts_feed
    ON feed_fetch_attempts(feed_id, attempted_at DESC);
```

Keep existing feed summary fields and add only fields needed for fast rendering:

```sql
ALTER TABLE feeds ADD COLUMN last_fetch_success_at TIMESTAMP;
ALTER TABLE feeds ADD COLUMN last_fetch_failed_at TIMESTAMP;
ALTER TABLE feeds ADD COLUMN last_failure_phase TEXT;
ALTER TABLE feeds ADD COLUMN last_failure_kind TEXT;
ALTER TABLE feeds ADD COLUMN last_http_status INTEGER;
```

`feeds.last_error` remains the short human-readable summary. Full details live in `feed_fetch_attempts.error_detail`.

Add DB methods:

- `record_feed_fetch_attempt(feed_id, &FetchAttempt)`
- `list_feed_fetch_attempts(feed_id, limit)`
- a shared helper that records an attempt and updates feed summary/health fields together

On success, reset `consecutive_failures`, clear `last_error`, clear failure phase/kind/status, update `last_fetch_at`, `last_fetch_success_at`, and content hash.

On failure, increment `consecutive_failures`, set `last_error` to the diagnostic summary, set failure phase/kind/status, update `last_fetch_at` and `last_fetch_failed_at`.

## Fetch Flow

Create a shared orchestration helper so manual fetch and auto-fetch use the same behavior.

Suggested shape:

```rust
pub struct FeedFetchOutcome {
    pub result: Option<FeedResult>,
    pub attempt: FetchAttempt,
}

pub fn run_feed_fetch_attempt(
    feed: &Feed,
    template: Option<ApiTemplate>,
) -> FeedFetchOutcome
```

Flow:

1. Start an elapsed timer.
2. Validate obvious invalid URLs before sending the request.
3. Call the appropriate existing fetcher.
4. On success, return the `FeedResult` plus a successful `FetchAttempt`.
5. On failure, return a failed `FetchAttempt` with a classified `FetchDiagnostic`.
6. The caller records the attempt and then processes alerts/items only when the fetch succeeded.

Manual fetch and auto-fetch should both call this shared helper. This keeps status updates, health logs, attempt history, and user-facing summaries consistent.

## TUI Design

Keep the Feeds table compact, but add the last short error summary if space allows:

```text
Status   Name          Type  Last Fetch        Fail  Last Error
Warning  CISA Alerts   RSS   2026-05-15 09:12  2     TLS certificate validation failed
```

Update the existing `Enter` feed detail modal into a diagnostic view. It should show:

- feed name and URL
- current status
- consecutive failures
- last success
- last failure
- latest attempt result
- phase and kind
- HTTP status if known
- elapsed time
- short error summary
- full error detail
- recent attempts

Example:

```text
Fetch Diagnostics

Feed: CISA Alerts
URL: https://example.com/feed.xml
Status: Warning
Consecutive failures: 2
Last success: 2026-05-15 08:45:00
Last failure: 2026-05-15 09:12:00

Last attempt:
  Result: Failed
  Phase: TLS handshake
  Kind: TLS certificate invalid
  HTTP status: n/a
  Elapsed: 824ms

Error:
  TLS certificate validation failed

Detail:
  invalid peer certificate: UnknownIssuer

Recent attempts:
  09:12 failed  TLS certificate validation failed
  09:00 failed  TLS certificate validation failed
  08:45 ok      18 items, 3 new
```

The Logs screen may continue to use `feed_health_logs` in the MVP. If convenient, health log messages should use the same short diagnostic summary.

## CLI Design

Add two diagnostic CLI paths:

```bash
ThreatDeck --debug-feed-id 12
ThreatDeck --check-feed https://example.com/feed.xml
```

`--debug-feed-id`:

- loads the feed from SQLite
- uses its saved feed type, template, auth headers, and proxy settings
- performs one fetch attempt
- records the attempt
- updates feed summary and health fields
- prints a diagnostic report

`--check-feed`:

- checks a raw URL outside the saved feed list
- does not write to the database
- defaults to RSS-style fetch/parse for the MVP
- prints a diagnostic report

Report shape:

```text
ThreatDeck Feed Diagnostic

Feed: CISA Alerts
URL: https://example.com/feed.xml
Result: failed
Phase: TLS handshake
Kind: TLS certificate invalid
HTTP status: n/a
Elapsed: 824ms

Error:
TLS certificate validation failed

Detail:
invalid peer certificate: UnknownIssuer
```

## Error Handling

Diagnostics should be useful even when classification is imperfect. Prefer conservative summaries over overclaiming. If a failure cannot be confidently identified, classify it as `Unknown` and preserve the full error detail.

Suggested user-facing summaries:

- `TLS certificate validation failed`
- `DNS resolution failed`
- `Connection timed out while fetching feed`
- `Connection refused by remote host`
- `Feed returned HTTP 403`
- `Feed response body could not be read`
- `Feed response could not be parsed as RSS`
- `API response could not be parsed as JSON`
- `Feed result could not be stored`

Do not disable TLS verification as part of diagnostics.

## Testing

Add focused tests for:

- inserting success and failure attempts and listing newest first
- updating feed summary fields after success and failure
- classifier behavior for representative TLS, DNS, timeout, refused, HTTP 404/500, RSS parse, and JSON parse failures
- CLI diagnostic report formatting
- manual fetch and auto-fetch continuing to update feed health through the shared helper

Where live network tests would be flaky, use classifier/unit tests and direct DB tests instead.

## Acceptance Criteria

- Failed fetches store structured phase/kind/status/summary/detail information.
- Successful fetches store successful attempts with elapsed time and item counts.
- The Feeds detail modal shows latest attempt diagnostics and recent attempts.
- Manual fetch and auto-fetch both record attempts consistently.
- `ThreatDeck --debug-feed-id <id>` runs and records a diagnostic attempt for a saved feed.
- `ThreatDeck --check-feed <url>` prints a diagnostic report without storing anything.
- Existing feed health behavior still works: consecutive failures increase on failure and reset on success.
- Existing tests pass, and new DB/classifier/formatting tests cover the MVP behavior.
