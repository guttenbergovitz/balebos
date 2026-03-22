//! Main UI rendering module.
//!
//! This module handles the overall layout and delegates tab-specific
//! rendering to the `tabs` submodules.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame,
};

use crate::tui::app::{App, InputMode, OrgAddType, Tab};
use crate::tui::tabs::{draw_inventory, draw_organization, draw_projects};

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &mut App) {
    let show_filter = app.input_mode == InputMode::Filtering
        || !app.filter.is_empty()
        || app.hide_external_redirects;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                                // Header/Stats
            Constraint::Length(3),                                // Tab bar
            Constraint::Length(if show_filter { 3 } else { 0 }),  // Filter Bar (conditional)
            Constraint::Min(10),                                  // Main View
            Constraint::Length(3),                                // Footer/Help
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_tab_bar(f, app, chunks[1]);
    if chunks[2].height > 0 {
        draw_filter_bar(f, app, chunks[2]);
    }
    draw_main_content(f, app, chunks[3]);
    draw_footer(f, app, chunks[4]);

    // Modal overlays
    if app.input_mode == InputMode::Editing {
        draw_input_modal(f, app);
    }
    if let InputMode::OrgAdding(add_type) = &app.input_mode {
        draw_org_add_modal(f, app, *add_type);
    }
    if app.input_mode == InputMode::Assigning {
        draw_assign_modal(f, app);
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let stats = &app.model.stats;
    let is_scanning = *app.is_scanning.lock().unwrap();
    let scan_status = app.scan_status.lock().unwrap().clone();

    let scanning_span = if is_scanning {
        let label = if scan_status.is_empty() {
            "SCANNING...".to_string()
        } else {
            scan_status
        };
        Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let text = vec![Line::from(vec![
        Span::styled(
            " Balebos Dashboard ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" Assets: {} ", stats.total),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!(" Alive: {} ", stats.alive),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!(" HTTP Errors: {} ", stats.http_errors),
            Style::default().fg(Color::Red),
        ),
        Span::styled(
            format!(" DNS Issues: {} ", stats.dns_issues),
            Style::default().fg(Color::Yellow),
        ),
        scanning_span,
    ])];

    let header = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Overview "));
    f.render_widget(header, area);
}

// ---------------------------------------------------------------------------
// Tab bar
// ---------------------------------------------------------------------------

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let num = format!("[{}] ", i + 1);
            let style = if *tab == app.active_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(vec![
                Span::styled(num, style),
                Span::styled(tab.title(), style),
            ])
        })
        .collect();

    let selected = match app.active_tab {
        Tab::Inventory => 0,
        Tab::Organization => 1,
        Tab::Projects => 2,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" Tabs "))
        .select(selected)
        .style(Style::default())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" | ");

    f.render_widget(tabs, area);
}

// ---------------------------------------------------------------------------
// Filter bar (only for Inventory tab currently)
// ---------------------------------------------------------------------------

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();

    if !app.filter.is_empty() || app.input_mode == InputMode::Filtering {
        let style = if app.input_mode == InputMode::Filtering {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let filter_text = if app.filter.is_empty() && app.input_mode == InputMode::Filtering {
            "Type to filter...".to_string()
        } else {
            app.filter.clone()
        };

        spans.push(Span::styled(format!(" SEARCH: /{} ", filter_text), style));
    }

    if app.hide_external_redirects {
        spans.push(Span::styled(
            " [EXT REDIRECTS HIDDEN] ",
            Style::default()
                .fg(Color::Indexed(208))
                .add_modifier(Modifier::BOLD),
        ));
    }

    let p = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(" Active Filters "));
    f.render_widget(p, area);
}

// ---------------------------------------------------------------------------
// Main content (delegates to tab modules)
// ---------------------------------------------------------------------------

fn draw_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    match app.active_tab {
        Tab::Inventory => draw_inventory(f, app, area),
        Tab::Organization => draw_organization(f, app, area),
        Tab::Projects => draw_projects(f, app, area),
    }
}

