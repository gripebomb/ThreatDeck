use crate::app::{App, InputMode};
use crate::types::Criticality;
use crate::ui::list::{criticality_label, motion_from_key, move_selection, selected_style};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
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

    let title_text = if app.filter_active {
        format!("Keywords | / {}", app.keywords_filter)
    } else if app.keywords_filter.is_empty() {
        "Keywords | Filter: none".to_string()
    } else {
        format!("Keywords | Filter: {}", app.keywords_filter)
    };
    let title = Paragraph::new(title_text)
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

    let header = Row::new(vec![
        "Pattern",
        "Type",
        "Case",
        "Criticality",
        "Enabled",
        "Tags",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let rows: Vec<Row> = app
        .keywords_list
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let style = if i == app.keywords_selected {
                selected_style()
            } else {
                Style::default().fg(app.theme.fg)
            };
            let crit_color = crate::theme::criticality_color(app.theme, k.criticality);
            let tag_str = app
                .keyword_tags
                .get(&k.id)
                .map(|tags| {
                    tags.iter()
                        .map(|t| t.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            Row::new(vec![
                Cell::from(k.pattern.as_str()).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(format!("{:?}", k.match_type())),
                Cell::from(if k.case_sensitive { "Aa" } else { "aa" }).style(Style::default().fg(
                    if k.case_sensitive {
                        app.theme.warning
                    } else {
                        app.theme.muted
                    },
                )),
                Cell::from(criticality_label(k.criticality))
                    .style(Style::default().fg(crit_color).add_modifier(Modifier::BOLD)),
                Cell::from(if k.enabled { "[x]" } else { "[ ]" }).style(Style::default().fg(
                    if k.enabled {
                        app.theme.success
                    } else {
                        app.theme.error
                    },
                )),
                Cell::from(tag_str),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Min(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(table, chunks[1]);

    let status_text = if app.keywords_show_form && app.input_mode == InputMode::Typing {
        "-- INSERT -- Type to enter text | [Enter] Save | [Esc] Stop typing".to_string()
    } else if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear".to_string()
    } else {
        "-- NORMAL -- [1-8] Nav  [a] Add  [e] Edit  [d] Delete  [t] Test  [Space] Toggle  [/] Filter  [?] Help  [q] Quit".to_string()
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);

    // Draw form overlay if active
    if app.keywords_show_form {
        draw_form(f, app);
    }

    // Draw test mode overlay if active
    if app.keywords_test_mode {
        draw_test_overlay(f, app);
    }
}

fn draw_form(f: &mut Frame, app: &App) {
    let area = f.area();
    let form_width = 60u16.min(area.width.saturating_sub(4)).max(45);
    let form_height = 22u16.min(area.height.saturating_sub(4));
    let form_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(form_width)) / 2,
        y: (area.height.saturating_sub(form_height)) / 2,
        width: form_width,
        height: form_height,
    };

    f.render_widget(Clear, form_area);

    let title = if app.keywords_form_edit_id.is_some() {
        "Edit Keyword"
    } else {
        "Add Keyword"
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
            Constraint::Length(3), // pattern (0)
            Constraint::Length(3), // is_regex toggle (1)
            Constraint::Length(3), // case_sensitive toggle (2)
            Constraint::Length(3), // criticality cycle (3)
            Constraint::Length(3), // enabled toggle (4)
            Constraint::Min(0),    // help text
        ])
        .split(inner);

    // Field 0: pattern (text)
    draw_text_field(f, app, 0, "Pattern *", &app.keywords_form.pattern, rows[1]);

    // Field 1: is_regex toggle
    let regex_label = if app.keywords_form.is_regex {
        "Regex"
    } else {
        "Simple"
    };
    draw_toggle_field(f, app, 1, "Match Type", regex_label, rows[2]);

    // Field 2: case_sensitive toggle
    let case_label = if app.keywords_form.case_sensitive {
        "Case Sensitive"
    } else {
        "Case Insensitive"
    };
    draw_toggle_field(f, app, 2, "Case Sensitivity", case_label, rows[3]);

    // Field 3: criticality cycle
    let crit_str = format!("{:?}", app.keywords_form.criticality);
    draw_cycle_field(f, app, 3, "Criticality", &crit_str, rows[4]);

    // Field 4: enabled toggle
    let enabled_label = if app.keywords_form.enabled {
        "Yes"
    } else {
        "No"
    };
    draw_toggle_field(f, app, 4, "Enabled", enabled_label, rows[5]);

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

fn draw_test_overlay(f: &mut Frame, app: &App) {
    let area = f.area();
    let test_area = ratatui::layout::Rect {
        x: area.width / 6,
        y: area.height / 4,
        width: area.width * 2 / 3,
        height: area.height / 2,
    };
    f.render_widget(Clear, test_area);
    let block = Block::default()
        .title("Keyword Test — Esc to close")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    f.render_widget(block.clone(), test_area);

    let inner = block.inner(test_area);
    let content =
        Paragraph::new("Keyword test mode placeholder. Type text to test against keywords.")
            .style(Style::default().fg(app.theme.fg));
    f.render_widget(content, inner);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.keywords_show_form {
        handle_form_key(app, key);
        return;
    }
    if app.keywords_test_mode {
        handle_test_key(app, key);
        return;
    }

    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.keywords_selected =
            move_selection(app.keywords_selected, app.keywords_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char('a') | KeyCode::Char('n') => {
            app.keywords_show_form = true;
            app.keywords_form = crate::types::KeywordForm::default();
            app.keywords_form_edit_id = None;
            app.form_focus = 0;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char('e') => {
            if let Some(k) = app.keywords_list.get(app.keywords_selected) {
                app.keywords_form = crate::types::KeywordForm {
                    pattern: k.pattern.clone(),
                    is_regex: k.is_regex,
                    case_sensitive: k.case_sensitive,
                    criticality: k.criticality,
                    enabled: k.enabled,
                };
                app.keywords_form_edit_id = Some(k.id);
                app.keywords_show_form = true;
                app.form_focus = 0;
                app.input_mode = InputMode::Normal;
            }
        }
        KeyCode::Char('d') => {
            if let Some(k) = app.keywords_list.get(app.keywords_selected) {
                app.show_confirm = Some(crate::types::ConfirmDialog::DeleteKeyword {
                    id: k.id,
                    pattern: k.pattern.clone(),
                });
            }
        }
        KeyCode::Char('t') => {
            app.keywords_test_mode = true;
            app.keywords_test_input.clear();
            app.keywords_test_results.clear();
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            if let Some(k) = app.keywords_list.get(app.keywords_selected) {
                let _ = app.db.toggle_keyword_enabled(k.id);
                app.refresh_keywords();
            }
        }
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = InputMode::Typing;
        }
        KeyCode::Char('T') => {
            if let Some(k) = app.keywords_list.get(app.keywords_selected) {
                app.tags_assignment_mode = true;
                app.tags_assignment_target = Some(crate::types::TagAssignmentTarget::Keyword(k.id));
                app.refresh_tags();
            }
        }
        _ => {}
    }
}

fn handle_form_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_form_normal_mode(app, key),
        InputMode::Typing => handle_form_typing_mode(app, key),
    }
}

fn handle_form_normal_mode(app: &mut App, key: KeyEvent) {
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
            app.keywords_show_form = false;
            app.keywords_form = crate::types::KeywordForm::default();
            app.keywords_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
        }
        // Enter Typing mode for text field (0 = pattern)
        KeyCode::Char('i') | KeyCode::Enter if app.form_focus == 0 => {
            app.input_mode = InputMode::Typing;
        }
        // Toggle fields (1 = is_regex, 2 = case_sensitive, 4 = enabled)
        KeyCode::Char(' ') | KeyCode::Enter if app.form_focus == 1 => {
            app.keywords_form.is_regex = !app.keywords_form.is_regex;
        }
        KeyCode::Char(' ') | KeyCode::Enter if app.form_focus == 2 => {
            app.keywords_form.case_sensitive = !app.keywords_form.case_sensitive;
        }
        KeyCode::Char(' ') | KeyCode::Enter if app.form_focus == 4 => {
            app.keywords_form.enabled = !app.keywords_form.enabled;
        }
        // Cycle criticality field (index 3) with arrows or Enter/Space
        KeyCode::Left | KeyCode::Right | KeyCode::Enter if app.form_focus == 3 => {
            cycle_criticality(
                &mut app.keywords_form.criticality,
                key.code == KeyCode::Right || key.code == KeyCode::Enter,
            );
        }
        KeyCode::Char(' ') if app.form_focus == 3 => {
            cycle_criticality(&mut app.keywords_form.criticality, true);
        }
        // Direct character input starts typing mode on text field
        KeyCode::Char(c) if app.form_focus == 0 => {
            app.input_mode = InputMode::Typing;
            app.keywords_form.pattern.push(c);
        }
        _ => {}
    }
}

