# ThreatDeck TUI Design Review

> Reviewed against the TUI Design System principles (layout, interaction, color, accessibility, anti-patterns).

---

## 1. Executive Summary

ThreatDeck uses a **screen-switching (tabbed) layout** with eight full-screen views: Dashboard, Feeds, Alerts, Articles, Keywords, Tags, Logs, and Settings. The visual foundation is solid—Ratatui-based, with a configurable theme system and consistent three-zone layouts (header / content / footer) on most screens. However, several interaction gaps, accessibility oversights, and missing polish keep the TUI from feeling like a native, keyboard-first professional tool.

**Overall grade: C+** — Functional and structured, but undiscoverable, inconsistent, and missing key affordances for power users.

---

## 2. Current Layout Architecture

```
┌─ Header (title or filter bar) ──────────────┐
│                                              │
│  Content (table, form, dashboard widgets)    │
│                                              │
├─ Footer (contextual shortcuts) ──────────────┤
```

- **Dashboard** attempts a Widget Dashboard paradigm (stats, pie distribution, recent alerts, trend chart).
- **Feeds, Alerts, Articles, Keywords, Tags, Logs, Settings** use a Header + Scrollable List paradigm.
- **Forms** are modal overlays centered with `Clear`.
- **Help, Confirm, Notifications** are global overlays rendered on top after the screen draw.

### What's Working
- Clean module separation (`ui/dashboard.rs`, `ui/feeds.rs`, etc.).
- Consistent bordered blocks and semantic color usage.
- Modal overlays use `Clear` to avoid ghosting.

### What's Broken
- **No persistent navigation** — users lose spatial context when switching screens. There is no sidebar, tab bar, or breadcrumb showing *where* you are.
- **Dashboard is the only screen with visual hierarchy**; the rest are flat tables.
- **No minimum size gate** — the app will render garbled layouts below ~80×24.

---

## 3. Interaction Model Assessment

### 3.1 Keyboard Layers (Current vs. Ideal)

| Layer | Current | Ideal | Gap |
|-------|---------|-------|-----|
| **L0 Universal** | `q`, `1-8`, `Esc`, arrows | Same + `Enter`, `Tab` | Partial |
| **L1 Vim motions** | `j/k` on some lists | `j/k`, `gg`, `G`, `Ctrl+d/u`, `/` | **Missing `gg/G`, `Ctrl+d/u`, `/` search** |
| **L2 Actions** | Single-letter per screen | Context-sensitive footer + Which-Key hints | **Footer is static, not contextual** |
| **L3 Power** | None | `:` command mode, macros | **No command palette** |

### 3.2 Specific Keybinding Issues

1. **`/` (search) is advertised but unimplemented.**
   - The Feeds footer says `[/] Filter`, but pressing `/` does nothing.
   - Alerts footer says `[/] Filter` — same problem.
   - **Fix:** Implement a universal `/` filter bar that drops into an inline search field at the top of the content pane, with live filtering and `n`/`N` to jump between matches.

2. **`q` behavior is inconsistent.**
   - On Dashboard: `[q] Quit`
   - On Feeds: `[q] Back`
   - On Alerts: `[q] Back`
   - **Fix:** Standardize. `q` should always quit the application from top-level screens. Use `Esc` or `h` for "go back / close panel."

3. **`Ctrl+C` is captured by the app to quit.**
   - While the app restores terminal state on exit, this breaks the unix convention of `Ctrl+C` sending `SIGINT`.
   - **Fix:** Remove the `Ctrl+C` handler. Rely on `q` for quit. This also frees `Ctrl+C` for copy (if mouse selection is ever supported).

4. **No `gg` / `G` for list navigation.**
   - Large alert/feed lists require holding `j` to scroll.
   - **Fix:** Add `gg` (top) and `G` (bottom) motion bindings.

5. **No `Ctrl+d` / `Ctrl+u` page scrolling.**
   - **Fix:** Add half-page scroll bindings for all list views.

6. **Forms use a simulated cursor (`_` appended to text).**
   - This is functional but primitive. It breaks on multi-byte UTF-8 characters and feels alien.
   - **Fix:** Use `ratatui`'s cursor support or a proper `Input` widget (e.g., `tui-textarea` or `tui-input`).

### 3.3 InputMode (Normal vs. Typing)

The two-mode system (Normal / Typing) is a good idea borrowed from Vim, but it has friction:

- Users must press `i` or `Enter` to enter Typing mode, then `Esc` to exit.
- Direct character input in Normal mode *does* auto-switch to Typing mode for text fields, which softens the pain.
- **Recommendation:** Keep the mode system, but add a **mode indicator** in the status bar (e.g., `-- NORMAL --` / `-- INSERT --`) so users always know which mode they're in.

