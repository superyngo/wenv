//! TUI rendering

use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::tui::app::TuiApp;
use crate::tui::state::AppMode;
use crate::model::profile::ListItem as ProfileListItem;

pub fn draw(f: &mut Frame, app: &TuiApp) {
    let has_search = app.mode == AppMode::Searching;
    let constraints = if has_search {
        vec![
            Constraint::Length(1),  // title bar
            Constraint::Min(1),     // main list
            Constraint::Length(1),  // search bar
            Constraint::Length(1),  // status bar
        ]
    } else {
        vec![
            Constraint::Length(1),  // title bar
            Constraint::Min(1),     // main list
            Constraint::Length(1),  // status bar
        ]
    };
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.size());

    draw_title(f, chunks[0], app);
    draw_list(f, chunks[1], app);
    
    if has_search {
        draw_search_bar(f, chunks[2], app);
        draw_status(f, chunks[3], app);
    } else {
        draw_status(f, chunks[2], app);
    }

    // Draw confirmation popups on top
    match &app.mode {
        AppMode::ConfirmDelete | AppMode::ConfirmQuit => {
            draw_confirm_popup(f, f.size(), app);
        }
        AppMode::ShowingDetail => {
            draw_detail_popup(f, f.size(), app);
        }
        AppMode::ShowingHelp => {
            draw_help_popup(f, f.size(), app);
        }
        _ => {}
    }
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
            let is_selected = app.selection.is_selected(i);
            let is_move_target = app.mode == AppMode::Moving 
                && app.move_state.as_ref().is_some_and(|ms| ms.insertion_cursor == i);
            
            match item {
                ProfileListItem::FileHeader(fi) => {
                    let file = &app.profile.files[*fi];
                    let icon = if file.expanded { "▼" } else { "▶" };
                    let dirty = if file.dirty { " ●" } else { "" };
                    let text = format!("📜 {} {} [{} entries]{}", icon, file.display_name(), file.entry_count(), dirty);
                    
                    let mut style = if is_cursor {
                        Style::default().bg(Color::DarkGray).fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    };
                    
                    // Override with move target style
                    if is_move_target {
                        style = style.bg(Color::Green).fg(Color::Black);
                    }
                    
                    ratatui::widgets::ListItem::new(text).style(style)
                }
                ProfileListItem::Entry(fi, ei) => {
                    let entry = &app.profile.files[*fi].entries[*ei];
                    let type_str = format!("{:8}", entry.entry_type.to_string());
                    let name = &entry.name;
                    
                    // Add selection marker prefix
                    let prefix = if is_selected { "● " } else { "  " };
                    let text = format!("  {}{} {}", prefix, type_str, name);
                    
                    // When rendering an Entry, check if it's a search match
                    let is_search_match = app.search.as_ref()
                        .is_some_and(|s| s.is_match(*fi, *ei));
                    let is_search_selected = app.search.as_ref()
                        .is_some_and(|s| s.is_selected_match(*fi, *ei));

                    // Adjust the style based on search state
                    let mut style = if app.mode == AppMode::Searching {
                        if is_search_selected {
                            Style::default().bg(Color::Yellow).fg(Color::Black)  // Highlighted match
                        } else if is_search_match {
                            Style::default().fg(Color::White)  // Match but not selected
                        } else {
                            Style::default().fg(Color::DarkGray)  // Non-match (dimmed)
                        }
                    } else {
                        // Existing style logic (cursor, selection)
                        match (is_cursor, is_selected) {
                            (true, true) => Style::default().bg(Color::Cyan).fg(Color::Black),      // cursor + selected
                            (true, false) => Style::default().bg(Color::DarkGray).fg(Color::White), // cursor only
                            (false, true) => Style::default().bg(Color::Blue).fg(Color::White),     // selected only
                            (false, false) => Style::default().fg(Color::Gray),                      // normal
                        }
                    };
                    
                    // Override with move target style
                    if is_move_target {
                        style = style.bg(Color::Green).fg(Color::Black);
                    }
                    
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
        if app.selection.selected_count() > 0 {
            format!(" {} selected | {} files, {} entries | q:quit ?:help s:select", 
                app.selection.selected_count(), files, total)
        } else {
            format!(" {} files, {} entries | q:quit ?:help 0/9:collapse/expand", files, total)
        }
    };
    let text = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(text, area);
}

