use crate::app::{App, InputMode};
use crate::types::*;
use crate::ui::list::{motion_from_key, move_selection, selected_style};
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

    let tab_titles = vec!["General", "Notifications", "Enrichment"];
    let tabs = Tabs::new(tab_titles)
        .select(match app.settings_tab {
            SettingsTab::General => 0,
            SettingsTab::Notifications => 1,
            SettingsTab::Enrichment => 2,
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
        SettingsTab::Enrichment => draw_enrichment(f, app, chunks[2]),
    }

    let status_text = if app.settings_notif_form && app.input_mode == InputMode::Typing {
        "-- INSERT -- Type to enter text | [Enter] Save | [Esc] Stop typing".to_string()
    } else if matches!(app.settings_tab, SettingsTab::Enrichment) {
        "-- NORMAL -- [1-9,0] Nav  [Tab] Tabs  [Space/e] Toggle  [t] Test provider  [r] Refresh  [?] Help  [q] Quit".to_string()
    } else {
        "-- NORMAL -- [1-9,0] Nav  [Tab] Tabs  [Left/Right] Theme  [c] Certs  [f] Auto-fetch  [i/j/e/o] Toggles  [s] Save  [?] Help  [q] Quit".to_string()
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
            Constraint::Length(3), // theme
            Constraint::Length(3), // retention
            Constraint::Length(3), // preview
            Constraint::Length(3), // ioc
            Constraint::Length(3), // enrichment
            Constraint::Length(3), // network
            Constraint::Length(3), // auto fetch
            Constraint::Length(5), // help
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

    let ioc_text = format!(
        "IOC extraction: {} | Raw JSON: {} | Max indicators/content: {}",
        if app.config.ioc.enabled {
            "enabled"
        } else {
            "disabled"
        },
        if app.config.ioc.extract_from_raw_json {
            "enabled"
        } else {
            "disabled"
        },
        app.config.ioc.max_indicators_per_content_item
    );
    let ioc_para = Paragraph::new(ioc_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(ioc_para, chunks[3]);

    let enrichment_text = format!(
        "Enrichment queueing: {} | Only alert indicators: {}",
        if app.config.enrichment.enabled {
            "enabled"
        } else {
            "disabled"
        },
        if app.config.enrichment.enrich_only_alert_indicators {
            "yes"
        } else {
            "no"
        }
    );
    let enrichment_para = Paragraph::new(enrichment_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(enrichment_para, chunks[4]);

    let network_text = format!("TLS trust store: {}", app.settings_tls_trust_store.label());
    let network_para = Paragraph::new(network_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(network_para, chunks[5]);

    let auto_fetch_text = format!(
        "Auto fetch: {} | Fetch interval: {} min",
        if app.settings_auto_fetch_enabled {
            "enabled"
        } else {
            "disabled"
        },
        app.settings_auto_fetch_interval
    );
    let auto_fetch_para = Paragraph::new(auto_fetch_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(auto_fetch_para, chunks[6]);

    let help = Paragraph::new("Keys: [Left/Right/Space] Theme  [c] TLS trust  [-/+] Interval  [f] Auto-fetch  [i/j/e/o] Toggles  [p] Preview  [s] Save")
        .style(Style::default().fg(app.theme.muted))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(help, chunks[7]);
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

fn draw_enrichment(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let header = Row::new(vec![
        "Provider",
        "Type",
        "Enabled",
        "Rate/min",
        "Supports",
        "Secret Ref",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let rows = app.settings_enrichment_providers.iter().map(|provider| {
        let supports = provider
            .supports_types
            .iter()
            .map(|indicator_type| format!("{indicator_type:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        Row::new(vec![
            Cell::from(provider.name.as_str()),
            Cell::from(provider.provider_type.as_str()),
            Cell::from(if provider.enabled { "✓" } else { "✗" }).style(Style::default().fg(
                if provider.enabled {
                    app.theme.success
                } else {
                    app.theme.error
                },
            )),
            Cell::from(
                provider
                    .rate_limit_per_minute
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            Cell::from(truncate_chars(&supports, 36)),
            Cell::from(provider.secret_ref.as_deref().unwrap_or("-")),
        ])
        .style(Style::default().fg(app.theme.fg))
    });

    let mut table_state = ratatui::widgets::TableState::default();
    table_state.select(Some(app.settings_enrichment_provider_selected));

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(20),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Min(20),
            Constraint::Min(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Enrichment Providers")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    )
    .row_highlight_style(selected_style());
    f.render_stateful_widget(table, area, &mut table_state);
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

    if matches!(app.settings_tab, SettingsTab::Enrichment) {
        handle_enrichment_key(app, key);
        if !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return;
        }
    }

    match key.code {
        KeyCode::Tab | KeyCode::BackTab => {
            app.settings_tab = match app.settings_tab {
                SettingsTab::General => SettingsTab::Notifications,
                SettingsTab::Notifications => SettingsTab::Enrichment,
                SettingsTab::Enrichment => SettingsTab::General,
            };
        }
        KeyCode::Right | KeyCode::Char(' ') if matches!(app.settings_tab, SettingsTab::General) => {
            cycle_theme(app, true);
        }
        KeyCode::Left if matches!(app.settings_tab, SettingsTab::General) => {
            cycle_theme(app, false);
        }
        KeyCode::Char('i') if matches!(app.settings_tab, SettingsTab::General) => {
            app.config.ioc.enabled = !app.config.ioc.enabled;
        }
        KeyCode::Char('j') if matches!(app.settings_tab, SettingsTab::General) => {
            app.config.ioc.extract_from_raw_json = !app.config.ioc.extract_from_raw_json;
        }
        KeyCode::Char('e') if matches!(app.settings_tab, SettingsTab::General) => {
            app.config.enrichment.enabled = !app.config.enrichment.enabled;
        }
        KeyCode::Char('o') if matches!(app.settings_tab, SettingsTab::General) => {
            app.config.enrichment.enrich_only_alert_indicators =
                !app.config.enrichment.enrich_only_alert_indicators;
        }
        KeyCode::Char('c') if matches!(app.settings_tab, SettingsTab::General) => {
            app.settings_tls_trust_store.toggle();
            app.set_notification(
                format!("TLS trust store: {}", app.settings_tls_trust_store.label()),
                crate::types::NotificationType::Info,
            );
        }
        KeyCode::Char('f') if matches!(app.settings_tab, SettingsTab::General) => {
            app.settings_auto_fetch_enabled = !app.settings_auto_fetch_enabled;
            if app.settings_auto_fetch_enabled {
                app.start_auto_fetch();
                app.set_notification(
                    "Auto-fetch enabled".to_string(),
                    crate::types::NotificationType::Success,
                );
            } else {
                app.stop_auto_fetch();
                app.set_notification(
                    "Auto-fetch disabled".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=')
            if matches!(app.settings_tab, SettingsTab::General)
                && app.settings_auto_fetch_interval < 60 =>
        {
            app.settings_auto_fetch_interval += 5;
            if app.settings_auto_fetch_enabled {
                app.restart_auto_fetch();
            }
            app.set_notification(
                format!(
                    "Auto-fetch interval: {} min",
                    app.settings_auto_fetch_interval
                ),
                crate::types::NotificationType::Info,
            );
        }
        KeyCode::Char('-')
            if matches!(app.settings_tab, SettingsTab::General)
                && app.settings_auto_fetch_interval > 5 =>
        {
            app.settings_auto_fetch_interval -= 5;
            if app.settings_auto_fetch_enabled {
                app.restart_auto_fetch();
            }
            app.set_notification(
                format!(
                    "Auto-fetch interval: {} min",
                    app.settings_auto_fetch_interval
                ),
                crate::types::NotificationType::Info,
            );
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
            let tls_trust_store_changed =
                app.config.network.tls_trust_store != app.settings_tls_trust_store;
            app.config.theme = app.settings_theme_name.clone();
            app.config.alert_retention_days = app.settings_retention_days;
            app.config.network.tls_trust_store = app.settings_tls_trust_store;
            app.config.auto_fetch.enabled = app.settings_auto_fetch_enabled;
            app.config.auto_fetch.interval_minutes = app.settings_auto_fetch_interval;
            app.theme = crate::theme::get_runtime_theme(&app.config.theme);
            if tls_trust_store_changed && app.settings_auto_fetch_enabled {
                app.restart_auto_fetch();
            }
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

fn handle_enrichment_key(app: &mut App, key: KeyEvent) {
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.settings_enrichment_provider_selected = move_selection(
            app.settings_enrichment_provider_selected,
            app.settings_enrichment_providers.len(),
            motion,
        );
        return;
    }

    match key.code {
        KeyCode::Char(' ') | KeyCode::Char('e') => {
            if let Some(provider) = app
                .settings_enrichment_providers
                .get(app.settings_enrichment_provider_selected)
                .cloned()
            {
                match app
                    .db
                    .set_enrichment_provider_enabled(&provider.name, !provider.enabled)
                {
                    Ok(()) => {
                        app.refresh_settings();
                        app.refresh_enrichment_queue();
                        let state = if provider.enabled {
                            "disabled"
                        } else {
                            "enabled"
                        };
                        app.set_notification(
                            format!("Provider {} {state}", provider.name),
                            NotificationType::Success,
                        );
                    }
                    Err(_) => app.set_notification(
                        format!("Unable to update provider {}", provider.name),
                        NotificationType::Error,
                    ),
                }
            }
        }
        KeyCode::Char('t') => {
            if let Some(provider) = app
                .settings_enrichment_providers
                .get(app.settings_enrichment_provider_selected)
            {
                match provider_health_message(provider, &app.paths.data_dir) {
                    Ok(message) => app.set_notification(message, NotificationType::Success),
                    Err(message) => app.set_notification(message, NotificationType::Warning),
                }
            }
        }
        KeyCode::Char('r') => app.refresh_settings(),
        _ => {}
    }
}

fn provider_health_message(
    provider: &crate::db::EnrichmentProviderRecord,
    data_dir: &std::path::Path,
) -> std::result::Result<String, String> {
    match provider.provider_type.as_str() {
        "cisa_kev" => {
            let path = data_dir.join("cisa-kev.json");
            if !path.exists() {
                return Err(format!("CISA KEV cache not installed: {}", path.display()));
            }
            sentinel_enrichment::CisaKevProvider::from_json_file(&path)
                .map(|_| format!("CISA KEV cache is valid: {}", path.display()))
                .map_err(|e| format!("CISA KEV cache failed validation: {}", e))
        }
        "urlhaus" => {
            if provider
                .secret_ref
                .as_deref()
                .is_some_and(|secret_ref| secret_ref.starts_with("env:"))
            {
                let env_name = provider
                    .secret_ref
                    .as_deref()
                    .unwrap_or_default()
                    .trim_start_matches("env:");
                if std::env::var(env_name).is_ok() {
                    Ok(format!(
                        "URLHaus Auth-Key environment variable {env_name} is set"
                    ))
                } else {
                    Err(format!(
                        "URLHaus Auth-Key environment variable {env_name} is not set"
                    ))
                }
            } else if provider
                .secret_ref
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                Ok("URLHaus Auth-Key is configured in provider secret_ref".into())
            } else {
                Err("URLHaus Auth-Key is required; set secret_ref to env:URLHAUS_AUTH_KEY".into())
            }
        }
        _ => Err(format!(
            "No local health check available for {}",
            provider.name
        )),
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

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!(
            "{}...",
            value
                .chars()
                .take(max.saturating_sub(3))
                .collect::<String>()
        )
    }
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

#[cfg(test)]
mod tests {
    use super::provider_health_message;
    use crate::db::EnrichmentProviderRecord;
    use chrono::Utc;

    fn provider(provider_type: &str) -> EnrichmentProviderRecord {
        EnrichmentProviderRecord {
            id: 1,
            name: "test-provider".into(),
            provider_type: provider_type.into(),
            enabled: true,
            config_json: None,
            secret_ref: None,
            rate_limit_per_minute: None,
            supports_types: vec![sentinel_ioc::IndicatorType::Cve],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn provider_with_secret(
        provider_type: &str,
        secret_ref: Option<&str>,
    ) -> EnrichmentProviderRecord {
        EnrichmentProviderRecord {
            secret_ref: secret_ref.map(str::to_string),
            ..provider(provider_type)
        }
    }

    #[test]
    fn provider_health_reports_missing_cisa_cache() {
        let dir = std::env::temp_dir().join(format!(
            "threatdeck-missing-cisa-cache-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let message = provider_health_message(&provider("cisa_kev"), &dir).unwrap_err();
        assert!(message.contains("not installed"));
    }

    #[test]
    fn provider_health_reports_missing_urlhaus_auth_key() {
        let message = provider_health_message(
            &provider_with_secret("urlhaus", Some("env:THREATDECK_TEST_MISSING_URLHAUS_KEY")),
            std::path::Path::new("/tmp"),
        )
        .unwrap_err();
        assert!(message.contains("not set"));
    }

    #[test]
    fn provider_health_reports_unsupported_provider() {
        let message = provider_health_message(&provider("unknown"), std::path::Path::new("/tmp"))
            .unwrap_err();
        assert!(message.contains("No local health check"));
    }
}