---

## 4. Visual Design & Color System

### 4.1 Theme System — Strengths

- 5 built-in themes (dark, light, solarized, dracula, monokai).
- Semantic slot mapping (`primary`, `secondary`, `success`, `warning`, `error`, `muted`, `highlight`, `border`).
- Criticality colors are mapped per-theme (`criticality_color()`).

### 4.2 Theme System — Weaknesses

| Issue | Severity | Details |
|-------|----------|---------|
| **No ANSI 16-color fallback** | 🔴 High | All themes hardcode RGB values. On terminals without truecolor support, colors may render as nearest-approximation and look bad. |
| **No `NO_COLOR` support** | 🔴 High | The app never checks `$NO_COLOR`. Accessibility and CI compatibility require this. |
| **No light/dark terminal detection** | 🟡 Medium | The app defaults to "dark". It should query the terminal background via OSC 11 or use `terminal-light`. |
| **Selection highlight is risky** | 🟡 Medium | `bg(highlight).fg(bg)` can become illegible if `highlight` and `bg` clash. Use `Reverse` (SGR 7) as a guaranteed-readable fallback. |
| **No dim/italic/underline usage** | 🟡 Medium | The visual hierarchy relies almost entirely on color and bold. Metadata should use `Dim` + `fg.muted`. |

### 4.3 Recommended Color Fixes

1. **Add an ANSI-only theme** that uses `Color::Blue`, `Color::Green`, etc., and a `NO_COLOR` mode that maps all colors to `Color::Reset` / `Color::White` / `Color::Black`.
2. **Change selection style to use `Modifier::REVERSED`** in addition to or instead of the custom `bg(fg)` swap. This is the most reliable cross-terminal highlight method.
3. **Add a `bg.surface` semantic slot** and use it for panel backgrounds to create depth without relying solely on borders.

---

## 5. Accessibility Audit

| Requirement | Status | Notes |
|-------------|--------|-------|
| Never use color alone | ❌ Fail | Criticality is shown only via colored `█` blocks and text color. No icon, no prefix letter. |
| WCAG AA contrast | ⚠️ Unknown | Not tested. The light theme's `muted` (#8c8c8c) on `bg` (#fafafa) is ~2.9:1 — likely fails. |
| `NO_COLOR` support | ❌ Fail | Not implemented. |
| Monochrome usability | ❌ Fail | Remove color and the app loses criticality information entirely. |
| Keyboard-only usable | ✅ Pass | All features are keyboard-accessible. |

### 5.1 Recommended Accessibility Fixes

1. **Add criticality symbols/letters:**
   ```
   Low      → "L" or "▁"
   Medium   → "M" or "▃"
   High     → "H" or "▇"
   Critical → "C" or "█"
   ```
   Display as `█ L` so color-blind users can still distinguish severity.

2. **Add `NO_COLOR` detection** at startup:
   ```rust
   let no_color = std::env::var("NO_COLOR").is_ok();
   ```
   When set, force the "ansi" theme or disable all custom colors.

3. **Test the light theme** with a contrast checker. Adjust `muted` to at least #767676 on white (4.5:1).

---

## 6. Screen-by-Screen Findings

### 6.1 Dashboard

**Current:**
- Title bar, 4 stat cards, pie chart, recent alerts list, 7-day bar chart, status bar.

**Issues:**
- The "pie chart" is actually a text bar chart. The name is misleading.
- The 7-day trend uses `█` blocks but is labeled "Sparkline" — it's not a sparkline (no braille). Rename to "Alert Trend" or switch to actual braille sparklines (`▁▂▃▄▅▆▇█`).
- Stats cards have no trend indicator (e.g., `↑ 3` vs yesterday).
- Recent alerts list is just text — no interactivity. Pressing `Enter` on a recent alert should jump to the Alerts screen with that alert selected.
- **Missing:** A single unread-alert counter with urgency color in the title bar.

### 6.2 Feeds

**Issues:**
- No title bar (inconsistent with Alerts, Logs, Settings).
- Filter bar is static text — typing `/` doesn't activate it.
- Sorting (`s`) cycles through modes but gives **no visual feedback** of which sort is active.
- The "Tags" column can overflow and push other columns off-screen.
- `m` (manual fetch) shows a toast but doesn't actually trigger anything (stub).
- `t` (tag assignment) shows a toast but doesn't open the assignment overlay.
- **Missing:** Detail view (`feeds_detail_view` exists in App state but is never drawn or handled).

### 6.3 Alerts

