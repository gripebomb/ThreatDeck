pub mod alerts;
pub mod articles;
pub mod dashboard;
pub mod feeds;
pub mod keywords;
pub mod list;
pub mod logs;
pub mod settings;
pub mod tags;
pub mod utils;

use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if area.width < 80 || area.height < 24 {
        let msg = Paragraph::new("Please resize the terminal to at least 80 x 24.")
            .style(Style::default().fg(app.theme.warning))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(app.theme.border)),
            );
        f.render_widget(msg, area);
        return;
    }

    match app.screen {
        crate::types::Screen::Dashboard => dashboard::draw(f, app),
        crate::types::Screen::Feeds => feeds::draw(f, app),
        crate::types::Screen::Alerts => alerts::draw(f, app),
        crate::types::Screen::Articles => articles::draw(f, app),
        crate::types::Screen::Keywords => keywords::draw(f, app),
        crate::types::Screen::Tags => tags::draw(f, app),
        crate::types::Screen::Logs => logs::draw(f, app),
        crate::types::Screen::Settings => settings::draw(f, app),
    }

    draw_nav_tabs(f, app);

    // Draw global notification toast
    if let Some((msg, typ)) = &app.notification {
        let area = f.area();
        let width = 40u16.min(area.width.saturating_sub(4)).max(20);
        let toast_area = ratatui::layout::Rect {
            x: area.width.saturating_sub(width + 1),
            y: area.height.saturating_sub(5),
            width,
            height: 3,
        };
        let color = match typ {
            crate::types::NotificationType::Info => app.theme.primary,
            crate::types::NotificationType::Success => app.theme.success,
            crate::types::NotificationType::Warning => app.theme.warning,
            crate::types::NotificationType::Error => app.theme.error,
        };
        let block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(ratatui::style::Style::default().fg(color));
        let paragraph = ratatui::widgets::Paragraph::new(msg.as_str())
            .style(
                ratatui::style::Style::default()
                    .fg(app.theme.fg)
                    .bg(app.theme.bg),
            )
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(paragraph, toast_area);
    }

    if app.tags_assignment_mode {
        draw_tag_assignment(f, app);
    }

    // Draw help overlay
    if app.show_help {
        let area = f.area();
        let help_area = ratatui::layout::Rect {
            x: area.width / 4,
            y: area.height / 4,
            width: area.width / 2,
            height: area.height / 2,
        };
        let block = ratatui::widgets::Block::default()
            .title("Help - Press ? or Esc to close")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(ratatui::style::Style::default().fg(app.theme.primary));
        let text = ratatui::text::Text::from(vec![
            ratatui::text::Line::from("Global Keys:"),
            ratatui::text::Line::from("  1-8        Switch screens (Dashboard, Feeds, Alerts, Articles, Keywords, Tags, Logs, Settings)"),
            ratatui::text::Line::from("  q          Quit"),
            ratatui::text::Line::from("  /          Filter current list"),
            ratatui::text::Line::from("  gg / G     Jump to top / bottom"),
            ratatui::text::Line::from("  Ctrl+d/u   Half-page down / up"),
            ratatui::text::Line::from("  ? / F1     Toggle help"),
            ratatui::text::Line::from("  Esc        Cancel / Go back"),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from("Form Input Mode:"),
            ratatui::text::Line::from("  i / Enter  Start typing in the focused field"),
            ratatui::text::Line::from("  Tab        Move to next field (Normal mode)"),
            ratatui::text::Line::from("  Space      Toggle boolean fields"),
            ratatui::text::Line::from("  <- ->      Cycle enum fields"),
            ratatui::text::Line::from("  Esc        Exit Typing mode / Cancel form"),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from("When in Typing mode:"),
            ratatui::text::Line::from("  Type text  Enter characters into the focused field"),
            ratatui::text::Line::from("  Backspace  Delete last character"),
            ratatui::text::Line::from("  Enter      Submit the form"),
            ratatui::text::Line::from("  Esc        Exit Typing mode (return to Normal mode)"),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from("Screen-specific keys shown in status bar."),
        ]);
        let paragraph = ratatui::widgets::Paragraph::new(text)
            .style(
                ratatui::style::Style::default()
                    .fg(app.theme.fg)
                    .bg(app.theme.bg),
            )
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(ratatui::widgets::Clear, help_area);
        f.render_widget(paragraph, help_area);
    }

    // Draw confirm dialog
    if let Some(dialog) = &app.show_confirm {
        let area = f.area();
        let dialog_area = ratatui::layout::Rect {
            x: area.width / 4,
            y: area.height / 3,
            width: area.width / 2,
            height: 8,
        };
        let msg = match dialog {
            crate::types::ConfirmDialog::DeleteFeed { name, .. } => {
                format!("Delete feed '{}' ?", name)
            }
            crate::types::ConfirmDialog::DeleteKeyword { pattern, .. } => {
                format!("Delete keyword '{}' ?", pattern)
            }
            crate::types::ConfirmDialog::DeleteTag { name, .. } => {
                format!("Delete tag '{}' ?", name)
            }
            crate::types::ConfirmDialog::DeleteAlert { .. } => "Delete this alert?".to_string(),
            crate::types::ConfirmDialog::DeleteOldAlerts { count, .. } => {
                format!("Delete {} old alerts?", count)
            }
            crate::types::ConfirmDialog::DeleteNotification { name, .. } => {
                format!("Delete notification '{}' ?", name)
            }
            crate::types::ConfirmDialog::BulkDeleteAlerts { count } => {
                format!("Delete {} selected alerts?", count)
            }
        };
        let block = ratatui::widgets::Block::default()
            .title("Confirm")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(ratatui::style::Style::default().fg(app.theme.warning));
        let text = ratatui::text::Text::from(vec![
            ratatui::text::Line::from(msg),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from("Press [y] to confirm, [n] or [Esc] to cancel."),
        ]);
        let paragraph = ratatui::widgets::Paragraph::new(text)
            .style(
                ratatui::style::Style::default()
                    .fg(app.theme.fg)
                    .bg(app.theme.bg),
            )
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(ratatui::widgets::Clear, dialog_area);
        f.render_widget(paragraph, dialog_area);
    }
}

