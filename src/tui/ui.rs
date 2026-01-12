//! UI rendering for TUI

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{AppMode, TuiApp};
use crate::model::EntryType;

/// Draw the main UI
pub fn draw(f: &mut Frame, app: &TuiApp) {
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
        AppMode::ShowingDetail => draw_detail_popup(f, app),
        AppMode::ShowingHelp => draw_help_popup(f, app),
        AppMode::ConfirmDelete => draw_confirm_popup(f, app),
        AppMode::AddingEntry | AppMode::EditingName | AppMode::EditingValue => {
            draw_input_popup(f, app)
        }
        _ => {}
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
fn draw_content(f: &mut Frame, app: &TuiApp, area: Rect) {
    let msg = app.messages;

    // Create header line
    let header_line = Line::from(vec![
        Span::styled(
            format!("{:<10}", msg.header_type),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<20}", msg.header_name),
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
                EntryType::Function => Color::Blue,
                EntryType::EnvVar => Color::Yellow,
                EntryType::Source => Color::Magenta,
                EntryType::Code => Color::Cyan,
                EntryType::Comment => Color::White,
            };

            // Truncate long values
            let value = if entry.value.len() > 40 {
                format!("{}...", &entry.value.chars().take(37).collect::<String>())
            } else {
                entry.value.clone()
            };
            let value = value.replace('\n', "\\n");

            // Format line info
            let line_info = format_line_info(entry);

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<10}", format!("{}", entry.entry_type)),
                    Style::default().fg(type_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<20}", entry.name),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<10}", line_info),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
            ]);

            // Offset by 2 for header and separator
            let style = if i == app.selected_index {
                Style::default()
                    .bg(Color::DarkGray)
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

    // Use stateful rendering to enable scrolling
    // Offset selected index by 2 (header + separator)
    use ratatui::widgets::ListState;
    let mut state = ListState::default();
    state.select(Some(app.selected_index + 2));

    f.render_stateful_widget(list, area, &mut state);
}

/// Draw the status bar
fn draw_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let help_text = match app.mode {
        AppMode::Normal => app.messages.tui_status_normal,
        AppMode::ShowingDetail => app.messages.tui_status_detail,
        AppMode::ShowingHelp => app.messages.tui_status_help,
        AppMode::ConfirmDelete => app.messages.tui_status_confirm_delete,
        AppMode::AddingEntry | AppMode::EditingName | AppMode::EditingValue => {
            app.messages.tui_status_input
        }
        AppMode::Exiting => app.messages.tui_status_exiting,
    };

    let status_text = if let Some(ref msg) = app.message {
        format!("{} | {}", msg, help_text)
    } else {
        help_text.to_string()
    };

    let status_paragraph = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center);

    f.render_widget(status_paragraph, area);
}

/// Draw detail popup
/// Unified format for all entry types: Type, Name, Line(s), Value
fn draw_detail_popup(f: &mut Frame, app: &TuiApp) {
    if let Some(entry) = app.get_selected_entry() {
        let area = centered_rect(70, 60, f.size());
        let msg = app.messages;

        // Format line info using helper function
        let line_info = format_line_info(entry);
        let is_multi_line = entry
            .end_line
            .is_some_and(|end| entry.line_number.is_some_and(|start| end > start));
        let line_label = if is_multi_line {
            msg.header_lines
        } else {
            msg.header_line
        };

        // Create detail text - unified for all entry types
        // Format: Type, Name, Line(s), Value (no Comment field)
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Type: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", entry.entry_type)),
            ]),
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Cyan)),
                Span::raw(&entry.name),
            ]),
            Line::from(vec![
                Span::styled(line_label, Style::default().fg(Color::Cyan)),
                Span::raw(line_info),
            ]),
        ];

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Value:",
            Style::default().fg(Color::Cyan),
        )]));

        for value_line in entry.value.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", value_line),
                Style::default().fg(Color::Gray),
            )));
        }

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(app.messages.tui_entry_details_title)
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black)),
            )
            .wrap(Wrap { trim: false });

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
    }
}

/// Draw help popup
fn draw_help_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(60, 50, f.size());
    let msg = app.messages;

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("↑/↓, k/j  ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_navigate),
        ]),
        Line::from(vec![
            Span::styled("i         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_info),
        ]),
        Line::from(vec![
            Span::styled("d         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_delete),
        ]),
        Line::from(vec![
            Span::styled("n         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_new),
        ]),
        Line::from(vec![
            Span::styled("r         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_rename),
        ]),
        Line::from(vec![
            Span::styled("e         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_edit),
        ]),
        Line::from(vec![
            Span::styled("c         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_check),
        ]),
        Line::from(vec![
            Span::styled("f         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_format),
        ]),
        Line::from(vec![
            Span::styled("?         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_help),
        ]),
        Line::from(vec![
            Span::styled("q         ", Style::default().fg(Color::Yellow)),
            Span::raw(msg.tui_help_quit),
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

/// Draw confirm delete popup
fn draw_confirm_popup(f: &mut Frame, app: &TuiApp) {
    if let Some(entry) = app.get_selected_entry() {
        let area = centered_rect(50, 20, f.size());
        let msg = app.messages;

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                msg.tui_delete_prompt,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Type: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", entry.entry_type)),
            ]),
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Cyan)),
                Span::raw(&entry.name),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                msg.tui_yes_no,
                Style::default().fg(Color::Yellow),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(msg.tui_confirm_delete_title)
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black).fg(Color::Red)),
            )
            .alignment(Alignment::Center);

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
    }
}

/// Draw input popup for adding/editing
fn draw_input_popup(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(60, 30, f.size());
    let msg = app.messages;

    let title = match app.mode {
        AppMode::AddingEntry => msg.tui_add_entry_title,
        AppMode::EditingName => msg.tui_edit_name_title,
        AppMode::EditingValue => msg.tui_edit_value_title,
        _ => msg.tui_input_title,
    };

    let prompt = if let Some(ref msg_text) = app.message {
        msg_text.as_str()
    } else {
        "Enter value:"
    };

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            prompt,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            &app.input_buffer,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            msg.tui_enter_submit_esc_cancel,
            Style::default().fg(Color::Gray),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(Alignment::Left);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
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
