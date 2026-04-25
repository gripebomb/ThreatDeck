use crate::app::{App, InputMode};
use crate::ui::list::{motion_from_key, move_selection, selected_style};
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
        format!("Tags | / {}", app.tags_filter)
    } else if app.tags_filter.is_empty() {
        "Tags | Filter: none".to_string()
    } else {
        format!("Tags | Filter: {}", app.tags_filter)
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

    let header = Row::new(vec!["Name", "Color", "Description", "Usage"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let rows: Vec<Row> = app
        .tags_list
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == app.tags_selected {
                selected_style()
            } else {
                Style::default().fg(app.theme.fg)
            };
            let color = crate::theme::hex_to_color(&t.color);
            Row::new(vec![
                Cell::from(t.name.as_str()).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from("██").style(Style::default().fg(color)),
                Cell::from(t.description.as_deref().unwrap_or("")),
                Cell::from(
                    app.tag_usage_counts
                        .get(&t.id)
                        .copied()
                        .unwrap_or(0)
                        .to_string(),
                ),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Min(30),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(table, chunks[1]);

    let status_text = if app.tags_show_form && app.input_mode == InputMode::Typing {
        "-- INSERT -- Type to enter text | [Enter] Save | [Esc] Stop typing".to_string()
    } else if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear".to_string()
    } else {
        "-- NORMAL -- [1-8] Nav  [a] Add  [e] Edit  [d] Delete  [Enter] View items  [/] Filter  [?] Help  [q] Quit".to_string()
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);

    // Draw form overlay if active
    if app.tags_show_form {
        draw_form(f, app);
    }
}

fn draw_form(f: &mut Frame, app: &App) {
    let area = f.area();
    let form_width = 60u16.min(area.width.saturating_sub(4)).max(45);
    let form_height = 20u16.min(area.height.saturating_sub(4));
    let form_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(form_width)) / 2,
        y: (area.height.saturating_sub(form_height)) / 2,
        width: form_width,
        height: form_height,
    };

    f.render_widget(Clear, form_area);

    let title = if app.tags_form_edit_id.is_some() {
        "Edit Tag"
    } else {
        "Add Tag"
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
            Constraint::Length(3), // color (1)
            Constraint::Length(3), // description (2)
            Constraint::Min(0),    // help text
        ])
        .split(inner);

    // Field 0: name (text)
    draw_text_field(f, app, 0, "Name *", &app.tags_form.name, rows[1]);

    // Field 1: color (text)
    draw_text_field(
        f,
        app,
        1,
        "Color (hex, e.g. #ff0000)",
        &app.tags_form.color,
        rows[2],
    );

    // Field 2: description (text)
    draw_text_field(
        f,
        app,
        2,
        "Description",
        &app.tags_form.description,
        rows[3],
    );

    // Help text
    let help_text = if app.input_mode == InputMode::Typing {
        "[Type] Enter text  [Backspace] Delete  [Enter] Submit form  [Esc] Cancel typing"
    } else {
        "[Tab] Next field  [i/Enter] Start typing  [Esc] Cancel form"
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(help, rows[4]);
}

/// Draw a text input field with focus highlight and cursor indicator
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

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.tags_show_form {
        handle_form_key(app, key);
        return;
    }

    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.tags_selected = move_selection(app.tags_selected, app.tags_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char('a') | KeyCode::Char('n') => {
            app.tags_show_form = true;
            app.tags_form = crate::types::TagForm::default();
            app.tags_form_edit_id = None;
            app.form_focus = 0;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char('e') => {
            if let Some(t) = app.tags_list.get(app.tags_selected) {
                app.tags_form = crate::types::TagForm {
                    name: t.name.clone(),
                    color: t.color.clone(),
                    description: t.description.clone().unwrap_or_default(),
                };
                app.tags_form_edit_id = Some(t.id);
                app.tags_show_form = true;
                app.form_focus = 0;
                app.input_mode = InputMode::Normal;
            }
        }
        KeyCode::Char('d') => {
            if let Some(t) = app.tags_list.get(app.tags_selected) {
                app.show_confirm = Some(crate::types::ConfirmDialog::DeleteTag {
                    id: t.id,
                    name: t.name.clone(),
                });
            }
        }
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = InputMode::Typing;
        }
        KeyCode::Enter => {
            if let Some(t) = app.tags_list.get(app.tags_selected) {
                app.set_notification(
                    format!(
                        "Tag '{}' has {} linked items",
                        t.name,
                        app.tag_usage_counts.get(&t.id).copied().unwrap_or(0)
                    ),
                    crate::types::NotificationType::Info,
                );
            }
        }
        _ => {}
    }
}

