//! UI rendering for TUI

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{AppMode, EditField, TuiApp};
use crate::model::EntryType;

/// Header row count in the entry list (header + separator)
const LIST_HEADER_OFFSET: usize = 2;

/// Draw the main UI
pub fn draw(f: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status/Help
        ])
        .split(f.size());

    // Draw title
    draw_title(f, app, chunks[0]);

    // Draw content
    draw_content(f, app, chunks[1]);

    // Draw status bar
    draw_status_bar(f, app, chunks[2]);

    // Draw popups based on mode
    match app.mode {
        AppMode::Searching => draw_search_popup(f, app),
        AppMode::ShowingDetail => draw_detail_popup(f, app),
        AppMode::ShowingHelp => draw_help_popup(f, app),
        AppMode::ConfirmDelete => draw_confirm_popup(f, app),
        AppMode::ConfirmQuit => draw_confirm_quit_popup(f, app),
        AppMode::ConfirmFormat => draw_format_preview_popup(f, app),
        AppMode::ConfirmSaveWithErrors => draw_validation_error_popup(f, app),
        AppMode::SelectingType => draw_type_selection_popup(f, app),
        AppMode::Editing => draw_edit_popup(f, app),
        AppMode::Moving => {} // No popup, just show indicator in status bar
        AppMode::Normal => {}
    }
}

/// Draw the title bar
fn draw_title(f: &mut Frame, app: &TuiApp, area: Rect) {
    let title = app
        .messages
        .tui_title
        .replace("{}", &app.file_path.display().to_string());
    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Blue).fg(Color::White));

    let title_paragraph = Paragraph::new(title)
        .block(title_block)
        .alignment(Alignment::Center);

    f.render_widget(title_paragraph, area);
}

/// Format line number display (single line or range)
fn format_line_info(entry: &crate::model::Entry) -> String {
    match (entry.line_number, entry.end_line) {
        (Some(start), Some(end)) if end > start => format!("{}-{}", start, end),
        (Some(line), _) => format!("{}", line),
        (None, _) => "-".to_string(),
    }
}

