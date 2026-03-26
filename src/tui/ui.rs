//! TUI rendering

use crate::model::profile::ListItem as ProfileListItem;
use crate::model::EntryType;
use crate::tui::app::TuiApp;
use crate::tui::state::AppMode;
use ratatui::prelude::*;
use ratatui::widgets::*;

/// Map entry type to its display color (replicating pre-redesign scheme)
fn type_color(et: &EntryType) -> Color {
    match et {
        EntryType::Alias => Color::Green,
        EntryType::Function => Color::LightBlue,
        EntryType::EnvVar => Color::Yellow,
        EntryType::Source => Color::Magenta,
        EntryType::Code => Color::Cyan,
        EntryType::Comment => Color::White,
    }
}

/// Format line number display
fn format_line_info(entry: &crate::model::Entry) -> String {
    match (entry.line_number, entry.end_line) {
        (Some(start), Some(end)) if end > start => format!("{}-{}", start, end),
        (Some(line), _) => format!("{}", line),
        (None, _) => "-".to_string(),
    }
}

/// Truncate and sanitise value for single-line display
fn format_value_display(value: &str) -> String {
    let v = value.replace('\n', "\\n");
    if v.chars().count() > 100 {
        format!("{}...", v.chars().take(97).collect::<String>())
    } else {
        v
    }
}

