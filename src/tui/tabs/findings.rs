//! Rendering for Findings tab.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::App;

/// Render the Findings tab content.
pub fn draw_findings(f: &mut Frame, app: &mut App, area: Rect) {
    // Load findings if not already loaded
    if app.findings.all_findings.is_empty() {
        let _ = app.load_findings();
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with filter info
            Constraint::Min(10),   // Findings table
            Constraint::Length(3), // Footer with stats
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_findings_table(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let filter_desc = app.findings.filter_description();
    let total = app.findings.all_findings.len();
    let visible = app.findings.visible_findings.len();

    let header_text = if total == visible {
        format!("Findings: {} total", total)
    } else {
        format!(
            "Findings: {} visible / {} total (filter: {})",
            visible, total, filter_desc
        )
    };

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title(" Findings "))
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(header, area);
}

fn draw_findings_table(f: &mut Frame, app: &mut App, area: Rect) {
    let findings = &app.findings.visible_findings;

    if findings.is_empty() {
        let message = if app.findings.all_findings.is_empty() {
            "No findings found. Run 'balebos finding run' to scan for issues."
        } else {
            "No findings match current filter."
        };

        let empty_msg = Paragraph::new(message)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

        f.render_widget(empty_msg, area);
        return;
    }

    // Create table rows
    let rows: Vec<Row> = findings
        .iter()
        .enumerate()
        .flat_map(|(i, finding)| {
            let _is_selected = app.findings.table_state.selected() == Some(i);
            let is_expanded = app.findings.expanded_findings.contains(&finding.id);

            // Status indicator (TODO: need to check finding events for status)
            let status_icon = "○"; // Default to open

            // Severity color
            let severity_color = match finding.severity {
                crate::app::FindingSeverity::Critical => Color::Red,
                crate::app::FindingSeverity::High => Color::LightRed,
                crate::app::FindingSeverity::Medium => Color::Yellow,
                crate::app::FindingSeverity::Low => Color::Green,
                crate::app::FindingSeverity::Info => Color::Blue,
            };

            // Main row
            let cells = vec![
                Span::styled(status_icon, Style::default().fg(Color::Cyan)),
                Span::styled(
                    finding.severity.to_string(),
                    Style::default()
                        .fg(severity_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&finding.check_name),
                Span::raw(&finding.target),
                Span::raw(finding.created_at.format("%Y-%m-%d").to_string()),
            ];

            // Expanded details row
            if is_expanded {
                let details = vec![
                    Row::new(vec![
                        Span::raw(""),
                        Span::raw(""),
                        Span::styled("Message:", Style::default().fg(Color::Cyan)),
                        Span::raw(&finding.message),
                    ]),
                    Row::new(vec![
                        Span::raw(""),
                        Span::raw(""),
                        Span::styled("Check:", Style::default().fg(Color::Cyan)),
                        Span::raw(&finding.check_name),
                    ]),
                    Row::new(vec![
                        Span::raw(""),
                        Span::raw(""),
                        Span::styled("Category:", Style::default().fg(Color::Cyan)),
                        Span::raw(finding.category.to_string()),
                    ]),
                    Row::new(vec![
                        Span::raw(""),
                        Span::raw(""),
                        Span::styled("Created:", Style::default().fg(Color::Cyan)),
                        Span::raw(finding.created_at.format("%Y-%m-%d %H:%M").to_string()),
                    ]),
                ];

                // Return main row + details rows
                return vec![Row::new(cells)].into_iter().chain(details).collect();
            }

            vec![Row::new(cells)]
        })
        .collect();

    // Table constraints
    let widths = [
        Constraint::Length(3),      // Status
        Constraint::Length(10),     // Severity
        Constraint::Percentage(30), // Title
        Constraint::Percentage(30), // Target
        Constraint::Length(12),     // Date
    ];

    let table = Table::new(rows, widths)
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut app.findings.table_state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let selected_idx = app.findings.table_state.selected();
    let total = app.findings.visible_findings.len();

    let position_info = if total > 0 {
        format!("{} of {}", selected_idx.unwrap_or(0) + 1, total)
    } else {
        "No findings".to_string()
    };

    let help_text = if app.findings.is_selected_expanded() {
        "[↑↓] Navigate [a] Acknowledge [r] Resolve [s] Scan [f] Filter [space] Collapse [q] Quit"
    } else {
        "[↑↓] Navigate [a] Acknowledge [r] Resolve [s] Scan [f] Filter [space] Expand [q] Quit"
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(position_info, Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::raw(help_text),
    ]))
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}
