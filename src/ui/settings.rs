use crate::app::{App, InputMode};
use crate::types::*;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Settings")
        .style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.theme.border)),
        );
    f.render_widget(title, chunks[0]);

    let tab_titles = vec!["General", "Notifications"];
    let tabs = Tabs::new(tab_titles)
        .select(match app.settings_tab {
            SettingsTab::General => 0,
            SettingsTab::Notifications => 1,
        })
        .style(Style::default().fg(app.theme.muted))
        .highlight_style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.theme.border)),
        );
    f.render_widget(tabs, chunks[1]);

    match app.settings_tab {
        SettingsTab::General => draw_general(f, app, chunks[2]),
        SettingsTab::Notifications => draw_notifications(f, app, chunks[2]),
    }

    let status_text = if app.settings_notif_form && app.input_mode == InputMode::Typing {
        "-- INSERT -- Type to enter text | [Enter] Save | [Esc] Stop typing".to_string()
    } else {
        "-- NORMAL -- [1-8] Nav  [Tab] Tabs  [Left/Right] Theme  [-/+] Retention  [p] Preview  [x] Cleanup  [s] Save  [?] Help  [q] Quit".to_string()
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[3]);

    // Draw notification form overlay if active
    if app.settings_notif_form {
        draw_notif_form(f, app);
    }
}