/// Build spans for a string with highlighted character positions.
/// Characters at `highlight_indices` get `hl_style`, others get `normal_style`.
fn build_highlighted_spans<'a>(
    text: &str,
    highlight_indices: &[usize],
    normal_style: Style,
    hl_style: Style,
) -> Vec<Span<'a>> {
    if highlight_indices.is_empty() {
        return vec![Span::styled(text.to_string(), normal_style)];
    }

    let hl_set: std::collections::HashSet<usize> = highlight_indices.iter().copied().collect();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_is_hl = false;

    for (i, ch) in text.chars().enumerate() {
        let is_hl = hl_set.contains(&i);
        if i == 0 {
            current_is_hl = is_hl;
        }
        if is_hl != current_is_hl {
            if !current.is_empty() {
                let style = if current_is_hl {
                    hl_style
                } else {
                    normal_style
                };
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            current_is_hl = is_hl;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        let style = if current_is_hl {
            hl_style
        } else {
            normal_style
        };
        spans.push(Span::styled(current, style));
    }
    spans
}

pub fn draw(f: &mut Frame, app: &TuiApp) {
    let has_search = app.search.is_some();
    let has_text_input = app.text_input.is_some();
    let constraints = if has_search || has_text_input {
        vec![
            Constraint::Length(1), // title bar
            Constraint::Min(1),    // main list
            Constraint::Length(1), // search/text input bar
            Constraint::Length(1), // status bar
        ]
    } else {
        vec![
            Constraint::Length(1), // title bar
            Constraint::Min(1),    // main list
            Constraint::Length(1), // status bar
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
    } else if has_text_input {
        draw_text_input_bar(f, chunks[2], app);
        draw_status(f, chunks[3], app);
    } else {
        draw_status(f, chunks[2], app);
    }

    // Draw confirmation popups on top
    match &app.mode {
        AppMode::ConfirmDelete | AppMode::ConfirmQuit 
        | AppMode::ConfirmRemoveFile | AppMode::ConfirmCreateFile => {
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
    // Reserve rows for fixed header (1) + separator (1)
    let header_height: u16 = 2;
    if area.height <= header_height {
        return;
    }

    let header_area = Rect::new(area.x, area.y, area.width, header_height);
    let list_area = Rect::new(
        area.x,
        area.y + header_height,
        area.width,
        area.height - header_height,
    );

    // --- Fixed column header ---
    let header_line = Line::from(vec![
        Span::raw("  "), // indent to match entry prefix
        Span::styled(
            format!("{:<20}", "NAME"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<10}", "TYPE"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<10}", "LINE"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            "VALUE",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let separator_line = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(Color::DarkGray),
    ));
    let header_widget = Paragraph::new(vec![header_line, separator_line]);
    f.render_widget(header_widget, header_area);

    // --- Entry list ---
    let items: Vec<ratatui::widgets::ListItem> = app
        .visible_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_cursor = i == app.cursor;
            let is_selected = app.selection.is_selected(i);
            let is_move_target = (app.mode == AppMode::Moving
                && app
                    .move_state
                    .as_ref()
                    .is_some_and(|ms| ms.insertion_cursor == i))
                || (app.mode == AppMode::MovingFile
                    && app
                        .file_move_state
                        .as_ref()
                        .is_some_and(|fms| fms.insertion_cursor == i));

            match item {
                ProfileListItem::FileHeader(fi) => {
                    let file = &app.profile.files[*fi];
                    let icon = if file.expanded { "▼" } else { "▶" };
                    let dirty = if file.dirty { " ●" } else { "" };
                    let readonly = if !file.writable { " 🔒" } else { "" };
                    let text = format!(
                        "📜 {} {} [{} entries]{}{}",
                        icon,
                        file.display_name(),
                        file.entry_count(),
                        dirty,
                        readonly
                    );

                    let mut style = if !file.writable {
                        if is_cursor {
                            Style::default()
                                .bg(Color::DarkGray)
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD)
                        }
                    } else if is_cursor {
                        Style::default()
                            .bg(Color::DarkGray)
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    };

                    if is_move_target {
                        style = style.bg(Color::Green).fg(Color::Black);
                    }

                    ratatui::widgets::ListItem::new(text).style(style)
                }
                ProfileListItem::Entry(fi, ei) => {
                    let file = &app.profile.files[*fi];
                    let entry = &file.entries[*ei];
                    let is_readonly = !file.writable;

                    let prefix = if is_selected { "● " } else { "  " };
                    let name_str = format!("{:<20}", entry.name);
                    let type_str = format!("{:<10}", entry.entry_type.to_string());
                    let line_str = format!("{:<10}", format_line_info(entry));
                    let value_str = format_value_display(&entry.value);
                    let tc = type_color(&entry.entry_type);

                    // Search mode styling
                    let is_search_match = app.search.as_ref().is_some_and(|s| s.is_match(*fi, *ei));
                    let is_search_selected = app
                        .search
                        .as_ref()
                        .is_some_and(|s| s.is_selected_match(*fi, *ei));

                    // Build line with per-character highlighting for search matches
                    let line = if is_readonly {
                        let grey = if is_cursor {
                            Style::default().fg(Color::Gray)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        Line::from(vec![
                            Span::raw(prefix.to_string()),
                            Span::styled(name_str, grey),
                            Span::raw(" "),
                            Span::styled(type_str, grey),
                            Span::raw(" "),
                            Span::styled(line_str, grey),
                            Span::raw(" "),
                            Span::styled(value_str, grey),
                        ])
                    } else if app.mode == AppMode::Searching && is_search_match {
                        let hl_style = Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD);
                        let char_indices = app
                            .search
                            .as_ref()
                            .and_then(|s| s.matched_char_indices(*fi, *ei, entry.name.len()));
                        let (name_hl, value_hl) = char_indices.unwrap_or_default();

                        let name_normal = Style::default().fg(Color::White);
                        let value_normal = Style::default().fg(Color::Gray);

                        let mut spans = vec![Span::raw(prefix.to_string())];
                        spans.extend(build_highlighted_spans(
                            &name_str,
                            &name_hl,
                            name_normal,
                            hl_style,
                        ));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            type_str,
                            Style::default().fg(tc).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(line_str, Style::default().fg(Color::Gray)));
                        spans.push(Span::raw(" "));
                        spans.extend(build_highlighted_spans(
                            &value_str,
                            &value_hl,
                            value_normal,
                            hl_style,
                        ));
                        Line::from(spans)
                    } else {
                        Line::from(vec![
                            Span::raw(prefix.to_string()),
                            Span::styled(name_str, Style::default().fg(Color::White)),
                            Span::raw(" "),
                            Span::styled(
                                type_str,
                                Style::default().fg(tc).add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(" "),
                            Span::styled(line_str, Style::default().fg(Color::Gray)),
                            Span::raw(" "),
                            Span::styled(value_str, Style::default().fg(Color::Gray)),
                        ])
                    };

                    let mut style = if app.mode == AppMode::Searching {
                        if is_search_selected {
                            Style::default()
                                .bg(Color::Rgb(80, 80, 0))
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else if is_search_match {
                            Style::default().bg(Color::Rgb(50, 50, 90))
                        } else {
                            Style::default().fg(Color::DarkGray)
                        }
                    } else {
                        match (is_cursor, is_selected) {
                            (true, true) => Style::default()
                                .bg(Color::Blue)
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                            (true, false) => Style::default()
                                .bg(Color::Blue)
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                            (false, true) => Style::default()
                                .bg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD),
                            (false, false) => Style::default(),
                        }
                    };

                    if is_move_target {
                        style = style.bg(Color::Green).fg(Color::Black);
                    }

                    ratatui::widgets::ListItem::new(line).style(style)
                }
            }
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::NONE));

    // Calculate scroll offset to keep cursor visible
    let visible_height = list_area.height as usize;
    let offset = if app.cursor >= visible_height {
        app.cursor - visible_height + 1
    } else {
        0
    };

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.cursor));
    *list_state.offset_mut() = offset;

    f.render_stateful_widget(list, list_area, &mut list_state);
}

