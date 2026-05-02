# ThreatDeck VHS Demo Tape — App-only view
# Run with: vhs demo.tape
# Requires: vhs (https://github.com/charmbracelet/vhs), cargo, sqlite3

Output demo.gif
Require cargo
Require sqlite3

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 750
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms
Set CursorBlink false
Set Theme "catppuccin-mocha"

# ── Hidden Setup ───────────────────────────────────────────────────────────

Hide

Type "cargo build --release 2>&1 | tail -3"
Enter
Sleep 2s

Type "mkdir -p ~/.local/share/ThreatDeck && ./demo-seed.sh ~/.local/share/ThreatDeck/ThreatDeck.db"
Enter
Sleep 1.5s

Type "clear"
Enter
Sleep 200ms

Show

# ── Launch ThreatDeck (visible from here) ──────────────────────────────────

Type "./target/release/ThreatDeck"
Enter
Sleep 2s

# ── Dashboard (Screen 1) ───────────────────────────────────────────────────
Sleep 1s

# ── Feeds (Screen 2) ───────────────────────────────────────────────────────
Type "2"
Sleep 800ms

Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms

# ── Alerts (Screen 3) ──────────────────────────────────────────────────────
Type "3"
Sleep 800ms

Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms

# ── Articles (Screen 4) ────────────────────────────────────────────────────
Type "4"
Sleep 800ms

Type "j"
Sleep 200ms
Type "j"
Sleep 200ms

# ── Keywords (Screen 5) ────────────────────────────────────────────────────
Type "5"
Sleep 800ms

Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms

# ── Tags (Screen 6) ────────────────────────────────────────────────────────
Type "6"
Sleep 800ms

# ── Logs (Screen 7) ────────────────────────────────────────────────────────
Type "7"
Sleep 800ms

Type "j"
Sleep 200ms
Type "j"
Sleep 200ms

# ── Settings (Screen 8) ────────────────────────────────────────────────────
Type "8"
Sleep 800ms

Type "Tab"
Sleep 600ms

Type "Tab"
Sleep 600ms

# ── Back to Dashboard ──────────────────────────────────────────────────────
Type "1"
Sleep 1s

# ── Help Overlay ───────────────────────────────────────────────────────────
Type "?"
Sleep 1.5s

Type "?"
Sleep 500ms

# ── Quit ───────────────────────────────────────────────────────────────────
Type "q"
Sleep 500ms

# ── Cleanup (hidden) ───────────────────────────────────────────────────────

Hide

Type "rm ~/.local/share/ThreatDeck/ThreatDeck.db"
Enter
Sleep 300ms

Show