fn handle_form_typing_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => submit_keyword_form(app),
        KeyCode::Backspace => {
            if app.form_focus == 0 {
                app.keywords_form.pattern.pop();
            }
        }
        KeyCode::Char(c) if app.form_focus == 0 => {
            app.keywords_form.pattern.push(c);
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

fn submit_keyword_form(app: &mut App) {
    let create = crate::db::KeywordCreate {
        pattern: app.keywords_form.pattern.clone(),
        is_regex: app.keywords_form.is_regex,
        case_sensitive: app.keywords_form.case_sensitive,
        criticality: app.keywords_form.criticality,
        enabled: app.keywords_form.enabled,
    };
    let res = if let Some(id) = app.keywords_form_edit_id {
        let update = crate::db::KeywordUpdate {
            pattern: Some(app.keywords_form.pattern.clone()),
            is_regex: Some(app.keywords_form.is_regex),
            case_sensitive: Some(app.keywords_form.case_sensitive),
            criticality: Some(app.keywords_form.criticality),
            enabled: Some(app.keywords_form.enabled),
        };
        app.db.update_keyword(id, &update)
    } else {
        app.db.create_keyword(&create).map(|_| ())
    };
    match res {
        Ok(_) => {
            app.keywords_show_form = false;
            app.keywords_form = crate::types::KeywordForm::default();
            app.keywords_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
            app.refresh_keywords();
            app.set_notification(
                "Keyword saved".to_string(),
                crate::types::NotificationType::Success,
            );
        }
        Err(e) => app.set_notification(
            format!("Error: {}", e),
            crate::types::NotificationType::Error,
        ),
    }
}

fn handle_test_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.keywords_test_mode = false;
        }
        _ => {}
    }
}
