//! Rendering for the Drift tab.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::tui::app::App;

/// Render the Drift tab: coverage banner, findings table, and selected
/// change investigation panel (before/after + evidence).
pub fn draw_drift(f: &mut Frame, app: &mut App, area: Rect) {
    if !app.drift.loaded {
        if let Err(e) = app.load_drift() {
            app.set_status(format!("❌ Failed to load drift: {e}"));
        }
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Coverage banner
            Constraint::Min(8),    // Findings table
            Constraint::Length(9), // Selected change & evidence
        ])
        .split(area);

    draw_coverage_banner(f, app, chunks[0]);
    draw_findings_table(f, app, chunks[1]);
    draw_change_details(f, app, chunks[2]);
}

fn draw_coverage_banner(f: &mut Frame, app: &App, area: Rect) {
    let (symbol, style, text) = if app.drift.coverage_complete {
        (
            "✓",
            Style::default().fg(Color::Green),
            format!(
                "Coverage: complete (latest scan fully observed) — window: last {} h",
                app.drift.window_hours
            ),
        )
    } else {
        (
            "⚠",
            Style::default().fg(Color::Yellow),
            format!(
                "Coverage: incomplete — absences are 'not observed', not verified change (window: last {} h)",
                app.drift.window_hours
            ),
        )
    };

    let banner = Paragraph::new(format!("{} {}", symbol, text))
        .style(style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Drift coverage "),
        );
    f.render_widget(banner, area);
}

fn draw_findings_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec!["Asset", "Class", "Evidence"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .drift
        .findings
        .iter()
        .map(|finding| {
            let class_style = match finding.class {
                crate::drift::DriftClass::VerifiedDisappearance
                | crate::drift::DriftClass::InspectionFailure => Style::default().fg(Color::Red),
                crate::drift::DriftClass::NotObserved
                | crate::drift::DriftClass::InsufficientEvidence => {
                    Style::default().fg(Color::Yellow)
                }
                _ => Style::default().fg(Color::Green),
            };
            Row::new(vec![
                Cell::from(finding.asset.clone()),
                Cell::from(format!("{:?}", finding.class)).style(class_style),
                Cell::from(finding.detail.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(22),
            Constraint::Percentage(48),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Drift findings ({}) ", app.drift.findings.len())),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, &mut app.drift.table_state);
}

fn draw_change_details(f: &mut Frame, app: &App, area: Rect) {
    let lines = match app.drift.selected_finding() {
        Some((finding, evidence)) => {
            let coverage_label = if app.drift.coverage_complete {
                "complete"
            } else {
                "incomplete"
            };
            let (prev, curr, meta) = match evidence {
                Some(ev) => {
                    let observed = ev
                        .last_observed
                        .map(|ts| ts.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let sources = if ev.sources.is_empty() {
                        "none".to_string()
                    } else {
                        ev.sources.join(", ")
                    };
                    let scan = ev.latest_scan_id.as_deref().unwrap_or("none");
                    (
                        ev.previous_summary.clone(),
                        ev.current_summary.clone(),
                        format!("Observed: {observed}  |  Sources: {sources}  |  Scan: {scan}"),
                    )
                }
                None => (
                    "unavailable".to_string(),
                    "unavailable".to_string(),
                    "No additional context".to_string(),
                ),
            };
            vec![
                Line::from(vec![
                    Span::styled("Selected: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        &finding.asset,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        "  |  Class: {:?}  |  Coverage: {}",
                        finding.class, coverage_label
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Previous: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(prev),
                ]),
                Line::from(vec![
                    Span::styled("Current:  ", Style::default().fg(Color::Green)),
                    Span::raw(curr),
                ]),
                Line::from(vec![
                    Span::styled("Evidence: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&finding.detail),
                ]),
                Line::from(Span::styled(meta, Style::default().fg(Color::DarkGray))),
            ]
        }
        None => vec![Line::from(Span::styled(
            "No drift findings in the selected window.",
            Style::default().fg(Color::DarkGray),
        ))],
    };

    let details = Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Change & Evidence "),
    );
    f.render_widget(details, area);
}