// ---------------------------------------------------------------------------
// Footer (context-sensitive help)
// ---------------------------------------------------------------------------

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.active_tab {
        Tab::Inventory => {
            " q: Quit | 1-3: Tabs | /: Filter | x: Ext | a: Add | s: Scan | o: Assign | Space: Toggle "
        }
        Tab::Organization => {
            " q: Quit | 1-3: Tabs | ←/→: Pane | a: Add | j/k: Nav "
        }
        Tab::Projects => {
            " q: Quit | 1-3: Tabs | a: Add Project | Space: Toggle | j/k: Nav "
        }
    };

    let footer =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(footer, area);
}

// ---------------------------------------------------------------------------
// Modals
// ---------------------------------------------------------------------------

fn draw_input_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, f.area());
    let block = Block::default()
        .title(" Add New Domain to Scan ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(block);
    f.render_widget(Clear, area);
    f.render_widget(input, area);

    f.set_cursor_position(ratatui::layout::Position::new(
        area.x + app.input.len() as u16 + 1,
        area.y + 1,
    ));
}

fn draw_assign_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, f.area());

    let asset_name = app
        .selected_asset()
        .map(|a| a.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let block = Block::default()
        .title(" Assign Asset to Project ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Asset info
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Project ID input
            Constraint::Length(1), // Spacer
            Constraint::Min(3),    // Available projects list
            Constraint::Length(1), // Hint
        ])
        .split(inner);

    // Asset info
    let asset_line = Paragraph::new(Line::from(vec![
        Span::styled("Asset: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(asset_name, Style::default().fg(Color::Cyan)),
    ]));
    f.render_widget(asset_line, chunks[0]);

    // Project ID input
    let input_line = Paragraph::new(Line::from(vec![
        Span::styled(
            "Project ID: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(app.input.as_str(), Style::default().fg(Color::Yellow)),
        Span::styled("_", Style::default().fg(Color::Yellow)),
    ]));
    f.render_widget(input_line, chunks[2]);

    // Available projects
    let mut project_lines: Vec<Line> = vec![Line::from(Span::styled(
        "Available Projects:",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::UNDERLINED),
    ))];

    for project in app.org_model.project_views.iter().take(5) {
        project_lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", project.id),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(&project.name),
            Span::styled(
                format!(" ({})", project.brand_name),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    if app.org_model.project_views.len() > 5 {
        project_lines.push(Line::from(Span::styled(
            format!("  ... and {} more", app.org_model.project_views.len() - 5),
            Style::default().fg(Color::DarkGray),
        )));
    }
    if app.org_model.project_views.is_empty() {
        project_lines.push(Line::from(Span::styled(
            "  No projects available. Create one first.",
            Style::default().fg(Color::Red),
        )));
    }

    let projects_p = Paragraph::new(project_lines);
    f.render_widget(projects_p, chunks[4]);

    // Hint
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::raw(": Assign | "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(": Cancel"),
    ]));
    f.render_widget(hint, chunks[5]);
}

fn draw_org_add_modal(f: &mut Frame, app: &App, add_type: OrgAddType) {
    let fields = add_type.fields();
    let height = (fields.len() as u16 * 3) + 4;
    let area = centered_rect(50, height.min(80), f.area());

    let block = Block::default()
        .title(format!(" {} ", add_type.title()))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let field_height = 2;
    let constraints: Vec<Constraint> = fields
        .iter()
        .map(|_| Constraint::Length(field_height))
        .chain(std::iter::once(Constraint::Min(1))) // space for hint
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, field_name) in fields.iter().enumerate() {
        let is_active = i == app.form_field_index;
        let value = app.form_fields.get(i).map(|s| s.as_str()).unwrap_or("");

        let style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let label_style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let text = Line::from(vec![
            Span::styled(format!("{}: ", field_name), label_style),
            Span::styled(value, style),
            if is_active {
                Span::styled("_", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]);

        let p = Paragraph::new(text);
        f.render_widget(p, chunks[i]);
    }

    // Hint at bottom
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::DarkGray)),
        Span::raw(": Next field | "),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::raw(": Submit | "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(": Cancel"),
    ]));
    if let Some(hint_area) = chunks.last() {
        f.render_widget(hint, *hint_area);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
