use crate::app::App;
use crate::ui::command_palette::PaletteMode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let state = &app.command_palette.state;
    if !state.is_open {
        return;
    }

    let area = f.area();
    let palette_width = (area.width * 3 / 4).clamp(60, 100);
    let palette_height = (area.height * 3 / 4).clamp(12, 30);
    let palette_area = Rect {
        x: (area.width - palette_width) / 2,
        y: (area.height - palette_height) / 3,
        width: palette_width,
        height: palette_height,
    };

    f.render_widget(Clear, palette_area);

    let block = Block::default()
        .title(" Command Palette ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    f.render_widget(block.clone(), palette_area);

    let inner = block.inner(palette_area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let input_text = match state.mode {
        PaletteMode::Fuzzy => format!("> {}", state.input),
        PaletteMode::Colon => state.input.clone(),
    };
    let input_paragraph = Paragraph::new(input_text)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.theme.border)),
        );
    f.render_widget(input_paragraph, chunks[0]);

    let results_area = chunks[1];
    let visible_count = results_area.height as usize;

    if state.results.is_empty() {
        let empty_text =
            if state.input.is_empty() || (state.mode == PaletteMode::Colon && state.input == ":") {
                "Type to search commands..."
            } else {
                "No commands found"
            };
        let empty = Paragraph::new(empty_text)
            .style(Style::default().fg(app.theme.muted))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(empty, results_area);
        return;
    }

    let start = state.scroll_offset;
    let end = (start + visible_count).min(state.results.len());

    for (i, cmd_match) in state.results[start..end].iter().enumerate() {
        let idx = start + i;
        let cmd = &cmd_match.command;
        let is_selected = idx == state.selected_index;

        let group_style = Style::default()
            .fg(app.theme.secondary)
            .add_modifier(Modifier::BOLD);
        let title_style = if is_selected {
            Style::default()
                .fg(app.theme.highlight)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(app.theme.fg)
        };
        let desc_style = Style::default().fg(app.theme.muted);

        let group_span = Span::styled(format!("{:>8} ", cmd.group.label()), group_style);
        let title_span = Span::styled(cmd.title.to_string(), title_style);

        let line = if results_area.width > 60 {
            let desc_span = Span::styled(format!("  {}", cmd.description), desc_style);
            Line::from(vec![group_span, title_span, desc_span])
        } else {
            Line::from(vec![group_span, title_span])
        };

        let row_area = Rect {
            x: results_area.x,
            y: results_area.y + i as u16,
            width: results_area.width,
            height: 1,
        };

        let paragraph = Paragraph::new(line);
        f.render_widget(paragraph, row_area);
    }
}
