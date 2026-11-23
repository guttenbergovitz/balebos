//! Rendering for the Drift tab.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::App;

/// Render the Drift tab: coverage banner plus the findings table.
pub fn draw_drift(f: &mut Frame, app: &mut App, area: Rect) {
    if !app.drift.loaded {
        let _ = app.load_drift();
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Coverage banner
            Constraint::Min(10),   // Findings table
        ])
        .split(area);

    draw_coverage_banner(f, app, chunks[0]);
    draw_findings_table(f, app, chunks[1]);
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