/// Draw the main content (entry list)
fn draw_content(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    // Update visible height for scroll calculations (used by keyboard navigation)
    app.list_visible_height = area.height as usize;

    let msg = app.messages;

    // Create header line
    let header_line = Line::from(vec![
        Span::styled(
            format!("{:<20}", msg.header_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<10}", msg.header_type),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<10}", msg.header_line_num),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            msg.header_value,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let header_item = ListItem::new(header_line);

    // Create separator line
    let separator_line = Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(Color::DarkGray),
    ));
    let separator_item = ListItem::new(separator_line);

    // Create entry items
    let entry_items: Vec<ListItem> = app
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let type_color = match entry.entry_type {
                EntryType::Alias => Color::Green,
                EntryType::Function => Color::LightBlue,
                EntryType::EnvVar => Color::Yellow,
                EntryType::Source => Color::Magenta,
                EntryType::Code => Color::Cyan,
                EntryType::Comment => Color::White,
            };

            // Truncate long values
            let value = if entry.value.len() > 100 {
                format!("{}...", &entry.value.chars().take(97).collect::<String>())
            } else {
                entry.value.clone()
            };
            let value = value.replace('\n', "\\n");

            // Format line info
            let line_info = format_line_info(entry);

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<20}", entry.name),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<10}", format!("{}", entry.entry_type)),
                    Style::default().fg(type_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<10}", line_info),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
            ]);

            // Offset by 2 for header and separator
            // Check if this item is in the selected indices set
            let is_selected = app.selected_indices.contains(&i);

            // Check if this item is a search match
            let is_search_match = app.search_active && app.search_matches.contains(&i);

            let style = if i == app.selected_index {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                // Multi-selection (both continuous and non-contiguous)
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if is_search_match {
                // Highlight search matches with different background
                Style::default()
                    .bg(Color::Rgb(40, 40, 0)) // Dark yellow background
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    // Combine header, separator, and entries
    let mut all_items = vec![header_item, separator_item];
    all_items.extend(entry_items);

    let list = List::new(all_items).block(
        Block::default().borders(Borders::ALL).title(
            app.messages
                .tui_entries
                .replace("{}", &app.entries.len().to_string()),
        ),
    );

    // Manual scroll control - only scroll when selection reaches edge
    // Use ListState with offset to control what's visible
    use ratatui::widgets::ListState;
    let mut state = ListState::default();

    // The list items are: header(0), separator(1), then entries starting at LIST_HEADER_OFFSET
    // We want to select the entry at app.selected_index, which is at list index (selected_index + LIST_HEADER_OFFSET)
    // But we also need to scroll the list so that the header stays visible when at top,
    // and entries scroll when we go down

    // When entries fit in visible area (no scrolling needed), offset should be 0
    // When scrolling is needed, offset should skip entries but always show header+separator

    // The actual item index in the list for the selected entry
    let selected_list_index = app.selected_index + LIST_HEADER_OFFSET;

    // Set the selected index
    state.select(Some(selected_list_index));

    // Set the scroll offset - this controls which items are visible
    // We want to keep header (0) and separator (1) visible, so offset should account for that
    // When list_scroll_offset is 0, we show from the beginning (header visible)
    // When list_scroll_offset is N, we want to scroll so that entry N is at the top
    // But we can't scroll past the header, so the minimum visible should be header+separator

    // Actually, we don't want to scroll the header off screen
    // The offset in ListState is absolute - it determines which item is at the top
    // If we set offset to 0, header is at top
    // If we set offset to 2, first entry is at top (header scrolled off)
    // We always want header visible, so:
    // - When scroll_offset is 0: offset = 0 (show header at top)
    // - When scroll_offset is N: offset = 0, but we need to handle this differently

    // The issue is that ListState.offset scrolls the entire list including header
    // We actually need a different approach: always show header, and only scroll entries

    // Simple fix: Don't use offset for scroll, let ratatui handle it naturally
    // The select() will auto-scroll to keep selected item visible

    f.render_stateful_widget(list, area, &mut state);
}

/// Draw the status bar
fn draw_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let msg = &app.messages;
    let help_text = match app.mode {
        AppMode::Normal => msg.tui_status_normal,
        AppMode::Searching => msg.tui_status_searching,
        AppMode::ShowingDetail => msg.tui_status_detail_extended,
        AppMode::ShowingHelp => msg.tui_status_help,
        AppMode::ConfirmDelete => msg.tui_status_confirm_delete_extended,
        AppMode::ConfirmQuit => msg.tui_status_confirm_quit,
        AppMode::ConfirmFormat => msg.tui_status_confirm_format,
        AppMode::ConfirmSaveWithErrors => msg.tui_status_confirm_save_errors,
        AppMode::SelectingType => msg.tui_status_selecting_type,
        AppMode::Editing => msg.tui_status_editing,
        AppMode::Moving => msg.tui_status_moving,
    };

    // Build status text with dirty indicator
    let dirty_indicator = if app.dirty { "[*] " } else { "" };

    // Add search info if search is active
    let search_info = if app.search_active && app.mode == AppMode::Normal {
        if !app.search_query.is_empty() {
            format!(
                "[Search: \"{}\" ({} matches)] ",
                app.search_query,
                app.search_matches.len()
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let status_text = if let Some(ref msg) = app.message {
        format!("{}{}{} | {}", dirty_indicator, search_info, msg, help_text)
    } else {
        format!("{}{}{}", dirty_indicator, search_info, help_text)
    };

    let status_style = if app.dirty {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let status_paragraph = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL))
        .style(status_style)
        .alignment(Alignment::Center);

    f.render_widget(status_paragraph, area);
}

/// Draw detail popup
/// Draw search popup
fn draw_search_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(60, 20, f.size());

    // Build search input with cursor
    let search_display = if app.search_cursor <= app.search_query.len() {
        let pos = app.search_cursor.min(app.search_query.len());
        let safe_pos = find_char_boundary(&app.search_query, pos);
        let (before, after) = app.search_query.split_at(safe_pos);
        format!("{}│{}", before, after)
    } else {
        app.search_query.clone()
    };

    let match_count = app.search_matches.len();
    let msg = &app.messages;
    let match_info = if match_count > 0 {
        msg.tui_search_matches
            .replace("{}", &match_count.to_string())
    } else {
        msg.tui_search_no_matches.to_string()
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_search_title.trim(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(msg.tui_search_query, Style::default().fg(Color::Yellow)),
            Span::styled(&search_display, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            &match_info,
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_search_hint,
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(msg.tui_search_title)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(Alignment::Center);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Detail popup
/// Unified format: Type, Line(s) → Name, Value
/// Fixed footer for hints
fn draw_detail_popup(f: &mut Frame, app: &mut TuiApp) {
    // Clone entry data to avoid borrow issues
    let entry = match app.get_selected_entry() {
        Some(e) => e.clone(),
        None => return,
    };

    let area = centered_rect(70, 60, f.size());
    let msg = app.messages;

    // Split area: main content + fixed footer (3 lines for hints)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Content area (scrollable)
            Constraint::Length(3), // Fixed footer for hints
        ])
        .split(area);

    let content_area = chunks[0];
    let footer_area = chunks[1];

    // Format line info using helper function
    let line_info = format_line_info(&entry);
    let is_multi_line = entry
        .end_line
        .is_some_and(|end| entry.line_number.is_some_and(|start| end > start));
    let line_label = if is_multi_line {
        msg.header_lines
    } else {
        msg.header_line
    };

    // Create detail text - new order: Type, Line(s), blank, Name, Value
    let msg = &app.messages;
    let mut lines = vec![
        Line::from(vec![
            Span::styled(msg.label_type, Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}", entry.entry_type)),
        ]),
        Line::from(vec![
            Span::styled(line_label, Style::default().fg(Color::Cyan)),
            Span::raw(line_info),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(msg.label_name, Style::default().fg(Color::Cyan)),
            Span::raw(&entry.name),
        ]),
        Line::from(vec![Span::styled(
            msg.label_value,
            Style::default().fg(Color::Cyan),
        )]),
    ];

    // Use value for Comment/Code to show full content
    let display_value = match entry.entry_type {
        EntryType::Comment | EntryType::Code => &entry.value,
        _ => &entry.value,
    };
    for value_line in display_value.lines() {
        lines.push(Line::from(Span::styled(
            value_line.to_string(),
            Style::default().fg(Color::Gray),
        )));
    }

    // Calculate visible area height (subtract 2 for borders)
    let visible_height = content_area.height.saturating_sub(2) as usize;
    let total_lines = lines.len();

    // Clamp scroll offset - only allow scrolling if content exceeds visible area
    let max_scroll = total_lines.saturating_sub(visible_height);
    if app.detail_scroll > max_scroll {
        app.detail_scroll = max_scroll;
    }

    let scroll_indicator = if total_lines > visible_height {
        format!(
            "{} ({}/{})",
            msg.tui_entry_details_title,
            app.detail_scroll + 1,
            max_scroll + 1
        )
    } else {
        msg.tui_entry_details_title.to_string()
    };

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(scroll_indicator)
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Black)),
        )
        .scroll((app.detail_scroll as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, content_area);

    // Draw fixed footer with hints - text on the line just above the bottom border
    let footer_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_detail_hint,
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let footer = Paragraph::new(footer_lines)
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}

/// Draw help popup
fn draw_help_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(60, 60, f.size());
    let msg = app.messages;

    let help_text = vec![
        Line::from(Span::styled(
            msg.tui_help_keyboard_shortcuts,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("↑/↓, k/j  ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_nav_updown),
        ]),
        Line::from(vec![
            Span::styled("Scroll    ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_nav_scroll),
        ]),
        Line::from(vec![
            Span::styled("i, Enter  ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_info_detail),
        ]),
        Line::from(vec![
            Span::styled("a         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_add),
        ]),
        Line::from(vec![
            Span::styled("e         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_edit_entry),
        ]),
        Line::from(vec![
            Span::styled("m         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_move),
        ]),
        Line::from(vec![
            Span::styled("d, Del    ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_delete),
        ]),
        Line::from(vec![
            Span::styled("t         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_toggle_select),
        ]),
        Line::from(vec![
            Span::styled("f         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_search),
        ]),
        Line::from(vec![
            Span::styled("c         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_check),
        ]),
        Line::from(vec![
            Span::styled("r         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_format_file),
        ]),
        Line::from(vec![
            Span::styled("s, Ctrl+S ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_save),
        ]),
        Line::from(vec![
            Span::styled("Ctrl/Alt+C", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_copy),
        ]),
        Line::from(vec![
            Span::styled("Ctrl/Alt+V", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_paste),
        ]),
        Line::from(vec![
            Span::styled("Shift+↑/↓ ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_select_range),
        ]),
        Line::from(vec![
            Span::styled("?         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_help_key),
        ]),
        Line::from(vec![
            Span::styled("q, Esc    ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_quit),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_help_edit_mode,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("Tab       ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_next_field),
        ]),
        Line::from(vec![
            Span::styled("Shift+Tab ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_prev_field),
        ]),
        Line::from(vec![
            Span::styled("Enter     ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_submit_on_button),
        ]),
        Line::from(vec![
            Span::styled("Esc       ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_cancel),
        ]),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(msg.tui_help_title)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Draw confirm delete popup with full entry details
/// Single entry: shows Type, Line, Name, Value (scrollable like detail popup)
/// Multi-select: shows summary list of all selected entries
fn draw_confirm_popup(f: &mut Frame, app: &mut TuiApp) {
    let msg = app.messages;
    // Clone entries to avoid borrow issues
    let selected_entries: Vec<_> = app.get_selected_entries().into_iter().cloned().collect();
    let count = selected_entries.len();

    if count == 0 {
        return;
    }

    // Use larger area for better readability (same as detail popup)
    let area = centered_rect(70, 60, f.size());

    // Split area: main content + fixed footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Content area (scrollable)
            Constraint::Length(3), // Fixed footer for hints
        ])
        .split(area);

    let content_area = chunks[0];
    let footer_area = chunks[1];

    // Build content lines based on single/multi selection
    let mut lines: Vec<Line> = Vec::new();

    if count == 1 {
        // Single entry: show full details like detail popup
        let entry = &selected_entries[0];
        let line_info = format_line_info(entry);
        let is_multi_line = entry
            .end_line
            .is_some_and(|end| entry.line_number.is_some_and(|start| end > start));
        let line_label = if is_multi_line {
            msg.header_lines
        } else {
            msg.header_line
        };

        lines.push(Line::from(Span::styled(
            msg.tui_delete_prompt,
            Style::default().add_modifier(Modifier::BOLD).fg(Color::Red),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(msg.label_type, Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}", entry.entry_type)),
        ]));
        lines.push(Line::from(vec![
            Span::styled(line_label, Style::default().fg(Color::Cyan)),
            Span::raw(line_info),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(msg.label_name, Style::default().fg(Color::Cyan)),
            Span::raw(&entry.name),
        ]));
        lines.push(Line::from(vec![Span::styled(
            msg.label_value,
            Style::default().fg(Color::Cyan),
        )]));

        // Add value lines - use value for Comment/Code
        let display_value = match entry.entry_type {
            EntryType::Comment | EntryType::Code => &entry.value,
            _ => &entry.value,
        };
        for value_line in display_value.lines() {
            lines.push(Line::from(Span::styled(
                value_line.to_string(),
                Style::default().fg(Color::Gray),
            )));
        }
    } else {
        // Multi-select: show summary list
        lines.push(Line::from(Span::styled(
            msg.tui_delete_multi_prompt
                .replace("{}", &count.to_string()),
            Style::default().add_modifier(Modifier::BOLD).fg(Color::Red),
        )));
        lines.push(Line::from(""));

        // Header for the list: TYPE NAME LINE VALUE (matches main list format)
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<20}", msg.header_name),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{:<10}", msg.header_type),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{:<8}", msg.header_line),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(msg.header_value, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from("─".repeat(70)));

        // List each entry
        for entry in &selected_entries {
            let line_info = format_line_info(entry);
            let type_str = format!("{}", entry.entry_type);
            // Truncate name if too long
            let name_display = if entry.name.len() > 17 {
                format!("{}...", &entry.name[..14])
            } else {
                entry.name.clone()
            };

            // Get display value with truncation - use value for Comment/Code
            let display_value = match entry.entry_type {
                EntryType::Comment | EntryType::Code => &entry.value,
                _ => &entry.value,
            };
            // Truncate and replace newlines for display
            let value_truncated: String = display_value.chars().take(27).collect();
            let value_display = if display_value.len() > 27 {
                format!("{}...", value_truncated.replace('\n', "\\n"))
            } else {
                value_truncated.replace('\n', "\\n")
            };

            lines.push(Line::from(vec![
                Span::raw(format!("{:<20}", name_display)),
                Span::raw(format!("{:<10}", type_str)),
                Span::styled(
                    format!("{:<8}", line_info),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(value_display, Style::default().fg(Color::Gray)),
            ]));
        }
    }

    // Calculate scroll limits
    let visible_height = content_area.height.saturating_sub(2) as usize;
    let total_lines = lines.len();
    let max_scroll = total_lines.saturating_sub(visible_height);

    // Clamp scroll offset
    if app.delete_confirm_scroll > max_scroll {
        app.delete_confirm_scroll = max_scroll;
    }

    // Build scroll indicator for title
    let title = if total_lines > visible_height {
        format!(
            "{} ({}/{})",
            msg.tui_confirm_delete_title,
            app.delete_confirm_scroll + 1,
            max_scroll + 1
        )
    } else {
        msg.tui_confirm_delete_title.to_string()
    };

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Black).fg(Color::Red)),
        )
        .scroll((app.delete_confirm_scroll as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, content_area);

    // Draw fixed footer with confirmation hints
    let footer_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_confirm_delete_hint,
            Style::default().fg(Color::Yellow),
        )),
    ];
    let footer = Paragraph::new(footer_lines)
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Black).fg(Color::Red)),
        )
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}

/// Draw confirm quit popup (unsaved changes)
fn draw_confirm_quit_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(50, 20, f.size());

    let msg = &app.messages;
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_confirm_quit_msg,
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(msg.tui_confirm_quit_question),
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_confirm_quit_hint,
            Style::default().fg(Color::Cyan),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(msg.tui_confirm_quit_title)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::Yellow)),
        )
        .alignment(Alignment::Center);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Draw format preview popup
fn draw_format_preview_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(80, 70, f.size());

    if let Some(ref preview) = app.format_preview {
        // Calculate available height for content
        let available_height = area.height.saturating_sub(4) as usize; // borders + title + footer

        // Build lines with scroll
        let total_lines = preview.summary.len();
        let scroll_offset = preview.scroll_offset.min(total_lines.saturating_sub(1));
        let end_line = (scroll_offset + available_height).min(total_lines);

        let mut lines: Vec<Line> = preview.summary[scroll_offset..end_line]
            .iter()
            .map(|s| Line::from(s.as_str()))
            .collect();

        // Add scroll indicator if needed
        if total_lines > available_height {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "[Showing {}-{} of {} lines]",
                    scroll_offset + 1,
                    end_line,
                    total_lines
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(app.messages.tui_format_preview_title)
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black).fg(Color::White)),
            )
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
    }
}

/// Draw validation error popup
fn draw_validation_error_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(80, 70, f.size());

    if let Some(ref error_msg) = app.validation_errors {
        // Calculate available height for content
        let available_height = area.height.saturating_sub(5) as usize; // borders + title + footer

        // Split error message into lines
        let error_lines: Vec<&str> = error_msg.lines().collect();
        let total_lines = error_lines.len();
        let scroll_offset = app
            .validation_scroll_offset
            .min(total_lines.saturating_sub(1));
        let end_line = (scroll_offset + available_height).min(total_lines);

        let mut lines: Vec<Line> = error_lines[scroll_offset..end_line]
            .iter()
            .map(|s| Line::from(*s))
            .collect();

        // Add scroll indicator if needed
        if total_lines > available_height {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "[Showing {}-{} of {} lines]",
                    scroll_offset + 1,
                    end_line,
                    total_lines
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }

        // Add footer with instructions
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            app.messages.tui_validation_error_hint,
            Style::default().fg(Color::Yellow),
        )));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(app.messages.tui_validation_error_title)
                    .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black).fg(Color::Red)),
            )
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
    }
}

