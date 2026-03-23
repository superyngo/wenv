//! TUI rendering

use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::tui::app::TuiApp;
use crate::model::profile::ListItem as ProfileListItem;

pub fn draw(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // title bar
            Constraint::Min(1),     // main list
            Constraint::Length(1),  // status bar
        ])
        .split(f.size());

    draw_title(f, chunks[0], app);
    draw_list(f, chunks[1], app);
    draw_status(f, chunks[2], app);
}

fn draw_title(f: &mut Frame, area: Rect, app: &TuiApp) {
    let shell_name = app.profile.shell_type.name();
    let title = format!(" wenv — {} ", shell_name);
    let block = Block::default().style(Style::default().bg(Color::Blue).fg(Color::White));
    let text = Paragraph::new(title).block(block);
    f.render_widget(text, area);
}

fn draw_list(f: &mut Frame, area: Rect, app: &TuiApp) {
    let items: Vec<ratatui::widgets::ListItem> = app
        .visible_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_cursor = i == app.cursor;
            match item {
                ProfileListItem::FileHeader(fi) => {
                    let file = &app.profile.files[*fi];
                    let icon = if file.expanded { "▼" } else { "▶" };
                    let dirty = if file.dirty { " ●" } else { "" };
                    let text = format!("📜 {} {} [{} entries]{}", icon, file.display_name(), file.entry_count(), dirty);
                    let style = if is_cursor {
                        Style::default().bg(Color::DarkGray).fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    };
                    ratatui::widgets::ListItem::new(text).style(style)
                }
                ProfileListItem::Entry(fi, ei) => {
                    let entry = &app.profile.files[*fi].entries[*ei];
                    let type_str = format!("{:8}", entry.entry_type.to_string());
                    let name = &entry.name;
                    let text = format!("    {} {}", type_str, name);
                    let style = if is_cursor {
                        Style::default().bg(Color::DarkGray).fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    ratatui::widgets::ListItem::new(text).style(style)
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE));

    // Calculate scroll offset to keep cursor visible
    let visible_height = area.height as usize;
    let offset = if app.cursor >= visible_height {
        app.cursor - visible_height + 1
    } else {
        0
    };

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.cursor));
    // Use offset to handle scrolling
    *list_state.offset_mut() = offset;

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_status(f: &mut Frame, area: Rect, app: &TuiApp) {
    let status = if let Some(ref msg) = app.message {
        msg.clone()
    } else {
        let total = app.profile.total_entries();
        let files = app.profile.files.len();
        format!(" {} files, {} entries | q:quit ?:help 0/9:collapse/expand", files, total)
    };
    let text = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(text, area);
}