fn draw_general(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(5),
        ])
        .split(area);

    let theme_names = crate::theme::theme_names().join(", ");
    let theme_text = format!(
        "Theme: {} (available: {})",
        app.settings_theme_name, theme_names
    );
    let theme_para = Paragraph::new(theme_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(theme_para, chunks[0]);

    let retention_text = format!("Alert retention: {} days", app.settings_retention_days);
    let retention_para = Paragraph::new(retention_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(retention_para, chunks[1]);

    let preview = if let Some(count) = app.settings_cleanup_preview {
        format!("Cleanup preview: {} alerts will be deleted", count)
    } else {
        "Press [p] to preview cleanup".to_string()
    };
    let preview_para = Paragraph::new(preview).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(preview_para, chunks[2]);

    let help = Paragraph::new("Keys: [Left/Right/Space] Theme  [-/+] Retention  [p] Preview cleanup  [x] Execute cleanup  [s] Save settings")
        .style(Style::default().fg(app.theme.muted))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(help, chunks[3]);
}

fn draw_notifications(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let header = Row::new(vec!["Name", "Channel", "Min Crit", "Enabled"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let rows: Vec<Row> = app
        .settings_notifications
        .iter()
        .map(|n| {
            let style = Style::default().fg(app.theme.fg);
            Row::new(vec![
                Cell::from(n.name.as_str()),
                Cell::from(format!("{:?}", n.channel)),
                Cell::from(format!("{:?}", n.min_criticality)).style(Style::default().fg(
                    crate::theme::criticality_color(app.theme, n.min_criticality),
                )),
                Cell::from(if n.enabled { "✓" } else { "✗" }).style(Style::default().fg(
                    if n.enabled {
                        app.theme.success
                    } else {
                        app.theme.error
                    },
                )),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(table, area);
}

fn draw_notif_form(f: &mut Frame, app: &App) {
    let area = f.area();
    let form_width = 70u16.min(area.width.saturating_sub(4)).max(50);
    let form_height = 24u16.min(area.height.saturating_sub(4));
    let form_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(form_width)) / 2,
        y: (area.height.saturating_sub(form_height)) / 2,
        width: form_width,
        height: form_height,
    };

    f.render_widget(Clear, form_area);

    let title = if app.settings_notif_form_edit_id.is_some() {
        "Edit Notification"
    } else {
        "Add Notification"
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
            Constraint::Length(3), // name (0)
            Constraint::Length(3), // config_json (1)
            Constraint::Length(3), // min_criticality cycle (2)
            Constraint::Length(3), // enabled toggle (3)
            Constraint::Length(3), // channel cycle (4)
            Constraint::Min(0),    // help text
        ])
        .split(inner);

    // Field 0: name (text)
    draw_text_field(
        f,
        app,
        0,
        "Name *",
        &app.settings_notif_form_data.name,
        rows[1],
    );

    // Field 1: config_json (text)
    draw_text_field(
        f,
        app,
        1,
        "Config JSON",
        &app.settings_notif_form_data.config_json,
        rows[2],
    );

    // Field 2: min_criticality cycle
    let crit_str = format!("{:?}", app.settings_notif_form_data.min_criticality);
    draw_cycle_field(f, app, 2, "Min Criticality", &crit_str, rows[3]);

    // Field 3: enabled toggle
    let enabled_label = if app.settings_notif_form_data.enabled {
        "Yes"
    } else {
        "No"
    };
    draw_toggle_field(f, app, 3, "Enabled", enabled_label, rows[4]);

    // Field 4: channel cycle
    let channel_str = format!("{:?}", app.settings_notif_form_data.channel);
    draw_cycle_field(f, app, 4, "Channel", &channel_str, rows[5]);

    // Help text
    let help_text = if app.input_mode == InputMode::Typing {
        "[Type] Enter text  [Backspace] Delete  [Enter] Submit form  [Esc] Cancel typing"
    } else {
        "[Tab] Next field  [i/Enter] Start typing  [Space] Toggle  [←→] Cycle  [Esc] Cancel form"
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(help, rows[6]);
}

/// Draw a text input field with focus highlight and cursor
fn draw_text_field(
    f: &mut Frame,
    app: &App,
    field_idx: usize,
    label: &str,
    value: &str,
    area: ratatui::layout::Rect,
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
    area: ratatui::layout::Rect,
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
    area: ratatui::layout::Rect,
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
    if app.settings_notif_form {
        handle_notif_form_key(app, key);
        return;
    }

    match key.code {
        KeyCode::Tab | KeyCode::BackTab => {
            app.settings_tab = match app.settings_tab {
                SettingsTab::General => SettingsTab::Notifications,
                SettingsTab::Notifications => SettingsTab::General,
            };
        }
        KeyCode::Right | KeyCode::Char(' ') if matches!(app.settings_tab, SettingsTab::General) => {
            cycle_theme(app, true);
        }
        KeyCode::Left if matches!(app.settings_tab, SettingsTab::General) => {
            cycle_theme(app, false);
        }
        KeyCode::Char('+') | KeyCode::Char('=')
            if matches!(app.settings_tab, SettingsTab::General) =>
        {
            app.settings_retention_days = app.settings_retention_days.saturating_add(1);
            app.settings_cleanup_preview = None;
        }
        KeyCode::Char('-') if matches!(app.settings_tab, SettingsTab::General) => {
            app.settings_retention_days = app.settings_retention_days.saturating_sub(1).max(1);
            app.settings_cleanup_preview = None;
        }
        KeyCode::Char('p') => {
            if let Some(cutoff) = chrono::Utc::now()
                .checked_sub_signed(chrono::Duration::days(app.settings_retention_days as i64))
            {
                let count = app.db.count_old_alerts(cutoff).unwrap_or(0);
                app.settings_cleanup_preview = Some(count);
            }
        }
        KeyCode::Char('x') => {
            if let Some(count) = app.settings_cleanup_preview {
                if let Some(cutoff) = chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::days(app.settings_retention_days as i64))
                {
                    app.show_confirm = Some(ConfirmDialog::DeleteOldAlerts { cutoff, count });
                }
            }
        }
        KeyCode::Char('s') => {
            app.config.theme = app.settings_theme_name.clone();
            app.config.alert_retention_days = app.settings_retention_days;
            app.theme = crate::theme::get_runtime_theme(&app.config.theme);
            let _ = crate::config::save_app_config(&app.paths.config_file, &app.config);
            app.set_notification("Settings saved".to_string(), NotificationType::Success);
        }
        KeyCode::Char('a') | KeyCode::Char('n') => {
            if matches!(app.settings_tab, SettingsTab::Notifications) {
                app.settings_notif_form = true;
                app.settings_notif_form_data = NotificationForm::default();
                app.settings_notif_form_edit_id = None;
                app.form_focus = 0;
                app.input_mode = InputMode::Normal;
            }
        }
        _ => {}
    }
}

fn cycle_theme(app: &mut App, forward: bool) {
    let names = crate::theme::theme_names();
    let idx = names
        .iter()
        .position(|name| *name == app.settings_theme_name)
        .unwrap_or(0);
    let next = if forward {
        (idx + 1) % names.len()
    } else if idx == 0 {
        names.len() - 1
    } else {
        idx - 1
    };
    app.settings_theme_name = names[next].to_string();
    app.theme = crate::theme::get_runtime_theme(&app.settings_theme_name);
}

fn handle_notif_form_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_notif_form_normal_mode(app, key),
        InputMode::Typing => handle_notif_form_typing_mode(app, key),
    }
}

fn handle_notif_form_normal_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Tab => app.form_focus = (app.form_focus + 1) % 5,
        KeyCode::BackTab => {
            app.form_focus = if app.form_focus == 0 {
                4
            } else {
                app.form_focus - 1
            };
        }
        KeyCode::Esc => {
            app.settings_notif_form = false;
            app.settings_notif_form_data = NotificationForm::default();
            app.settings_notif_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
        }
        // Enter Typing mode for text fields (0 = name, 1 = config_json)
        KeyCode::Char('i') | KeyCode::Enter if app.form_focus <= 1 => {
            app.input_mode = InputMode::Typing;
        }
        // Toggle enabled field (index 3) with Space or Enter
        KeyCode::Char(' ') | KeyCode::Enter if app.form_focus == 3 => {
            app.settings_notif_form_data.enabled = !app.settings_notif_form_data.enabled;
        }
        // Cycle min_criticality field (index 2) with arrows or Enter
        KeyCode::Left | KeyCode::Right | KeyCode::Enter if app.form_focus == 2 => {
            cycle_criticality(
                &mut app.settings_notif_form_data.min_criticality,
                key.code == KeyCode::Right || key.code == KeyCode::Enter,
            );
        }
        KeyCode::Char(' ') if app.form_focus == 2 => {
            cycle_criticality(&mut app.settings_notif_form_data.min_criticality, true);
        }
        // Cycle channel field (index 4) with arrows or Enter
        KeyCode::Left | KeyCode::Right | KeyCode::Enter if app.form_focus == 4 => {
            cycle_channel(
                &mut app.settings_notif_form_data.channel,
                key.code == KeyCode::Right || key.code == KeyCode::Enter,
            );
        }
        KeyCode::Char(' ') if app.form_focus == 4 => {
            cycle_channel(&mut app.settings_notif_form_data.channel, true);
        }
        // Direct character input for text fields
        KeyCode::Char(c) if app.form_focus <= 1 => {
            app.input_mode = InputMode::Typing;
            append_to_notif_field(app, c);
        }
        _ => {}
    }
}