/// Handle keystrokes when the tag form is open.
/// All 3 fields are text input, so Typing mode applies to all of them.
fn handle_form_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_form_normal_mode(app, key),
        InputMode::Typing => handle_form_typing_mode(app, key),
    }
}

fn handle_form_normal_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Tab => app.form_focus = (app.form_focus + 1) % 3,
        KeyCode::BackTab => {
            app.form_focus = if app.form_focus == 0 {
                2
            } else {
                app.form_focus - 1
            };
        }
        KeyCode::Esc => {
            app.tags_show_form = false;
            app.tags_form = crate::types::TagForm::default();
            app.tags_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
        }
        // Enter Typing mode for any text field
        KeyCode::Char('i') | KeyCode::Enter => {
            app.input_mode = InputMode::Typing;
        }
        // Direct character input starts typing mode immediately
        KeyCode::Char(c) => {
            app.input_mode = InputMode::Typing;
            append_to_tag_field(app, c);
        }
        _ => {}
    }
}

fn handle_form_typing_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => submit_tag_form(app),
        KeyCode::Backspace => {
            backspace_tag_field(app);
        }
        KeyCode::Char(c) => {
            append_to_tag_field(app, c);
        }
        KeyCode::Tab => {
            app.input_mode = InputMode::Normal;
            app.form_focus = (app.form_focus + 1) % 3;
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

fn append_to_tag_field(app: &mut App, c: char) {
    match app.form_focus {
        0 => app.tags_form.name.push(c),
        1 => app.tags_form.color.push(c),
        2 => app.tags_form.description.push(c),
        _ => {}
    }
}

fn backspace_tag_field(app: &mut App) {
    match app.form_focus {
        0 => {
            app.tags_form.name.pop();
        }
        1 => {
            app.tags_form.color.pop();
        }
        2 => {
            app.tags_form.description.pop();
        }
        _ => {}
    }
}

fn submit_tag_form(app: &mut App) {
    let create = crate::db::TagCreate {
        name: app.tags_form.name.clone(),
        color: app.tags_form.color.clone(),
        description: if app.tags_form.description.is_empty() {
            None
        } else {
            Some(app.tags_form.description.clone())
        },
    };
    let res = if let Some(id) = app.tags_form_edit_id {
        let update = crate::db::TagUpdate {
            name: Some(app.tags_form.name.clone()),
            color: Some(app.tags_form.color.clone()),
            description: if app.tags_form.description.is_empty() {
                None
            } else {
                Some(app.tags_form.description.clone())
            },
        };
        app.db.update_tag(id, &update)
    } else {
        app.db.create_tag(&create).map(|_| ())
    };
    match res {
        Ok(_) => {
            app.tags_show_form = false;
            app.tags_form = crate::types::TagForm::default();
            app.tags_form_edit_id = None;
            app.input_mode = InputMode::Normal;
            app.form_focus = 0;
            app.refresh_tags();
            app.set_notification(
                "Tag saved".to_string(),
                crate::types::NotificationType::Success,
            );
        }
        Err(e) => app.set_notification(
            format!("Error: {}", e),
            crate::types::NotificationType::Error,
        ),
    }
}
