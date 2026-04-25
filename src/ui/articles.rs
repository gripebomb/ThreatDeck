use crate::app::App;
use crate::ui::list::{motion_from_key, move_selection, selected_style};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
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
        format!("Articles | / {}", app.articles_filter)
    } else {
        format!(
            "Articles | Filter: {}{}",
            if app.articles_filter.is_empty() {
                "none"
            } else {
                &app.articles_filter
            },
            if app.articles_unread_only {
                " | Unread only"
            } else {
                ""
            }
        )
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

    let header = Row::new(vec!["Read", "Published", "Feed", "Title"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );
    let rows: Vec<Row> = app
        .articles_list
        .iter()
        .enumerate()
        .map(|(i, article)| {
            let style = if i == app.articles_selected {
                selected_style()
            } else {
                Style::default().fg(app.theme.fg)
            };
            let read_mark = if article.item.read { "○" } else { "●" };
            let published = article
                .item
                .published_at
                .unwrap_or(article.item.fetched_at)
                .format("%Y-%m-%d %H:%M")
                .to_string();
            Row::new(vec![
                Cell::from(read_mark).style(Style::default().fg(if article.item.read {
                    app.theme.muted
                } else {
                    app.theme.primary
                })),
                Cell::from(published),
                Cell::from(article.feed_name.as_str()),
                Cell::from(article.item.title.as_str()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(5),
            Constraint::Length(16),
            Constraint::Min(18),
            Constraint::Percentage(50),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(table, chunks[1]);

    let status_text = if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear"
    } else {
        "-- NORMAL -- [1-8] Nav  [Enter] Read/fetch  [r] Toggle read  [u] Unread only  [/] Filter  [?] Help  [q] Quit"
    };
    f.render_widget(
        Paragraph::new(status_text).style(Style::default().fg(app.theme.muted)),
        chunks[2],
    );

    if app.articles_reader {
        draw_reader(f, app, area);
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.articles_reader {
        handle_reader_key(app, key);
        return;
    }

    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.articles_selected =
            move_selection(app.articles_selected, app.articles_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Enter => open_selected_article(app),
        KeyCode::Char('r') => {
            if let Some(article) = app.articles_list.get(app.articles_selected) {
                let new_read = !article.item.read;
                let _ = app.db.mark_feed_item_read(article.item.id, new_read);
                app.refresh_articles();
            }
        }
        KeyCode::Char('u') => {
            app.articles_unread_only = !app.articles_unread_only;
            app.refresh_articles();
        }
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = crate::app::InputMode::Typing;
        }
        _ => {}
    }
}

fn open_selected_article(app: &mut App) {
    let selected = app.articles_selected;
    let Some(article) = app.articles_list.get(selected) else {
        return;
    };
    let article_id = article.item.id;
    let url = article.item.url.clone();
    let needs_full_text = article
        .item
        .content
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty();

    let _ = app.db.mark_feed_item_read(article_id, true);

    if needs_full_text {
        if let Some(url) = url.as_deref().filter(|url| !url.trim().is_empty()) {
            match crate::article::fetch_article_text(url) {
                Ok(content) => {
                    if let Err(e) = app.db.cache_feed_item_content(article_id, &content) {
                        app.set_notification(
                            format!("Unable to cache full article: {}", e),
                            crate::types::NotificationType::Warning,
                        );
                    } else if let Some(article) = app.articles_list.get_mut(selected) {
                        article.item.content = Some(content);
                    }
                }
                Err(e) => {
                    app.set_notification(
                        format!("Full article unavailable; showing feed summary: {}", e),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
        } else {
            app.set_notification(
                "No article URL; showing feed summary".to_string(),
                crate::types::NotificationType::Warning,
            );
        }
    }

    if let Some(article) = app.articles_list.get_mut(selected) {
        article.item.read = true;
    }
    app.articles_reader = true;
    app.articles_scroll = 0;
}

fn handle_reader_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.articles_reader = false;
            app.refresh_articles();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.articles_scroll = app.articles_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.articles_scroll = app.articles_scroll.saturating_sub(1);
        }
        KeyCode::PageDown => {
            app.articles_scroll = app.articles_scroll.saturating_add(10);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.articles_scroll = app.articles_scroll.saturating_add(10);
        }
        KeyCode::PageUp => {
            app.articles_scroll = app.articles_scroll.saturating_sub(10);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.articles_scroll = app.articles_scroll.saturating_sub(10);
        }
        _ => {}
    }
}

fn draw_reader(f: &mut Frame, app: &App, area: Rect) {
    let Some(article) = app.articles_list.get(app.articles_selected) else {
        return;
    };
    let reader_area = Rect {
        x: area.width / 10,
        y: area.height / 8,
        width: area.width * 4 / 5,
        height: area.height * 3 / 4,
    };
    f.render_widget(Clear, reader_area);

    let published = article
        .item
        .published_at
        .unwrap_or(article.item.fetched_at)
        .format("%Y-%m-%d %H:%M UTC")
        .to_string();
    let url = article.item.url.as_deref().unwrap_or("no URL");
    let body_source = article
        .item
        .content
        .as_deref()
        .or(article.item.summary.as_deref())
        .unwrap_or("No article body was provided by this feed.");
    let cleaned = clean_article_text(body_source);
    let text = Text::from(vec![
        Line::from(article.item.title.as_str()).style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(format!("Feed: {}", article.feed_name)),
        Line::from(format!("Published: {}", published)),
        Line::from(format!("URL: {}", url)),
        Line::from(""),
        Line::from(cleaned),
    ]);
    let block = Block::default()
        .title("Article - Esc to close")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.bg))
        .wrap(Wrap { trim: false })
        .scroll((app.articles_scroll, 0));
    f.render_widget(paragraph, reader_area);
}

pub fn clean_article_text(input: &str) -> String {
    let decoded = input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let mut out = String::with_capacity(decoded.len());
    let mut in_tag = false;
    for ch in decoded.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_article_text_strips_html_and_entities() {
        assert_eq!(
            clean_article_text("<p>Hello&nbsp;<b>world</b>&amp; teams</p>"),
            "Hello world & teams"
        );
    }
}