fn handle_notif_form_typing_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => submit_notif_form(app),
        KeyCode::Backspace => {
            backspace_notif_field(app);
        }
        KeyCode::Char(c) => {
            append_to_notif_field(app, c);
        }
        KeyCode::Tab => {
            app.input_mode = InputMode::Normal;
            app.form_focus = (app.form_focus + 1) % 5;
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

fn append_to_notif_field(app: &mut App, c: char) {
    match app.form_focus {
        0 => app.settings_notif_form_data.name.push(c),
        1 => app.settings_notif_form_data.config_json.push(c),
        _ => {}
    }
}

fn backspace_notif_field(app: &mut App) {
    match app.form_focus {
        0 => {
            app.settings_notif_form_data.name.pop();
        }
        1 => {
            app.settings_notif_form_data.config_json.pop();
        }
        _ => {}
    }
}

fn cycle_criticality(crit: &mut Criticality, forward: bool) {
    let variants = [
        Criticality::Low,
        Criticality::Medium,
        Criticality::High,
        Criticality::Critical,
    ];
    let idx = variants.iter().position(|&v| v == *crit).unwrap_or(0);
    let new_idx = if forward {
        (idx + 1) % variants.len()
    } else {
        if idx == 0 {
            variants.len() - 1
        } else {
            idx - 1
        }
    };
    *crit = variants[new_idx];
}

fn cycle_channel(channel: &mut NotificationChannel, forward: bool) {
    let variants = [
        NotificationChannel::Email,
        NotificationChannel::Webhook,
        NotificationChannel::Discord,
    ];
    let idx = variants.iter().position(|&v| v == *channel).unwrap_or(0);
    let new_idx = if forward {
        (idx + 1) % variants.len()
    } else {
        if idx == 0 {
            variants.len() - 1
        } else {
            idx - 1
        }
    };
    *channel = variants[new_idx];
}

fn submit_notif_form(app: &mut App) {
    let create = crate::db::NotificationCreate {
        name: app.settings_notif_form_data.name.clone(),
        channel: app.settings_notif_form_data.channel,
        config_json: app.settings_notif_form_data.config_json.clone(),
        enabled: app.settings_notif_form_data.enabled,
        min_criticality: app.settings_notif_form_data.min_criticality,
    };
    let res = if let Some(id) = app.settings_notif_form_edit_id {
        let update = crate::db::NotificationUpdate {
            name: Some(app.settings_notif_form_data.name.clone()),
            channel: Some(app.settings_notif_form_data.channel),
            config_json: Some(app.settings_notif_form_data.config_json.clone()),
            enabled: Some(app.settings_notif_form_data.enabled),
            min_criticality: Some(app.settings_notif_form_data.min_criticality),
        };
        app.db.update_notification(id, &update)
    } else {
        app.db.create_notification(&create).map(|_| ())
    };
    match res {
        Ok(_) => {
            app.settings_notif_form = false;
            app.settings_notif_form_data = NotificationForm::default();
            app.settings_notif_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
            app.refresh_settings();
            app.set_notification(
                "Notification saved".to_string(),
                crate::types::NotificationType::Success,
            );
        }
        Err(e) => app.set_notification(
            format!("Error: {}", e),
            crate::types::NotificationType::Error,
        ),
    }
}