**Issues:**
- No filter implementation for `/`.
- Bulk mode (`D`) works but has no visual indicator in the UI that you're in bulk mode (only the key handler changes).
- `alerts_detail_view` exists in App state but is never drawn.
- The "Read" column uses `○` (unread) and `●` (read) — this is backward. Filled circle should mean unread/new.
- **Missing:** Ability to filter by criticality from the UI (exists in state: `alerts_filter_criticality`).

### 6.4 Keywords

**Issues:**
- No title bar.
- Test mode (`t`) draws a placeholder overlay with no actual functionality.
- `Case` column shows "Aa" for both sensitive and insensitive — only color changes. Use "Aa" / "aa" or "✓ Aa" / "✗ aa".
- Tags are fetched per-row inside the draw loop (`app.db.get_keyword_tags(k.id)`). This is an N+1 query and will lag with many keywords. Pre-fetch tags in `refresh_keywords()`.

### 6.5 Tags

**Issues:**
- No title bar.
- "Usage" column is always "—". It should show how many feeds/alerts/keywords use this tag.
- `Enter` is advertised as "View items" but does nothing.
- No tag assignment flow is implemented despite UI stubs.

### 6.6 Logs

**Issues:**
- Title bar says "Feed Health Logs" but the screen key is `6` — consistent, but the title is verbose.
- Filter by feed (`f`) uses the *feeds list selection* (`app.feeds_selected`), which is on a different screen. This is confusing — users on the Logs screen don't know which feed is selected in Feeds.
- **Missing:** A feed selector popup when pressing `f` on the Logs screen.

### 6.7 Settings

**Issues:**
- General tab shows theme and retention as read-only text. Users cannot change the theme or retention days without editing the config file.
- `s` (save) saves settings, but there's no feedback that settings were modified before saving.
- Theme cycling should be `←/→` or `Space` on the theme field.
- Retention days should be editable in-place.
- Notifications tab lacks row selection, edit, or delete keyboard shortcuts.

---

## 7. Missing Features (High Impact)

### 7.1 Search / Filter
- **Priority: Critical**
- Implement `/` on Feeds, Alerts, Keywords, Tags, and Logs.
- Pattern: press `/` → footer becomes an input line → type to filter live → `Esc` clears → `n`/`N` jumps between matches.

### 7.2 Detail Views
- **Priority: High**
- `feeds_detail_view`, `alerts_detail_view` exist in state but are dead code.
- Alerts detail should show full content, metadata JSON, related feed info, and keyword match context.
- Feeds detail should show fetch history, last content hash, assigned tags, and health log for that feed.

### 7.3 Tag Assignment Overlay
- **Priority: High**
- `tags_assignment_mode` exists but is never drawn or functionally handled.
- Pattern: select a feed/alert/keyword → press `t` → overlay shows checkable tag list → `Space` toggles → `Enter` saves.

### 7.4 Command Palette (`:` mode)
- **Priority: Medium**
- A `:` command mode would power-user features: `:goto alerts`, `:filter critical`, `:delete-old 30d`, `:export alerts csv`.

### 7.5 Persistent Sidebar / Tab Bar
- **Priority: Medium**
- Add a left sidebar or top tab bar showing all screens with the current one highlighted. This fixes the spatial consistency problem.
- Example:
  ```
  ┌─[1]Dashboard─┬─ Content ──────────────┐
  │ [2]Feeds     │                        │
  │ [3]Alerts    │                        │
  │ [4]Keywords  │                        │
  │ [5]Tags      │                        │
  │ [6]Logs      │                        │
  │ [7]Settings  │                        │
  └──────────────┴────────────────────────┘
  ```

### 7.6 Notification Toast Improvements
- **Priority: Medium**
- Toasts appear at `x: width-40, y: 0` and can overlap the title bar.
- Move toasts to the **bottom-right** or **bottom-center** above the footer.
- Add an auto-dismiss timer (3-5 seconds) with a subtle countdown indicator.

### 7.7 Loading / Async States
- **Priority: Medium**
- Database refreshes happen synchronously in the main loop. With 500+ alerts, `refresh_alerts()` may cause frame drops.
- Show a **spinner** in the status bar during refreshes.
- Consider moving DB operations to a background thread and updating the UI when complete.

### 7.8 Mouse Support
- **Priority: Low**
- Add `crossterm::event::MouseEvent` handling for:
  - Clicking rows to select
  - Clicking tabs to switch screens
  - Clicking buttons in forms
- Always respect `Shift+click` to bypass mouse capture for terminal text selection.

---

## 8. Anti-Pattern Checklist