fn draw_nav_tabs(f: &mut Frame, app: &App) {
    let area = f.area();
    let titles = vec![
        "[1] Dashboard",
        "[2] Feeds",
        "[3] Alerts",
        "[4] Articles",
        "[5] Keywords",
        "[6] Tags",
        "[7] Logs",
        "[8] Settings",
    ];
    let selected = match app.screen {
        crate::types::Screen::Dashboard => 0,
        crate::types::Screen::Feeds => 1,
        crate::types::Screen::Alerts => 2,
        crate::types::Screen::Articles => 3,
        crate::types::Screen::Keywords => 4,
        crate::types::Screen::Tags => 5,
        crate::types::Screen::Logs => 6,
        crate::types::Screen::Settings => 7,
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(app.theme.muted).bg(app.theme.surface))
        .highlight_style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        );
    f.render_widget(
        tabs,
        Rect {
            x: 0,
            y: 0,
            width: area.width,
            height: 1,
        },
    );
}

fn draw_tag_assignment(f: &mut Frame, app: &App) {
    let area = f.area();
    let popup = Rect {
        x: area.width / 4,
        y: area.height / 5,
        width: area.width / 2,
        height: (area.height * 3 / 5).max(10),
    };
    f.render_widget(Clear, popup);

    let target = match app.tags_assignment_target {
        Some(crate::types::TagAssignmentTarget::Feed(_)) => "Feed",
        Some(crate::types::TagAssignmentTarget::Keyword(_)) => "Keyword",
        Some(crate::types::TagAssignmentTarget::Alert(_)) => "Alert",
        None => "Item",
    };

    let items: Vec<ListItem> = app
        .tags_list
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            let assigned = match app.tags_assignment_target {
                Some(crate::types::TagAssignmentTarget::Feed(id)) => app
                    .db
                    .get_feed_tags(id)
                    .map(|tags| tags.iter().any(|t| t.id == tag.id))
                    .unwrap_or(false),
                Some(crate::types::TagAssignmentTarget::Keyword(id)) => app
                    .db
                    .get_keyword_tags(id)
                    .map(|tags| tags.iter().any(|t| t.id == tag.id))
                    .unwrap_or(false),
                Some(crate::types::TagAssignmentTarget::Alert(id)) => app
                    .db
                    .get_alert_tags(id)
                    .map(|tags| tags.iter().any(|t| t.id == tag.id))
                    .unwrap_or(false),
                None => false,
            };
            let mark = if assigned { "[x]" } else { "[ ]" };
            let style = if i == app.tags_selected {
                crate::ui::list::selected_style()
            } else {
                Style::default().fg(app.theme.fg)
            };
            ListItem::new(format!("{} {}", mark, tag.name)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(
                    "Assign Tags to {} - Space toggles, Enter saves, Esc closes",
                    target
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.primary)),
        )
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface));
    f.render_widget(list, popup);
}
