//! Rendering for the Overview tab.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::drift::DriftClass;
use crate::tui::app::App;
use crate::tui::state::findings::FindingStatus;

/// Render the Overview tab: situation summary across coverage, assets,
/// recent changes (drift), and findings.
pub fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(4),
        ])
        .split(area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let mid_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    draw_coverage_card(f, app, top_cols[0]);
    draw_assets_card(f, app, top_cols[1]);
    draw_changes_card(f, app, mid_cols[0]);
    draw_findings_card(f, app, mid_cols[1]);
    draw_workflow_card(f, rows[2]);
}

fn draw_coverage_card(f: &mut Frame, app: &App, area: Rect) {
    let (label, style, detail) = match app.coverage_complete {
        Some(true) => (
            "✓ Complete",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "Latest scan inspected every planned target.",
        ),
        Some(false) => (
            "⚠ Incomplete",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            "Some targets were skipped or failed; absences are 'not observed', not verified removals.",
        ),
        None => (
            "○ No scan recorded",
            Style::default().fg(Color::DarkGray),
            "Run a scan (s) to establish baseline observations and coverage.",
        ),
    };

    let text = vec![
        Line::from(Span::styled(label, style)),
        Line::from(""),
        Line::from(detail),
    ];
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Coverage "));
    f.render_widget(p, area);
}

fn draw_assets_card(f: &mut Frame, app: &App, area: Rect) {
    let s = &app.model.stats;
    let text = vec![
        Line::from(vec![
            Span::raw("Total assets: "),
            Span::styled(
                s.total.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Alive HTTP (200): ", Style::default().fg(Color::Green)),
            Span::raw(s.alive.to_string()),
        ]),
        Line::from(vec![
            Span::styled("HTTP errors: ", Style::default().fg(Color::Red)),
            Span::raw(s.http_errors.to_string()),
            Span::raw("   DNS issues: "),
            Span::styled(s.dns_issues.to_string(), Style::default().fg(Color::Yellow)),
        ]),
    ];
    let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Assets "));
    f.render_widget(p, area);
}

fn draw_changes_card(f: &mut Frame, app: &App, area: Rect) {
    let total = app.drift.findings.len();
    let uncertain = app
        .drift
        .findings
        .iter()
        .filter(|d| {
            matches!(
                d.class,
                DriftClass::NotObserved | DriftClass::InsufficientEvidence
            )
        })
        .count();
    let verified = total.saturating_sub(uncertain);

    let text = vec![
        Line::from(vec![
            Span::raw(format!(
                "Window: last {} h — Total: ",
                app.drift.window_hours
            )),
            Span::styled(
                total.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Verified changes: ", Style::default().fg(Color::Green)),
            Span::raw(verified.to_string()),
        ]),
        Line::from(vec![
            Span::styled(
                "Uncertain / Not observed: ",
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(uncertain.to_string()),
        ]),
    ];
    let p = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Changes (Drift) "),
    );
    f.render_widget(p, area);
}

fn draw_findings_card(f: &mut Frame, app: &App, area: Rect) {
    let total = app.findings.all_findings.len();
    let open = app
        .findings
        .all_findings
        .iter()
        .filter(|f| app.findings.get_finding_status(&f.id) == FindingStatus::Open)
        .count();
    let ack = app
        .findings
        .all_findings
        .iter()
        .filter(|f| app.findings.get_finding_status(&f.id) == FindingStatus::Acknowledged)
        .count();

    let text = vec![
        Line::from(vec![
            Span::raw("Total findings: "),
            Span::styled(
                total.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Open: ", Style::default().fg(Color::Red)),
            Span::raw(open.to_string()),
            Span::raw("   Acknowledged: "),
            Span::styled(ack.to_string(), Style::default().fg(Color::Yellow)),
        ]),
    ];
    let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Findings "));
    f.render_widget(p, area);
}

fn draw_workflow_card(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled("Investigation flow: ", Style::default().fg(Color::Cyan)),
            Span::raw("Notice change (3: Changes) → Inspect before/after & evidence (Enter/i) → Assess coverage → Verify or rescan (s)."),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Shortcuts: 1 Overview  |  2 Assets  |  3 Changes  |  4 Findings  |  R Refresh  |  q Quit",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Workflow "));
    f.render_widget(p, area);
}