| # | Anti-Pattern | Status | Fix |
|---|--------------|--------|-----|
| 1 | Colors break on different terminals | ⚠️ Partial | Add ANSI theme + `NO_COLOR` support |
| 2 | Flickering / full redraws | ✅ Pass | Ratatui handles double-buffering |
| 3 | Undiscoverable keybindings | ❌ Fail | Add sidebar/tab bar, contextual footers, Which-Key hints |
| 4 | Broken on Windows / WSL | ⚠️ Unknown | Test on Windows Terminal; avoid Unicode beyond box-drawing |
| 5 | Unicode rendering inconsistency | ⚠️ Risk | Uses `✓`/`✗` which may not render on all terminals. Use `[x]` / `[ ]` as fallback. |
| 6 | Terminal multiplexer incompatibility | ⚠️ Unknown | Test in tmux; should work with crossterm |
| 7 | No accessibility support | ❌ Fail | Add `NO_COLOR`, monochrome mode, symbols for criticality |
| 8 | Blocking UI during operations | ⚠️ Risk | DB refreshes are sync; add spinner + async loading |
| 9 | Modal confusion | ⚠️ Partial | InputMode exists but has no visual indicator |
| 10 | Over-decorated chrome | ✅ Pass | Clean, content-focused design |

---

## 9. Recommended Implementation Roadmap

### Phase 1: Critical Fixes (Week 1)
1. Implement `/` search/filter for Feeds, Alerts, Keywords, Logs.
2. Add `NO_COLOR` support and an ANSI-16 fallback theme.
3. Fix selection highlight to use `Modifier::REVERSED`.
4. Add criticality letters (`L`, `M`, `H`, `C`) alongside color.
5. Add `gg`/`G` and `Ctrl+d`/`Ctrl+u` to all lists.

### Phase 2: Consistency & Polish (Week 2)
6. Add a persistent left sidebar or top tab bar for navigation.
7. Standardize footer text across all screens (always show `1-8`, `?`, `q`).
8. Make footers **context-sensitive** (update when in a form, bulk mode, etc.).
9. Add a mode indicator (`-- NORMAL --` / `-- INSERT --`) to the footer.
10. Implement detail views for Alerts and Feeds (`Enter` to open, `Esc` to close).

### Phase 3: Power User Features (Week 3)
11. Implement tag assignment overlay (`t` on feeds/alerts/keywords).
12. Add `:` command palette with common actions.
13. Implement keyword test mode with actual matching logic.
14. Add a minimum terminal size gate (`< 80×24` → "Please resize" message).

### Phase 4: Performance & Robustness (Week 4)
15. Move DB refresh operations to a background thread.
16. Add loading spinners for async operations.
17. Add mouse support (optional, but enhances discoverability).
18. Run compatibility tests on tmux, Windows Terminal, light themes.

---

## 10. Code-Level Recommendations

### 10.1 Reduce Form Boilerplate
Each screen re-implements `draw_text_field`, `draw_toggle_field`, `draw_cycle_field`, and form key handlers. Extract these into `ui/components.rs`:

```rust
// ui/components.rs
pub struct FormField<'a> {
    pub label: &'a str,
    pub value: &'a str,
    pub focused: bool,
    pub typing: bool,
    pub field_type: FieldType, // Text | Toggle | Cycle
}

pub fn draw_form_field(f: &mut Frame, field: FormField, area: Rect, theme: &Theme) { ... }
```

### 10.2 Fix N+1 Query in Keywords
In `ui/keywords.rs`:
```rust
// BAD: queries inside the draw loop
let tag_str = match app.db.get_keyword_tags(k.id) { ... };

// GOOD: pre-fetch in refresh_keywords()
app.keywords_list = kws;
app.keyword_tags = app.db.get_all_keyword_tags()?; // batch query
```

### 10.3 Add a Shared List Component
All list screens (Feeds, Alerts, Keywords, Tags, Logs) share the same pattern:
- Header row
- Selectable rows
- `j`/`k` navigation
- `gg`/`G`
- `/` filter

Extract a `ScrollableTable` component that takes `items`, `selected`, `columns`, and `filter`.

### 10.4 Use `ratatui::widgets::List` for Simple Lists
For Tags and Logs, `Table` is overkill. Use `List` with custom `ListItem` rendering for better performance and simpler code.

---

## 11. Positive Highlights (Keep These)

- ✅ Clean module architecture (`ui/`, `app.rs`, `theme.rs`).
- ✅ Theme system with 5 presets and criticality mapping.
- ✅ Confirmation dialogs for all destructive actions.
- ✅ Notification toast system with color-coded types.
- ✅ Vim-inspired Normal/Typing modes for forms.
- ✅ Global help overlay (`?`).
- ✅ Consistent three-zone layout on most screens.
- ✅ Auto-refresh timer for the dashboard.

---

*Review completed. The app has a strong foundation but needs polish in discoverability, accessibility, and feature completeness to feel like a production-grade TUI.*
