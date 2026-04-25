#!/usr/bin/env bash
#
# demo-seed.sh — Populate ThreatDeck database with demo data
#
# Usage:
#   ./demo-seed.sh                    # Use default database path
#   ./demo-seed.sh /path/to/db.sqlite # Use custom database path
#

set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────

DEFAULT_DB="${HOME}/.local/share/ThreatDeck/ThreatDeck.db"
DB_PATH="${1:-$DEFAULT_DB}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA_FILE="${SCRIPT_DIR}/src/schema.sql"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# ── Helper Functions ─────────────────────────────────────────────────────────

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

ok() {
    echo -e "${GREEN}[OK]${NC}   $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[FAIL]${NC} $1" >&2
}

# ── Pre-flight Checks ────────────────────────────────────────────────────────

info "ThreatDeck Demo Data Seeder"
info "=============================="

# Check sqlite3 is available
if ! command -v sqlite3 &>/dev/null; then
    error "sqlite3 CLI is not installed. Please install it first."
    echo ""
    echo "  Ubuntu/Debian:  sudo apt-get install sqlite3"
    echo "  macOS:          brew install sqlite"
    echo "  Fedora:         sudo dnf install sqlite"
    exit 1
fi

ok "sqlite3 found: $(sqlite3 --version | head -n1 | cut -d' ' -f1-2)"

# Determine database path
info "Database path: ${DB_PATH}"

# Ensure directory exists
DB_DIR="$(dirname "${DB_PATH}")"
if [[ ! -d "${DB_DIR}" ]]; then
    info "Creating data directory: ${DB_DIR}"
    mkdir -p "${DB_DIR}"
fi

# Initialize schema if database doesn't exist or is empty
if [[ ! -f "${DB_PATH}" ]] || [[ "$(sqlite3 "${DB_PATH}" "SELECT count(*) FROM sqlite_master WHERE type='table';" 2>/dev/null || echo 0)" == "0" ]]; then
    info "Initializing database schema..."
    if [[ -f "${SCHEMA_FILE}" ]]; then
        sqlite3 "${DB_PATH}" < "${SCHEMA_FILE}"
        ok "Schema loaded from ${SCHEMA_FILE}"
    else
        warn "Schema file not found at ${SCHEMA_FILE}"
        warn "Will attempt to insert data anyway (tables must exist)"
    fi
else
    info "Database already exists with schema"
fi

# ── Seed Data ────────────────────────────────────────────────────────────────

info ""
info "Inserting demo data..."
info ""

# ── Feeds ────────────────────────────────────────────────────────────────────

info "Creating 6 demo feeds..."

sqlite3 "${DB_PATH}" << 'FEED_SQL'
-- API feeds
INSERT OR IGNORE INTO feeds (id, name, url, feed_type, enabled, interval_secs, api_template_id, api_key, custom_headers)
VALUES (1, 'Ransomfeed.it Tracker', 'https://api.ransomfeed.it/v1/posts', 'Api', 1, 300, 1, NULL, NULL);

INSERT OR IGNORE INTO feeds (id, name, url, feed_type, enabled, interval_secs, api_template_id, api_key, custom_headers)
VALUES (2, 'RansomLook Groups', 'https://api.ransomlook.io/v1/groups', 'Api', 1, 600, 2, NULL, NULL);

-- RSS feeds
INSERT OR IGNORE INTO feeds (id, name, url, feed_type, enabled, interval_secs)
VALUES (3, 'BleepingComputer', 'https://www.bleepingcomputer.com/feed/', 'Rss', 1, 300);

INSERT OR IGNORE INTO feeds (id, name, url, feed_type, enabled, interval_secs)
VALUES (4, 'SecurityWeek News', 'https://feeds.securityweek.com/securityweek', 'Rss', 1, 600);

-- Website feed
INSERT OR IGNORE INTO feeds (id, name, url, feed_type, enabled, interval_secs)
VALUES (5, 'CISA Alerts', 'https://www.cisa.gov/news-events/cybersecurity-advisories', 'Website', 1, 900);

-- Onion feed
INSERT OR IGNORE INTO feeds (id, name, url, feed_type, enabled, interval_secs, tor_proxy)
VALUES (6, 'Dark Web Monitor', 'http://ransomxifxwc5ste.onion/posts', 'Onion', 0, 1200, 'socks5h://127.0.0.1:9050');

-- Reset sequence
DELETE FROM sqlite_sequence WHERE name='feeds';
INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('feeds', 6);
FEED_SQL

ok "6 feeds created"

# ── Keywords ─────────────────────────────────────────────────────────────────

info "Creating 8 demo keywords..."

