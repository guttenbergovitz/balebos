//! Runs tab — operational command centre for active scans, live discoveries,
//! and historical `ScanReport`s.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::App;

pub fn draw_runs(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(8),
            Constraint::Length(8),
        ])
        .split(area);

    let is_scanning = *app.is_scanning.lock().unwrap_or_else(|p| p.into_inner());
    let scan = app
        .scan_status
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();

    // Top: Active Run card (never fabricates a % when total_targets is None)
    let progress_line = match (is_scanning, scan.total_targets) {
        (true, Some(total)) if total > 0 => {
            let done = scan.stage_processed.min(total);
            let pct = (done * 100) / total;
            format!("{} / {} targets ({}%)", done, total, pct)
        }
        (true, _) => format!(
            "{} processed (total count pending dynamic discovery)",
            scan.stage_processed
        ),
        (false, _) if scan.run_id.is_some() => format!(
            "Last run: {} | {} processed | {} errors",
            scan.message, scan.processed_targets, scan.error_count
        ),
        (false, _) => "Idle — press 's' to start a scan".to_string(),
    };

    let active_lines = vec![
        Line::from(vec![
            Span::styled("Run: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(scan.run_id.as_deref().unwrap_or("-")),
            Span::raw("  |  "),
            Span::styled("Target: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(scan.target.as_deref().unwrap_or("-")),
            Span::raw("  |  "),
            Span::styled("Stage: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(scan.current_stage.as_deref().unwrap_or("idle")),
        ]),
        Line::from(vec![
            Span::styled("Progress: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                progress_line,
                Style::default().fg(if is_scanning {
                    Color::Magenta
                } else {
                    Color::Green
                }),
            ),
        ]),
        Line::from(format!(
            "Active: {}  |  New discoveries: {}  |  Services: {}  |  Errors: {}  |  (c: Cancel, s: New scan)",
            if scan.active_targets.is_empty() {
                "-".to_string()
            } else {
                scan.active_targets.join(", ")
            },
            scan.new_discoveries,
            scan.services_discovered,
            scan.error_count
        )),
    ];
    let active_block = Paragraph::new(active_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Active Reconnaissance "),
    );
    f.render_widget(active_block, chunks[0]);

    // Middle: Live Discoveries & Stage Steps side-by-side
    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let step_items: Vec<ListItem> = if scan.steps.is_empty() {
        vec![ListItem::new("No stage results yet")]
    } else {
        scan.steps
            .iter()
            .rev()
            .take(15)
            .map(|st| {
                let icon = if st.success { "✓" } else { "✗" };
                let color = if st.success { Color::Green } else { Color::Red };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{} {}: ", icon, st.name),
                        Style::default().fg(color),
                    ),
                    Span::raw(st.message.clone()),
                ]))
            })
            .collect()
    };
    f.render_widget(
        List::new(step_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Stage Results "),
        ),
        mid_chunks[0],
    );

    let disc_items: Vec<ListItem> = if scan.live_discoveries.is_empty() {
        vec![ListItem::new(
            "No live discoveries recorded in current session",
        )]
    } else {
        scan.live_discoveries
            .iter()
            .rev()
            .take(15)
            .map(|d| ListItem::new(d.as_str()))
            .collect()
    };
    f.render_widget(
        List::new(disc_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Live Discoveries & Events "),
        ),
        mid_chunks[1],
    );

    // Bottom: Historical ScanReports persisted in SQLite
    let history_items: Vec<ListItem> = if app.model.scan_reports.is_empty() {
        vec![ListItem::new("No persisted scan reports in storage")]
    } else {
        app.model
            .scan_reports
            .iter()
            .rev()
            .take(6)
            .map(|r| {
                let complete = if !r.outcomes.is_empty()
                    && r.outcomes.iter().all(|o| {
                        matches!(
                            o.coverage,
                            crate::domain::coverage::TargetCoverage::Observed
                        )
                    }) {
                    "complete"
                } else {
                    "incomplete"
                };
                ListItem::new(format!(
                    "{} | {} | planned={} | {}",
                    r.started_at.format("%Y-%m-%d %H:%M"),
                    r.scan_id,
                    r.planned_targets.len(),
                    complete
                ))
            })
            .collect()
    };
    f.render_widget(
        List::new(history_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Historical Scan Runs "),
        ),
        chunks[2],
    );
}
