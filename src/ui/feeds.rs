use crate::app::{App, InputMode};
use crate::types::FeedType;
use crate::ui::list::{motion_from_key, move_selection, selected_style};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Filter bar
    let sort_name = ["id", "name", "status", "last fetch"][app.feeds_sort.min(3)];
    let filter_text = if app.filter_active {
        format!("/ {}", app.feeds_filter)
    } else if app.feeds_filter.is_empty() {
        format!("Feeds | Filter: none | Sort: {}", sort_name)
    } else {
        format!("Feeds | Filter: {} | Sort: {}", app.feeds_filter, sort_name)
    };
    let filter_para = Paragraph::new(filter_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(filter_para, chunks[0]);

    // Table
    let header = Row::new(vec![
        "Status",
        "Name",
        "Type",
        "Interval",
        "Last Fetch",
        "Fails",
        "Tags",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let mut table_state = TableState::default();
    table_state.select(Some(app.feeds_selected));

    let rows: Vec<Row> = app
        .feeds_list
        .iter()
        .map(|ft| {
            let style = Style::default().fg(app.theme.fg);
            let status_color = match ft.status {
                crate::types::FeedStatus::Healthy => app.theme.success,
                crate::types::FeedStatus::Warning => app.theme.warning,
                crate::types::FeedStatus::Error => app.theme.error,
                crate::types::FeedStatus::Disabled => app.theme.muted,
            };
            let status_str = match ft.status {
                crate::types::FeedStatus::Healthy => "● Healthy",
                crate::types::FeedStatus::Warning => "● Warning",
                crate::types::FeedStatus::Error => "● Error  ",
                crate::types::FeedStatus::Disabled => "○ Disabled",
            };
            let tag_str = ft
                .tags
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let last_fetch = ft
                .feed
                .last_fetch_at
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Never".to_string());
            Row::new(vec![
                Cell::from(status_str).style(Style::default().fg(status_color)),
                Cell::from(ft.feed.name.as_str()),
                Cell::from(format!("{:?}", ft.feed.feed_type)),
                Cell::from(format!("{}s", ft.feed.interval_secs)),
                Cell::from(last_fetch),
                Cell::from(format!("{}", ft.feed.consecutive_failures)),
                Cell::from(tag_str),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(6),
            Constraint::Min(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    )
    .highlight_style(selected_style());
    f.render_stateful_widget(table, chunks[1], &mut table_state);

    let status_text = if app.input_mode == InputMode::Typing {
        "-- INSERT -- Type to enter text | [Enter] Save | [Esc] Stop typing".to_string()
    } else if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear".to_string()
    } else {
        "-- NORMAL -- [1-8] Nav  [a] Add  [e] Edit  [d] Delete  [m] Fetch  [Space] Toggle  [Enter] Detail  [/] Filter  [s] Sort  [?] Help  [q] Quit".to_string()
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);

    // Draw form overlay if active
    if app.feeds_show_form {
        draw_form(f, app);
    }

    if app.feeds_detail_view {
        draw_detail(f, app);
    }
}

fn draw_detail(f: &mut Frame, app: &App) {
    let area = f.area();
    let detail_area = Rect {
        x: area.width / 8,
        y: area.height / 6,
        width: area.width * 3 / 4,
        height: area.height * 2 / 3,
    };
    f.render_widget(Clear, detail_area);
    let Some(feed) = app.feeds_list.get(app.feeds_selected) else {
        return;
    };
    let tags = feed
        .tags
        .iter()
        .map(|t| t.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let block = Block::default()
        .title("Feed Detail - Esc to close")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    let text = [
        format!("Name: {}", feed.feed.name),
        format!("URL: {}", feed.feed.url),
        format!("Type: {}", feed.feed.feed_type),
        format!("Enabled: {}", if feed.feed.enabled { "yes" } else { "no" }),
        format!("Interval: {}s", feed.feed.interval_secs),
        format!("Status: {}", feed.status.label()),
        format!("Failures: {}", feed.feed.consecutive_failures),
        format!(
            "Last fetch: {}",
            feed.feed
                .last_fetch_at
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Never".into())
        ),
        format!(
            "Last error: {}",
            feed.feed.last_error.as_deref().unwrap_or("none")
        ),
        format!(
            "Content hash: {}",
            feed.feed.content_hash.as_deref().unwrap_or("none")
        ),
        format!(
            "Tags: {}",
            if tags.is_empty() { "none".into() } else { tags }
        ),
    ]
    .join("\n");
    let para = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, detail_area);
}

fn draw_form(f: &mut Frame, app: &App) {
    let area = f.area();
    // Center the form popup
    let form_width = 70u16.min(area.width.saturating_sub(4)).max(50);
    let form_height = 26u16.min(area.height.saturating_sub(4));
    let form_area = Rect {
        x: (area.width.saturating_sub(form_width)) / 2,
        y: (area.height.saturating_sub(form_height)) / 2,
        width: form_width,
        height: form_height,
    };

    f.render_widget(Clear, form_area);

    let title = if app.feeds_form_edit_id.is_some() {
        "Edit Feed"
    } else {
        "Add Feed"
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.input_mode == InputMode::Typing {
            app.theme.warning
        } else {
            app.theme.primary
        }));
    f.render_widget(block.clone(), form_area);

    let inner = block.inner(form_area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // gap
            Constraint::Length(3), // name
            Constraint::Length(3), // url
            Constraint::Length(3), // interval
            Constraint::Length(3), // api_key
            Constraint::Length(3), // custom_headers
            Constraint::Length(3), // tor_proxy
            Constraint::Length(3), // enabled
            Constraint::Length(3), // feed_type
            Constraint::Min(0),    // help text
        ])
        .split(inner);

    // Field 0: name
    draw_form_field(f, app, 0, "Name *", &app.feeds_form.name, rows[1]);
    // Field 1: url
    draw_form_field(f, app, 1, "URL *", &app.feeds_form.url, rows[2]);
    // Field 2: interval
    draw_form_field(
        f,
        app,
        2,
        "Interval (sec)",
        &app.feeds_form.interval_secs.to_string(),
        rows[3],
    );
    // Field 3: api_key
    draw_form_field(f, app, 3, "API Key", &app.feeds_form.api_key, rows[4]);
    // Field 4: custom_headers
    draw_form_field(
        f,
        app,
        4,
        "Custom Headers (JSON)",
        &app.feeds_form.custom_headers,
        rows[5],
    );
    // Field 5: tor_proxy
    draw_form_field(f, app, 5, "Tor Proxy", &app.feeds_form.tor_proxy, rows[6]);
    // Field 6: enabled toggle
    let enabled_label = if app.feeds_form.enabled { "Yes" } else { "No" };
    draw_toggle_field(f, app, 6, "Enabled", enabled_label, rows[7]);
    // Field 7: feed_type cycle
    draw_cycle_field(
        f,
        app,
        7,
        "Feed Type",
        &app.feeds_form.feed_type.to_string(),
        rows[8],
    );

    // Help text
    let help_text = if app.input_mode == InputMode::Typing {
        "[Type] Enter text  [Backspace] Delete  [Enter] Submit form  [Esc] Cancel typing"
    } else {
        "[Tab] Next field  [i/Enter] Start typing  [Space] Toggle  [←→] Cycle  [Esc] Cancel form"
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(help, rows[9]);
}

/// Draw a text input field with focus highlight
fn draw_form_field(
    f: &mut Frame,
    app: &App,
    field_idx: usize,
    label: &str,
    value: &str,
    area: Rect,
) {
    let is_focused = app.form_focus == field_idx;
    let border_color = if is_focused && app.input_mode == InputMode::Typing {
        app.theme.warning
    } else if is_focused {
        app.theme.primary
    } else {
        app.theme.border
    };
    let block = Block::default()
        .title(label)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let display_value = if is_focused && app.input_mode == InputMode::Typing {
        // Show cursor indicator with the value
        format!("{}_", value)
    } else {
        value.to_string()
    };

    let para = Paragraph::new(display_value)
        .block(block)
        .style(Style::default().fg(app.theme.fg));
    f.render_widget(para, area);
}

/// Draw a toggle field with focus highlight
fn draw_toggle_field(
    f: &mut Frame,
    app: &App,
    field_idx: usize,
    label: &str,
    value: &str,
    area: Rect,
) {
    let is_focused = app.form_focus == field_idx;
    let border_color = if is_focused {
        app.theme.primary
    } else {
        app.theme.border
    };
    let block = Block::default()
        .title(format!("{} (Space to toggle)", label))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let para = Paragraph::new(value).block(block).style(
        Style::default()
            .fg(if is_focused {
                app.theme.highlight
            } else {
                app.theme.fg
            })
            .add_modifier(if is_focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    );
    f.render_widget(para, area);
}

/// Draw a cycle field with focus highlight
fn draw_cycle_field(
    f: &mut Frame,
    app: &App,
    field_idx: usize,
    label: &str,
    value: &str,
    area: Rect,
) {
    let is_focused = app.form_focus == field_idx;
    let border_color = if is_focused {
        app.theme.primary
    } else {
        app.theme.border
    };
    let block = Block::default()
        .title(format!("{} (← → to cycle)", label))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let para = Paragraph::new(value).block(block).style(
        Style::default()
            .fg(if is_focused {
                app.theme.highlight
            } else {
                app.theme.fg
            })
            .add_modifier(if is_focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    );
    f.render_widget(para, area);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.feeds_show_form {
        handle_form_key(app, key);
        return;
    }

    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.feeds_selected = move_selection(app.feeds_selected, app.feeds_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char('a') | KeyCode::Char('n') => {
            app.feeds_show_form = true;
            app.feeds_form = crate::types::FeedForm::default();
            app.feeds_form_edit_id = None;
            app.form_focus = 0;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char('e') => {
            if let Some(ft) = app.feeds_list.get(app.feeds_selected) {
                let f = &ft.feed;
                app.feeds_form = crate::types::FeedForm {
                    name: f.name.clone(),
                    url: f.url.clone(),
                    feed_type: f.feed_type,
                    interval_secs: f.interval_secs,
                    enabled: f.enabled,
                    api_template_id: f.api_template_id,
                    api_key: f.api_key.clone().unwrap_or_default(),
                    custom_headers: f.custom_headers.clone().unwrap_or_default(),
                    tor_proxy: f.tor_proxy.clone().unwrap_or_default(),
                };
                app.feeds_form_edit_id = Some(f.id);
                app.feeds_show_form = true;
                app.form_focus = 0;
                app.input_mode = InputMode::Normal;
            }
        }
        KeyCode::Char('d') => {
            if let Some(ft) = app.feeds_list.get(app.feeds_selected) {
                app.show_confirm = Some(crate::types::ConfirmDialog::DeleteFeed {
                    id: ft.feed.id,
                    name: ft.feed.name.clone(),
                });
            }
        }
        KeyCode::Char('m') => {
            app.fetch_selected_feed();
        }
        KeyCode::Char('t') => {
            if let Some(ft) = app.feeds_list.get(app.feeds_selected) {
                app.tags_assignment_mode = true;
                app.tags_assignment_target =
                    Some(crate::types::TagAssignmentTarget::Feed(ft.feed.id));
                app.refresh_tags();
            }
        }
        KeyCode::Enter => {
            app.feeds_detail_view = true;
        }
        KeyCode::Char(' ') => {
            if let Some(ft) = app.feeds_list.get(app.feeds_selected) {
                let _ = app.db.toggle_feed_enabled(ft.feed.id);
                app.refresh_feeds();
            }
        }
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = InputMode::Typing;
        }
        KeyCode::Char('s') => {
            app.feeds_sort = (app.feeds_sort + 1) % 4;
            app.refresh_feeds();
        }
        _ => {}
    }
}

/// Handle keystrokes when the feed form is open.
/// Operates in two modes:
/// - Normal: Tab moves between fields, i/Enter enters Typing mode, Space toggles, arrows cycle
/// - Typing: characters are appended to the focused field, Backspace deletes, Enter submits, Esc exits typing
fn handle_form_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_form_normal_mode(app, key),
        InputMode::Typing => handle_form_typing_mode(app, key),
    }
}

fn handle_form_normal_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Tab => app.form_focus = (app.form_focus + 1) % 8,
        KeyCode::BackTab => {
            app.form_focus = if app.form_focus == 0 {
                7
            } else {
                app.form_focus - 1
            };
        }
        KeyCode::Esc => {
            app.feeds_show_form = false;
            app.feeds_form = crate::types::FeedForm::default();
            app.feeds_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
        }
        // Enter Typing mode for text fields (0-5)
        KeyCode::Char('i') | KeyCode::Enter if app.form_focus <= 5 => {
            app.input_mode = InputMode::Typing;
        }
        // Toggle enabled field (index 6) with Space or Enter
        KeyCode::Char(' ') | KeyCode::Enter if app.form_focus == 6 => {
            app.feeds_form.enabled = !app.feeds_form.enabled;
        }
        // Cycle feed_type field (index 7) with arrows, Tab, or Enter
        KeyCode::Left | KeyCode::Right | KeyCode::Enter if app.form_focus == 7 => {
            cycle_feed_type(
                app,
                key.code == KeyCode::Right || key.code == KeyCode::Enter,
            );
        }
        // Also allow cycling with Tab on feed_type
        KeyCode::Char(' ') if app.form_focus == 7 => {
            cycle_feed_type(app, true);
        }
        // Direct character input for text fields only when not a special key
        KeyCode::Char(c) if app.form_focus <= 5 => {
            // Start typing mode and insert the character
            app.input_mode = InputMode::Typing;
            append_to_feed_field(app, c);
        }
        _ => {}
    }
}

fn handle_form_typing_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        // Submit the form
        KeyCode::Enter => submit_feed_form(app),
        // Delete last character
        KeyCode::Backspace => {
            backspace_feed_field(app);
        }
        // Append typed character to the focused field
        KeyCode::Char(c) => {
            // For numeric field (interval_secs), only accept digits
            if app.form_focus == 2 && !c.is_ascii_digit() {
                return;
            }
            append_to_feed_field(app, c);
        }
        // Tab exits typing mode and moves to next field
        KeyCode::Tab => {
            app.input_mode = InputMode::Normal;
            app.form_focus = (app.form_focus + 1) % 8;
        }
        // Esc exits typing mode (does not close the form)
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

fn append_to_feed_field(app: &mut App, c: char) {
    match app.form_focus {
        0 => app.feeds_form.name.push(c),
        1 => app.feeds_form.url.push(c),
        2 => {
            // interval_secs: only digits, build the number
            if c.is_ascii_digit() {
                let current = app.feeds_form.interval_secs;
                let digit = c.to_digit(10).unwrap_or(0) as u64;
                app.feeds_form.interval_secs = current.saturating_mul(10).saturating_add(digit);
            }
        }
        3 => app.feeds_form.api_key.push(c),
        4 => app.feeds_form.custom_headers.push(c),
        5 => app.feeds_form.tor_proxy.push(c),
        _ => {}
    }
}

fn backspace_feed_field(app: &mut App) {
    match app.form_focus {
        0 => {
            app.feeds_form.name.pop();
        }
        1 => {
            app.feeds_form.url.pop();
        }
        2 => {
            // interval_secs: divide by 10 to remove last digit
            app.feeds_form.interval_secs /= 10;
        }
        3 => {
            app.feeds_form.api_key.pop();
        }
        4 => {
            app.feeds_form.custom_headers.pop();
        }
        5 => {
            app.feeds_form.tor_proxy.pop();
        }
        _ => {}
    }
}

fn cycle_feed_type(app: &mut App, forward: bool) {
    let variants = [
        FeedType::Api,
        FeedType::Rss,
        FeedType::Website,
        FeedType::Onion,
    ];
    let current = app.feeds_form.feed_type;
    let idx = variants.iter().position(|&v| v == current).unwrap_or(0);
    let new_idx = if forward {
        (idx + 1) % variants.len()
    } else {
        if idx == 0 {
            variants.len() - 1
        } else {
            idx - 1
        }
    };
    app.feeds_form.feed_type = variants[new_idx];
}

fn submit_feed_form(app: &mut App) {
    let create = crate::db::FeedCreate {
        name: app.feeds_form.name.clone(),
        url: app.feeds_form.url.clone(),
        feed_type: app.feeds_form.feed_type,
        enabled: app.feeds_form.enabled,
        interval_secs: app.feeds_form.interval_secs.max(60),
        api_template_id: app.feeds_form.api_template_id,
        api_key: if app.feeds_form.api_key.is_empty() {
            None
        } else {
            Some(app.feeds_form.api_key.clone())
        },
        custom_headers: if app.feeds_form.custom_headers.is_empty() {
            None
        } else {
            Some(app.feeds_form.custom_headers.clone())
        },
        tor_proxy: if app.feeds_form.tor_proxy.is_empty() {
            None
        } else {
            Some(app.feeds_form.tor_proxy.clone())
        },
    };
    let res = if let Some(id) = app.feeds_form_edit_id {
        let update = crate::db::FeedUpdate {
            name: Some(app.feeds_form.name.clone()),
            url: Some(app.feeds_form.url.clone()),
            feed_type: Some(app.feeds_form.feed_type),
            enabled: Some(app.feeds_form.enabled),
            interval_secs: Some(app.feeds_form.interval_secs.max(60)),
            api_template_id: app.feeds_form.api_template_id,
            api_key: if app.feeds_form.api_key.is_empty() {
                None
            } else {
                Some(app.feeds_form.api_key.clone())
            },
            custom_headers: if app.feeds_form.custom_headers.is_empty() {
                None
            } else {
                Some(app.feeds_form.custom_headers.clone())
            },
            tor_proxy: if app.feeds_form.tor_proxy.is_empty() {
                None
            } else {
                Some(app.feeds_form.tor_proxy.clone())
            },
        };
        app.db.update_feed(id, &update)
    } else {
        app.db.create_feed(&create).map(|_| ())
    };
    match res {
        Ok(_) => {
            app.feeds_show_form = false;
            app.feeds_form = crate::types::FeedForm::default();
            app.feeds_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
            app.refresh_feeds();
            app.set_notification(
                "Feed saved".to_string(),
                crate::types::NotificationType::Success,
            );
        }
        Err(e) => app.set_notification(
            format!("Error: {}", e),
            crate::types::NotificationType::Error,
        ),
    }
}
