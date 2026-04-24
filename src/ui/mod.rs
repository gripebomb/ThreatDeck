pub mod utils;
pub mod dashboard;
pub mod feeds;
pub mod alerts;
pub mod keywords;
pub mod tags;
pub mod logs;
pub mod settings;

use ratatui::Frame;
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.screen {
        crate::types::Screen::Dashboard => dashboard::draw(f, app),
        crate::types::Screen::Feeds => feeds::draw(f, app),
        crate::types::Screen::Alerts => alerts::draw(f, app),
        crate::types::Screen::Keywords => keywords::draw(f, app),
        crate::types::Screen::Tags => tags::draw(f, app),
        crate::types::Screen::Logs => logs::draw(f, app),
        crate::types::Screen::Settings => settings::draw(f, app),
    }

    // Draw global notification toast
    if let Some((msg, typ)) = &app.notification {
        let area = f.area();
        let toast_area = ratatui::layout::Rect {
            x: area.width.saturating_sub(40),
            y: 0,
            width: 40,
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
            .style(ratatui::style::Style::default().fg(app.theme.fg).bg(app.theme.bg))
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(paragraph, toast_area);
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
            ratatui::text::Line::from("  1-7        Switch screens (Dashboard, Feeds, Alerts, Keywords, Tags, Logs, Settings)"),
            ratatui::text::Line::from("  q / Ctrl+C Quit"),
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
            .style(ratatui::style::Style::default().fg(app.theme.fg).bg(app.theme.bg))
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
            crate::types::ConfirmDialog::DeleteFeed { name, .. } => format!("Delete feed '{}' ?", name),
            crate::types::ConfirmDialog::DeleteKeyword { pattern, .. } => format!("Delete keyword '{}' ?", pattern),
            crate::types::ConfirmDialog::DeleteTag { name, .. } => format!("Delete tag '{}' ?", name),
            crate::types::ConfirmDialog::DeleteAlert { .. } => "Delete this alert?".to_string(),
            crate::types::ConfirmDialog::DeleteOldAlerts { count, .. } => format!("Delete {} old alerts?", count),
            crate::types::ConfirmDialog::DeleteNotification { name, .. } => format!("Delete notification '{}' ?", name),
            crate::types::ConfirmDialog::BulkDeleteAlerts { count } => format!("Delete {} selected alerts?", count),
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
            .style(ratatui::style::Style::default().fg(app.theme.fg).bg(app.theme.bg))
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(ratatui::widgets::Clear, dialog_area);
        f.render_widget(paragraph, dialog_area);
    }
}