sqlite3 "${DB_PATH}" << 'KEYWORD_SQL'
INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (1, 'ransomware', 0, 0, 'Critical', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (2, 'CVE-[0-9]{4}-[0-9]+', 1, 0, 'High', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (3, 'APT[0-9]+', 1, 0, 'High', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (4, 'zero-day', 0, 0, 'Critical', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (5, 'phishing', 0, 0, 'Medium', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (6, 'malware', 0, 0, 'Medium', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (7, 'exploit', 0, 0, 'High', 1);

INSERT OR IGNORE INTO keywords (id, pattern, is_regex, case_sensitive, criticality, enabled)
VALUES (8, 'backdoor', 0, 0, 'Critical', 1);

-- Reset sequence
DELETE FROM sqlite_sequence WHERE name='keywords';
INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('keywords', 8);
KEYWORD_SQL

ok "8 keywords created"

# ── Alerts ───────────────────────────────────────────────────────────────────

info "Creating 15 demo alerts..."

sqlite3 "${DB_PATH}" << 'ALERT_SQL'
-- Feed 1 (Ransomfeed) alerts - ransomware keyword
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (1, 1, 1, 'LockBit hits healthcare provider', 'LockBit ransomware group claims attack on major healthcare provider, exfiltrating 2TB of patient data including medical records and insurance information.', 'Critical', 0, 'a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456', datetime('now', '-2 minutes'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, 'BlackCat targets energy sector', 'content_snippet', criticality, read, content_hash, detected_at)
VALUES (2, 1, 1, 'BlackCat targets energy sector', 'ALPHV/BlackCat ransomware operators have breached a European energy company, deploying encryption across Windows and Linux systems.', 'Critical', 0, 'b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456a1', datetime('now', '-15 minutes'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (3, 1, 7, 'Ransomware exploit chain disclosed', 'Security researchers disclose a new exploit chain used by ransomware groups leveraging CVE-2024-1234 for initial access.', 'High', 1, 'c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456a1b2', datetime('now', '-1 hour'));

-- Feed 2 (RansomLook) alerts - APT keyword
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (4, 2, 3, 'APT29 targets diplomatic missions', 'Cozy Bear (APT29) has been observed targeting diplomatic missions in Eastern Europe with sophisticated spear-phishing campaigns.', 'High', 0, 'd4e5f6789012345678901234567890abcdef1234567890abcdef123456a1b2c3', datetime('now', '-45 minutes'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (5, 2, 3, 'APT42 Iran-linked activity surge', 'Mandiant reports increased activity from APT42, an Iran-nexus actor targeting journalists and academics with credential harvesting.', 'High', 1, 'e5f6789012345678901234567890abcdef1234567890abcdef123456a1b2c3d4', datetime('now', '-3 hours'));

-- Feed 3 (BleepingComputer) alerts - CVE keyword
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (6, 3, 2, 'Critical CVE-2024-9876 in OpenSSL', 'A critical buffer overflow vulnerability has been discovered in OpenSSL 3.0.x allowing remote code execution under specific configurations.', 'High', 0, 'f6789012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5', datetime('now', '-30 minutes'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (7, 3, 4, 'Chrome zero-day exploited in wild', 'Google confirms active exploitation of a zero-day vulnerability (CVE-2024-8765) in Chrome. Patch immediately.', 'Critical', 0, '6789012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f6', datetime('now', '-5 minutes'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (8, 3, 5, 'Phishing campaign targets banks', 'A large-scale phishing campaign using brand impersonation of major banks has been detected, affecting users across 12 countries.', 'Medium', 1, '789012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f67', datetime('now', '-2 hours'));

-- Feed 4 (SecurityWeek) alerts - malware keyword
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (9, 4, 6, 'New InfoStealer malware family', 'Researchers identify a new information stealer malware dubbed "LummaC2" being distributed via malicious Google Ads.', 'Medium', 1, '89012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f678', datetime('now', '-4 hours'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (10, 4, 8, 'Supply chain backdoor discovered', 'A backdoor has been discovered in a popular npm package downloaded over 2 million times weekly. The malicious code exfiltrates environment variables.', 'Critical', 0, '9012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f6789', datetime('now', '-20 minutes'));

-- Feed 5 (CISA Website) alerts - exploit keyword
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (11, 5, 7, 'CISA adds CVE-2024-5432 to KEV catalog', 'CISA has added CVE-2024-5432 to the Known Exploited Vulnerabilities catalog, requiring federal agencies to patch by January 30.', 'High', 0, '012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f67890', datetime('now', '-1 hour'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (12, 5, 4, 'Zero-day in enterprise VPN appliances', 'CISA warns of active exploitation of a zero-day vulnerability in widely deployed enterprise VPN appliances. No patch available yet.', 'Critical', 0, '12345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f678901', datetime('now', '-10 minutes'));

INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (13, 5, 2, 'Microsoft Patch Tuesday: 87 CVEs', 'Microsoft releases January 2024 Patch Tuesday updates addressing 87 CVEs including 6 critical remote code execution vulnerabilities.', 'Medium', 1, '2345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f6789012', datetime('now', '-6 hours'));

-- Feed 1 (Ransomfeed) - backdoor alert
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (14, 1, 8, 'Ransomware deploys persistent backdoor', 'Analysis reveals that recent ransomware deployments include a persistent backdoor mechanism ensuring re-entry even after remediation.', 'Critical', 0, '345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f67890123', datetime('now', '-8 hours'));

-- Feed 3 (BleepingComputer) - malware alert
INSERT OR IGNORE INTO alerts (id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at)
VALUES (15, 3, 6, 'TrickBot malware resurfaces', 'TrickBot malware infrastructure shows signs of reactivation with new command and control servers identified in Eastern Europe.', 'Medium', 1, '456789012345678901234567890abcdef1234567890abcdef123456a1b2c3d4e5f', datetime('now', '-12 hours'));

-- Reset sequence
DELETE FROM sqlite_sequence WHERE name='alerts';
INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('alerts', 15);
ALERT_SQL

ok "15 alerts created"

# ── Tag Assignments ──────────────────────────────────────────────────────────

info "Assigning tags to feeds and keywords..."

sqlite3 "${DB_PATH}" << 'TAG_SQL'
-- Ensure default tags exist
INSERT OR IGNORE INTO tags (id, name, color, description)
VALUES (1, 'X', '#1DA1F2', 'X (Twitter) feeds');

INSERT OR IGNORE INTO tags (id, name, color, description)
VALUES (2, 'Ransomware Gang', '#FF6B6B', 'Dark web ransomware sources');

INSERT OR IGNORE INTO tags (id, name, color, description)
VALUES (3, 'API', '#4CAF50', 'REST API feeds');

INSERT OR IGNORE INTO tags (id, name, color, description)
VALUES (4, 'News', '#FF9800', 'General security news');

INSERT OR IGNORE INTO tags (id, name, color, description)
VALUES (5, 'Government', '#9C27B0', 'Government security sources');

INSERT OR IGNORE INTO tags (id, name, color, description)
VALUES (6, 'Dark Web', '#333333', 'Tor/onion sources');

-- Reset sequence
DELETE FROM sqlite_sequence WHERE name='tags';
INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('tags', 6);

-- Feed tag assignments
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (1, 3);  -- Ransomfeed -> API
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (1, 2);  -- Ransomfeed -> Ransomware Gang
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (2, 3);  -- RansomLook -> API
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (2, 2);  -- RansomLook -> Ransomware Gang
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (3, 4);  -- BleepingComputer -> News
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (4, 4);  -- SecurityWeek -> News
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (5, 5);  -- CISA -> Government
INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (6, 6);  -- Dark Web Monitor -> Dark Web

-- Keyword tag assignments
INSERT OR IGNORE INTO keyword_tags (keyword_id, tag_id) VALUES (1, 2);  -- ransomware -> Ransomware Gang
INSERT OR IGNORE INTO keyword_tags (keyword_id, tag_id) VALUES (2, 5);  -- CVE -> Government
INSERT OR IGNORE INTO keyword_tags (keyword_id, tag_id) VALUES (4, 5);  -- zero-day -> Government
TAG_SQL

ok "Tag assignments created"

# ── Health Logs ──────────────────────────────────────────────────────────────

info "Adding feed health logs..."

sqlite3 "${DB_PATH}" << 'HEALTH_SQL'
INSERT OR IGNORE INTO feed_health_logs (feed_id, status, error_message, checked_at)
VALUES (1, 'Healthy', NULL, datetime('now', '-5 minutes'));

INSERT OR IGNORE INTO feed_health_logs (feed_id, status, error_message, checked_at)
VALUES (2, 'Healthy', NULL, datetime('now', '-10 minutes'));

INSERT OR IGNORE INTO feed_health_logs (feed_id, status, error_message, checked_at)
VALUES (3, 'Healthy', NULL, datetime('now', '-3 minutes'));

INSERT OR IGNORE INTO feed_health_logs (feed_id, status, error_message, checked_at)
VALUES (4, 'Warning', 'RSS parse warning: unexpected element', datetime('now', '-1 hour'));

INSERT OR IGNORE INTO feed_health_logs (feed_id, status, error_message, checked_at)
VALUES (5, 'Healthy', NULL, datetime('now', '-15 minutes'));

INSERT OR IGNORE INTO feed_health_logs (feed_id, status, error_message, checked_at)
VALUES (6, 'Disabled', 'Feed disabled: Tor proxy unreachable', datetime('now', '-1 day'));
HEALTH_SQL

ok "Feed health logs created"

# ── Summary ──────────────────────────────────────────────────────────────────

info ""
info "=============================="
ok  "Demo data seeding complete!"
info "=============================="
info ""
info "Summary:"
info "  • Database: ${DB_PATH}"
info "  • Feeds:    $(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM feeds;")"
info "  • Keywords: $(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM keywords;")"
info "  • Alerts:   $(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM alerts;")"
info "  • Tags:     $(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM tags;")"
info ""
ok  "Launch the app with: ThreatDeck"
info "Press keys 1-8 to navigate between screens."
