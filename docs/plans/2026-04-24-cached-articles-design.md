# Cached Articles Feed Design

## Goal

Add a top-level Articles tab that shows the full cached feed of RSS articles across all feeds. Users can scroll the list and press Enter to read an article inside ThreatDeck.

## Chosen Approach

ThreatDeck will store normalized feed entries in a new `feed_items` table. The feed fetch pipeline will save each RSS item independently of keyword matching, so the Articles tab can show the complete feed rather than only alert-triggering items.

This approach keeps browsing fast, supports offline reading after sync, avoids repeatedly fetching remote feeds while users browse, and creates a base for later search, filtering, read state, and article retention.

## User Experience

Add a new top-level `[4] Articles` tab. The tab displays a table of cached articles newest first, with feed name, published time, title, and read state. The standard list controls move through articles. `/` filters by title, content, URL, or feed name. `Enter` opens the selected article.

The article reader view shows:

- Title
- Feed name
- Published date
- URL
- Cleaned summary/content as wrapped terminal text

`Esc` returns to the article list. Article text should be readable plain text in the first implementation; richer rendering can come later.

## Data Model

Create a `feed_items` table with:

- `id`
- `feed_id`
- `title`
- `url`
- `author`
- `summary`
- `content`
- `published_at`
- `fetched_at`
- `content_hash`
- `read`
- `metadata_json`

Add indexes for feed lookup, newest-first browsing, read state, and duplicate detection. Use a unique content hash so repeated fetches do not duplicate articles.

## Data Flow

When a feed is fetched, normalize each remote item into a local `FeedItem`. Save it with `INSERT OR IGNORE` or equivalent duplicate-safe logic. Keyword matching and alert creation continue to work separately.

The Articles tab reads from `feed_items`, joined to `feeds` for display. Opening an article marks it read or exposes a simple read toggle.

## Rendering

Article rendering will strip HTML tags, decode basic HTML entities, normalize whitespace, and wrap text to the terminal width. This keeps the reader reliable in a terminal without embedding browser behavior.

## Error Handling

Missing item dates fall back to fetch time. Missing titles fall back to the URL or "Untitled article". Duplicate articles are skipped quietly. Feed fetch failures continue to use the existing feed health behavior.

## Testing

Tests should cover schema creation, duplicate-safe item inserts, newest-first article listing, filtering, and HTML cleanup.
