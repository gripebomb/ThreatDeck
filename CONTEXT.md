# ThreatDeck

Terminal-based threat intelligence platform that ingests external feeds, matches content against user-defined keywords, surfaces alerts, and enriches extracted indicators. Single context.

## Language

### Sources

**Feed**:
A configured external source polled on its own interval. Each feed has a `FeedType` (`Api`, `Rss`, `Website`, `Onion`) that determines how its body is parsed.
_Avoid_: Source, subscription, channel

**ApiTemplate**:
A reusable JSONPath extractor binding title/description/date/url/source paths for API feeds. Selected at fetch time, not embedded in the feed.
_Avoid_: Parser, feed config

**FeedItem**:
One item extracted from a `Feed` (a post, an entry, a scraped page). Persisted with a SHA-256 `content_hash` for deduplication.
_Avoid_: Entry, record, row

**FeedHealthLog**:
Append-only log of fetch outcomes per `Feed` (`Healthy`/`Warning`/`Error`/`Disabled`), used by the Logs screen and to drive `FeedStatus`.
_Avoid_: Error log, fetch history

**FeedStatus**:
Derived health label for a `Feed` — `Healthy` (0 consecutive failures), `Warning` (1–2), `Error` (≥3), or `Disabled` when toggled off.
_Avoid_: Health, state

### Reading

**Article**:
Full readable text pulled on demand from a `FeedItem.url` and rendered in the Articles screen reader. Separate from `FeedItem`, which is the cached metadata.
_Avoid_: Post, page, content

### Detection

**Keyword**:
A user-defined pattern (`Simple` text or `Regex`, with a `case_sensitive` flag) paired with a `Criticality`. When a `FeedItem`'s content matches, the engine produces an `Alert`.
_Avoid_: Rule, pattern, trigger

**Criticality**:
A four-level severity scale (`Low`, `Medium`, `High`, `Critical`) attached to a `Keyword` and inherited by the `Alert` it creates. An `Alert` may carry a `severity_override` to differ from the originating keyword.
_Avoid_: Severity, priority, risk

**Alert**:
The record produced when a `Keyword` matches a `FeedItem`. Tracks `AlertStatus` (workflow), `AlertDisposition` (analyst verdict), an owner, and a `content_hash` for dedup.
_Avoid_: Hit, match, event, finding

**AlertStatus**:
Workflow state of an `Alert`: `New` → `Acknowledged` → `Investigating` → `Escalated` → `Closed`. Reflects *what the analyst is doing*, not what they concluded.
_Avoid_: State, phase

**AlertDisposition**:
Analyst verdict on an `Alert`: `Unknown`, `ConfirmedThreat`, `FalsePositive`, `Benign`, `Duplicate`, `Informational`, `NeedsMoreContext`. Reflects *what the alert means*, not the workflow.
_Avoid_: Outcome, classification, label

**AlertTriageEvent**:
Audit row recording a change to an `Alert`'s status, disposition, owner, or notes — append-only history of triage actions.
_Avoid_: Audit log, history

**ContentHash**:
SHA-256 digest over `(feed_id, keyword_id, title, content)` used to deduplicate `Alert`s. Distinct from the `content_hash` on `FeedItem` (which dedupes items, not alerts).
_Avoid_: Hash, fingerprint

### Indicators & Enrichment

**Indicator**:
An atomic observable extracted from content: `Ipv4`, `Ipv6`, `Domain`, `Url`, `Email`, `Md5`, `Sha1`, `Sha256`, `Cve`, `MitreAttackTechnique`, `OnionDomain`, `OnionUrl`, `CryptoWallet`, `CloudAccessKey`, or `Unknown`.
_Avoid_: IOC, observable (the user-facing UI uses "Indicators" as the label, so we follow it)

**ExtractedIndicator**:
A raw `Indicator` as produced by the extractor (`sentinel-ioc`), with offsets, surrounding text, and an optional `confidence_hint`. Persisted via the `Db` after extraction.
_Avoid_: Finding, hit

**EnrichmentProvider**:
A named external reputation source (e.g. `cisa-kev`, `urlhaus`) that can answer questions about an `Indicator`. Each is enabled/disabled and has its own health.
_Avoid_: Enricher, source, lookup

**EnrichmentJob**:
A row in the outbox-style enrichment queue, linking an `Indicator` to a `Provider`. Processed asynchronously by the enrichment worker so feed ingestion stays fast.
_Avoid_: Task, work item

### Organization

**Tag**:
A user-created color-coded label that can be attached to a `Feed`, `Keyword`, or `Alert`. Used for filtering, not for routing.
_Avoid_: Label, category

**NotificationConfig**:
A configured outbound channel (`Email`, `Webhook`, `Discord`) with a `min_criticality` threshold below which `Alert`s are not sent.
_Avoid_: Sink, notifier, alert target