fn draw_confirm_popup(f: &mut Frame, area: Rect, app: &TuiApp) {
    let msg = app.message.as_deref().unwrap_or("Confirm? (y/n)");
    
    // Center a popup
    let popup_width = (msg.len() as u16 + 4).min(area.width - 4);
    let popup_height = 3;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear area behind popup
    f.render_widget(Clear, popup_area);

    let title = match &app.mode {
        AppMode::ConfirmDelete => " Confirm Delete ",
        AppMode::ConfirmQuit => " Unsaved Changes ",
        _ => " Confirm ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let text = Paragraph::new(msg)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(text, popup_area);
}

fn draw_search_bar(f: &mut Frame, area: Rect, app: &TuiApp) {
    if let Some(ref search) = app.search {
        let match_count = search.matches.len();
        let text = format!(" / {}  [{} matches]", search.query, match_count);
        let style = Style::default().bg(Color::Black).fg(Color::Yellow);
        f.render_widget(Paragraph::new(text).style(style), area);
    }
}

fn draw_detail_popup(f: &mut Frame, area: Rect, app: &TuiApp) {
    // Get the entry at cursor
    let (fi, ei) = match app.visible_items.get(app.cursor) {
        Some(ProfileListItem::Entry(fi, ei)) => (*fi, *ei),
        _ => return,
    };
    let file = &app.profile.files[fi];
    let entry = &file.entries[ei];

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("File: ", Style::default().fg(Color::Yellow)),
        Span::raw(file.display_name()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Type: ", Style::default().fg(Color::Yellow)),
        Span::raw(entry.entry_type.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Name: ", Style::default().fg(Color::Yellow)),
        Span::raw(&entry.name),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Lines: ", Style::default().fg(Color::Yellow)),
        Span::raw(match (entry.line_number, entry.end_line) {
            (Some(start), Some(end)) => format!("{}-{}", start, end),
            (Some(start), None) => format!("{}", start),
            _ => "unknown".to_string(),
        }),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Value:", Style::default().fg(Color::Yellow))));
    
    // Split value by \n and show each line
    for line in entry.value.split('\n') {
        lines.push(Line::from(Span::raw(format!("  {}", line))));
    }

    // Size popup to content, max 80% of screen
    let max_width = (area.width * 4 / 5).max(40);
    let max_height = (area.height * 4 / 5).max(10);
    let content_height = (lines.len() as u16 + 2).min(max_height); // +2 for borders
    let popup_width = max_width;
    let popup_height = content_height;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Entry Detail (Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let text = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(text, popup_area);
}

fn draw_help_popup(f: &mut Frame, area: Rect, _app: &TuiApp) {
    let lines = vec![
        Line::from(Span::styled("Navigation", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  ↑/k ↓/j    Navigate up/down"),
        Line::from("  Home/End    Jump to first/last"),
        Line::from("  Enter/Space Toggle file / View entry detail"),
        Line::from("  0/9         Collapse/Expand all files"),
        Line::from(""),
        Line::from(Span::styled("Selection", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  s           Toggle select entry"),
        Line::from("  Shift+↑/↓  Range select"),
        Line::from(""),
        Line::from(Span::styled("Editing", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  e           Edit (file or entry in $EDITOR)"),
        Line::from("  a           Add new entry via $EDITOR"),
        Line::from("  d           Delete selected entries"),
        Line::from("  x           Cut selected entries"),
        Line::from("  p           Paste entries"),
        Line::from("  m           Move mode (drag to reposition)"),
        Line::from("  u           Undo last operation"),
        Line::from(""),
        Line::from(Span::styled("Other", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  /           Fuzzy search"),
        Line::from("  w/Ctrl+s    Save all changes"),
        Line::from("  q           Quit"),
        Line::from("  ?           Show this help"),
    ];

    let popup_height = (lines.len() as u16 + 2).min(area.height - 2);
    let popup_width = 50u16.min(area.width - 4);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Help (Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let text = Paragraph::new(lines).block(block);
    f.render_widget(text, popup_area);
}