/// Draw type selection popup for Add
fn draw_type_selection_popup(f: &mut Frame, app: &mut TuiApp) {
    let area = centered_rect(40, 40, f.size());

    let msg = &app.messages;
    let types = [
        ("1", "Alias", msg.tui_type_alias_desc),
        ("2", "Function", msg.tui_type_func_desc),
        ("3", "EnvVar", msg.tui_type_env_desc),
        ("4", "Source", msg.tui_type_source_desc),
        ("5", "Code/Comment", msg.tui_type_code_desc),
    ];

    // Calculate available height for type list
    // area.height includes borders (top=1, bottom=1)
    // We need space for: title line (1), blank line (1), footer blank (1), footer hint (1)
    // So available for items = area.height - 2 (borders) - 4 (header/footer) = area.height - 6
    let available_height = area.height.saturating_sub(6).max(1) as usize;
    let total_items = types.len();

    // Adjust scroll offset to keep selected item visible
    if app.type_selection_index < app.type_list_scroll_offset {
        app.type_list_scroll_offset = app.type_selection_index;
    } else if app.type_selection_index >= app.type_list_scroll_offset + available_height {
        app.type_list_scroll_offset = app.type_selection_index + 1 - available_height;
    }

    // Build header
    let mut lines = vec![
        Line::from(Span::styled(
            msg.tui_add_entry_title.trim(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Show scroll indicator if needed
    let show_scroll_up = app.type_list_scroll_offset > 0;
    let show_scroll_down = app.type_list_scroll_offset + available_height < total_items;

    if show_scroll_up {
        lines.push(Line::from(Span::styled(
            "          ▲ More above ▲",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Render visible items
    let visible_start = app.type_list_scroll_offset;
    let visible_end = (app.type_list_scroll_offset + available_height).min(total_items);

    for (idx, &(key, name, desc)) in types
        .iter()
        .enumerate()
        .skip(visible_start)
        .take(visible_end - visible_start)
    {
        let style = if idx == app.type_selection_index {
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(format!(" [{}] ", key), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{:<10}", name), style),
            Span::styled(format!(" - {}", desc), Style::default().fg(Color::DarkGray)),
        ]));
    }

    if show_scroll_down {
        lines.push(Line::from(Span::styled(
            "          ▼ More below ▼",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        msg.tui_type_select_hint,
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(msg.tui_add_entry_title)
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Draw unified edit popup
/// Fixed footer for Submit button and hints
fn draw_edit_popup(f: &mut Frame, app: &mut TuiApp) {
    let Some(ref mut state) = app.edit_state else {
        return;
    };

    let area = centered_rect(70, 60, f.size());

    let msg = &app.messages;
    let title = if state.is_new {
        msg.tui_add_entry_title
    } else {
        msg.tui_edit_value_title
    };

    // All entry types skip the Name field since value contains complete syntax
    let skip_name = true;

    // Split area: main content + fixed footer (5 lines for submit + hints)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Content area (scrollable value)
            Constraint::Length(5), // Fixed footer: submit button + hints
        ])
        .split(area);

    let content_area = chunks[0];
    let footer_area = chunks[1];

    // Styles for focused/unfocused fields
    let focused_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let unfocused_style = Style::default().fg(Color::White);
    let label_style = Style::default().fg(Color::Cyan);
    let submit_focused = Style::default()
        .bg(Color::Green)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let submit_unfocused = Style::default().bg(Color::DarkGray).fg(Color::White);

    let name_style = if state.field == EditField::Name {
        focused_style
    } else {
        unfocused_style
    };
    let value_style = if state.field == EditField::Value {
        focused_style
    } else {
        unfocused_style
    };
    let submit_style = if state.field == EditField::Submit {
        submit_focused
    } else {
        submit_unfocused
    };

    // Build name display with cursor
    let name_display = if state.field == EditField::Name {
        // Ensure cursor position is at a valid char boundary
        let pos = state.cursor_position.min(state.name_buffer.len());
        let safe_pos = find_char_boundary(&state.name_buffer, pos);
        let (before, after) = state.name_buffer.split_at(safe_pos);
        format!("{}│{}", before, after)
    } else {
        state.name_buffer.clone()
    };

    // Build content lines (Type, Name, Value header)
    let mut lines = vec![
        Line::from(vec![
            Span::styled(msg.label_type, label_style),
            Span::raw(format!("{}", state.entry_type)),
        ]),
        Line::from(""),
    ];

    // Only show Name field for types that need it (Alias, Function, EnvVar)
    if !skip_name {
        lines.push(Line::from(vec![Span::styled(msg.label_name, label_style)]));
        lines.push(Line::from(vec![
            Span::styled(
                if state.field == EditField::Name {
                    "▸ "
                } else {
                    "  "
                },
                if state.field == EditField::Name {
                    focused_style
                } else {
                    unfocused_style
                },
            ),
            Span::styled(&name_display, name_style),
        ]));
        lines.push(Line::from(""));
    }

    // Value label with hint for Source type
    let value_label = if state.entry_type == EntryType::Source {
        "Value: (single-line, Enter to submit)"
    } else {
        msg.label_value
    };
    lines.push(Line::from(vec![Span::styled(value_label, label_style)]));

    // Build value lines with cursor on the correct row
    let value_lines: Vec<&str> = if state.value_buffer.is_empty() {
        vec![""]
    } else {
        // Use split('\n') to preserve trailing empty lines (separator format)
        state.value_buffer.split('\n').collect()
    };

    // Handle trailing newline
    let has_trailing_newline = state.value_buffer.ends_with('\n');
    let total_value_lines = if has_trailing_newline {
        value_lines.len() + 1
    } else {
        value_lines.len().max(1)
    };

    for i in 0..total_value_lines {
        let line_content = value_lines.get(i).copied().unwrap_or("");
        let is_cursor_line = state.field == EditField::Value && i == state.cursor_row;

        let display_line = if is_cursor_line {
            // Insert cursor at correct column position
            // Ensure we split at a valid character boundary
            let col = state.cursor_col.min(line_content.len());
            let safe_col = find_char_boundary(line_content, col);
            let (before, after) = line_content.split_at(safe_col);
            format!("{}│{}", before, after)
        } else {
            line_content.to_string()
        };

        let prefix = if state.field == EditField::Value && i == 0 {
            "▸ "
        } else {
            "  "
        };

        lines.push(Line::from(vec![
            Span::styled(
                prefix,
                if state.field == EditField::Value {
                    focused_style
                } else {
                    unfocused_style
                },
            ),
            Span::styled(display_line, value_style),
        ]));
    }

    // Calculate scroll offset for value field
    // Header lines: depends on whether Name is shown
    let header_lines = if skip_name { 3 } else { 6 }; // Type(1) + blank(1) + [Name label(1) + Name value(1) + blank(1)] + Value label(1)
    let visible_height = content_area.height.saturating_sub(2) as usize; // minus borders
    let total_lines = lines.len();

    // Calculate scroll: only scroll when cursor reaches edge
    let scroll_offset = if state.field == EditField::Value && total_lines > visible_height {
        // The cursor line in the full content
        let cursor_line_pos = header_lines + state.cursor_row;

        // Current visible range
        let visible_start = state.scroll_offset;
        let visible_end = visible_start + visible_height;

        // Only scroll when cursor hits the exact boundary
        if cursor_line_pos < visible_start {
            // Cursor above visible area - scroll up to show cursor at top
            cursor_line_pos
        } else if cursor_line_pos >= visible_end {
            // Cursor below visible area - scroll down to show cursor at bottom
            cursor_line_pos.saturating_sub(visible_height) + 1
        } else {
            // Cursor is within visible area - keep current scroll
            state.scroll_offset
        }
    } else if state.field == EditField::Name {
        0 // Name field - show from top
    } else {
        state.scroll_offset
    };

    // Update scroll_offset in state for next frame
    state.scroll_offset = scroll_offset.min(total_lines.saturating_sub(visible_height));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Black)),
        )
        .scroll((state.scroll_offset as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, content_area);

    // Draw fixed footer with Submit button and hints
    let submit_text = if state.field == EditField::Submit {
        msg.tui_submit_button_focused
    } else {
        msg.tui_submit_button
    };

    // Different hint for Source type
    let hint_text = if state.entry_type == EntryType::Source {
        msg.tui_edit_hint_source
    } else {
        msg.tui_edit_hint
    };

    let footer_lines = vec![
        Line::from(""),
        Line::from(Span::styled(submit_text, submit_style)),
        Line::from(""),
        Line::from(Span::styled(
            hint_text,
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let footer = Paragraph::new(footer_lines)
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Find a valid character boundary at or before the given byte position
/// This ensures safe string slicing by backing up to a valid boundary
fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    if s.is_char_boundary(pos) {
        return pos;
    }
    // Back up to find a valid boundary
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}