fn draw_status(f: &mut Frame, area: Rect, app: &TuiApp) {
    let status = if let Some(ref msg) = app.message {
        msg.clone()
    } else {
        let total = app.profile.total_entries();
        let files = app.profile.files.len();
        if app.selection.selected_count() > 0 {
            format!(
                " {} selected | {} files, {} entries | q:quit ?:help s:select",
                app.selection.selected_count(),
                files,
                total
            )
        } else {
            format!(
                " {} files, {} entries | q:quit ?:help 0/9:collapse/expand",
                files, total
            )
        }
    };
    let text = Paragraph::new(status).style(Style::default().bg(Color::DarkGray).fg(Color::White));
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

fn draw_text_input_bar(f: &mut Frame, area: Rect, app: &TuiApp) {
    if let Some(ref input) = app.text_input {
        let text = format!("{}{}", input.prompt, input.value);
        let style = Style::default().fg(Color::Cyan);
        f.render_widget(Paragraph::new(text).style(style), area);
        // Position cursor
        let cursor_x = area.x + (input.prompt.len() + input.cursor_pos) as u16;
        f.set_cursor(cursor_x, area.y);
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
    lines.push(Line::from(Span::styled(
        "Value:",
        Style::default().fg(Color::Yellow),
    )));

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
        .title(" Entry Detail (e:edit r:remark Esc:close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let text = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(text, popup_area);
}

fn draw_help_popup(f: &mut Frame, area: Rect, _app: &TuiApp) {
    let lines = vec![
        Line::from(Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ↑/k ↓/j    Navigate up/down"),
        Line::from("  PgUp/PgDn  Jump half page"),
        Line::from("  Home/End    Jump to first/last"),
        Line::from("  Enter/Space Toggle file / View entry detail"),
        Line::from("  0/9         Collapse/Expand all files"),
        Line::from(""),
        Line::from(Span::styled(
            "Selection",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  s           Toggle select entry"),
        Line::from("  Shift+↑/↓  Range select"),
        Line::from(""),
        Line::from(Span::styled(
            "Editing",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  e           Edit (file or entry in $EDITOR)"),
        Line::from("  n           New entry via $EDITOR"),
        Line::from("  d           Delete entry / Remove file"),
        Line::from("  x           Cut selected entries"),
        Line::from("  c           Copy selected entries"),
        Line::from("  v           Paste entries"),
        Line::from("  m           Move entry/file (drag to reposition)"),
        Line::from("  z           Undo last operation"),
        Line::from("  r           Toggle remark"),
        Line::from("  a           Add file path"),
        Line::from(""),
        Line::from(Span::styled(
            "Other",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
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
