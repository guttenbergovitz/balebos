//! Projects tab rendering.
//!
//! Displays projects with their assigned assets, persons, and companies.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::App;

/// Draw the Projects tab content.
pub fn draw_projects(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Projects list
            Constraint::Percentage(50), // Project details
        ])
        .split(area);

    draw_projects_table(f, app, chunks[0]);
    draw_project_details(f, app, chunks[1]);
}

fn draw_projects_table(f: &mut Frame, app: &mut App, area: Rect) {
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Yellow);

    let header_cells = ["Project", "Brand", "Company", "Assets"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let mut rows = Vec::new();
    for project in &app.org_model.project_views {
        let is_expanded = app.expanded_projects.contains(&project.id.to_string());
        let prefix = if is_expanded { "[-] " } else { "[+] " };

        rows.push(Row::new(vec![
            Cell::from(Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    project.name.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            Cell::from(project.brand_name.clone()),
            Cell::from(project.company_name.clone()),
            Cell::from(format!("{}", project.asset_count)),
        ]));

        if is_expanded {
            if let Some(assets) = app.org_model.assets_by_project.get(&project.id) {
                for asset in assets {
                    rows.push(Row::new(vec![
                        Cell::from(format!("  └─ {}", asset)).style(Style::default().fg(Color::DarkGray)),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                    ]));
                }
            } else {
                rows.push(Row::new(vec![
                    Cell::from("  (no assets assigned)").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            }
        }
    }

    let t = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Projects ({}) ",
                app.org_model.project_views.len()
            )),
    )
    .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, &mut app.projects_table_state);
}

fn draw_project_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Project Details ");

    let idx = match app.projects_table_state.selected() {
        Some(i) => i,
        None => {
            let details = Paragraph::new("Select a project to see details").block(block);
            f.render_widget(details, area);
            return;
        }
    };

    // Calculate which project is actually selected because of flat rows
    let mut current_idx = 0;
    let mut selected_project = None;

    for project in &app.org_model.project_views {
        if current_idx == idx {
            selected_project = Some(project);
            break;
        }
        current_idx += 1;
        if app.expanded_projects.contains(&project.id.to_string()) {
            if let Some(assets) = app.org_model.assets_by_project.get(&project.id) {
                current_idx += assets.len();
            } else {
                current_idx += 1; // "no assets" line
            }
        }
    }

    let project = match selected_project {
        Some(p) => p,
        None => {
            let details = Paragraph::new("Asset selected (Select project row for details)").block(block);
            f.render_widget(details, area);
            return;
        }
    };

    let mut text = vec![
        Line::from(vec![
            Span::styled("Project: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                project.name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Brand: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(project.brand_name.clone()),
        ]),
        Line::from(vec![
            Span::styled("Company: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(project.company_name.clone()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Statistics:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(format!("  Assets:    {}", project.asset_count)),
        Line::from(format!("  Persons:   {}", project.person_count)),
        Line::from(format!("  Companies: {}", project.company_count)),
        Line::from(""),
        Line::from(Span::styled(
            "Assigned People:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )),
    ];

    if project.assigned_persons.is_empty() {
        text.push(Line::from(Span::styled("  No persons assigned", Style::default().fg(Color::DarkGray))));
    } else {
        for person in &project.assigned_persons {
            text.push(Line::from(vec![
                Span::styled(format!("  • {} ", person.name), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(format!("({}) ", person.role), Style::default().fg(Color::Yellow)),
                Span::styled(format!("<{}>", person.email), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Assigned Companies:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    )));

    if project.assigned_companies.is_empty() {
        text.push(Line::from(Span::styled("  No companies assigned", Style::default().fg(Color::DarkGray))));
    } else {
        for company in &project.assigned_companies {
            text.push(Line::from(vec![
                Span::styled(format!("  • {} ", company.name), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(format!("({})", company.role), Style::default().fg(Color::Yellow)),
            ]));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Assigned Assets:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    )));

    if let Some(assets) = app.org_model.assets_by_project.get(&project.id) {
        for asset in assets {
            text.push(Line::from(format!("  • {}", asset)));
        }
    } else {
        text.push(Line::from(Span::styled("  No assets assigned", Style::default().fg(Color::DarkGray))));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            project.id.to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Show hint about expansion
    let is_expanded = app.expanded_projects.contains(&project.id.to_string());
    if project.asset_count > 0 {
        text.push(Line::from(""));
        if is_expanded {
            text.push(Line::from(Span::styled(
                "Press Space to collapse in table",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            text.push(Line::from(Span::styled(
                "Press Space to expand in table",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let details = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(details, area);
}
