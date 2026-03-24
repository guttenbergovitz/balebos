//! Organization tab rendering.
//!
//! Displays persons, companies, and brands with a sub-pane selector.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::OrgPane;

/// Draw the Organization tab content.
pub fn draw_organization(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Sub-tabs (Persons/Companies/Brands)
            Constraint::Min(5),    // Content
        ])
        .split(area);

    draw_org_tabs(f, app, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // List
            Constraint::Percentage(50), // Details
        ])
        .split(chunks[1]);

    draw_org_list(f, app, content_chunks[0]);
    draw_org_details(f, app, content_chunks[1]);
}

fn draw_org_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = OrgPane::all()
        .iter()
        .map(|pane| {
            let style = if *pane == app.organization.pane {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::styled(pane.title(), style)
        })
        .collect();

    let selected = match app.organization.pane {
        OrgPane::Persons => 0,
        OrgPane::Companies => 1,
        OrgPane::Brands => 2,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Organization (←/→ to switch) "),
        )
        .select(selected)
        .style(Style::default())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn draw_org_list(f: &mut Frame, app: &mut App, area: Rect) {
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Yellow);

    match app.organization.pane {
        OrgPane::Persons => draw_persons_table(f, app, area, selected_style),
        OrgPane::Companies => draw_companies_table(f, app, area, selected_style),
        OrgPane::Brands => draw_brands_table(f, app, area, selected_style),
    }
}

fn draw_persons_table(f: &mut Frame, app: &mut App, area: Rect, selected_style: Style) {
    let header_cells = ["Name", "Email"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows = app.org_model.persons.iter().map(|person| {
        Row::new(vec![
            Cell::from(format!("{} {}", person.first_name, person.last_name)),
            Cell::from(person.email.clone()),
        ])
    });

    let t = Table::new(
        rows,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Persons ({}) ", app.org_model.persons.len())),
    )
    .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, &mut app.organization.table_state);
}

fn draw_companies_table(f: &mut Frame, app: &mut App, area: Rect, selected_style: Style) {
    let header_cells = ["Name", "Country", "Brand", "Type"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows = app
        .org_model
        .companies
        .iter()
        .map(|company: &crate::org::CompanyRecord| {
            let type_str = if company.is_external() {
                "External"
            } else {
                "Internal"
            };
            let type_style = if company.is_external() {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };

            let brand_name = company
                .brand
                .and_then(|id| app.org_model.brands_by_id.get(&id))
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "-".to_string());

            Row::new(vec![
                Cell::from(company.name.clone()),
                Cell::from(company.country.alpha2().to_string()),
                Cell::from(brand_name),
                Cell::from(type_str).style(type_style),
            ])
        });

    let t = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(10),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Companies ({}) ", app.org_model.companies.len())),
    )
    .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, &mut app.organization.table_state);
}

fn draw_brands_table(f: &mut Frame, app: &mut App, area: Rect, selected_style: Style) {
    let header_cells = ["Name", "ID"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows = app.org_model.brands.iter().map(|brand| {
        Row::new(vec![
            Cell::from(brand.name.clone()),
            Cell::from(brand.id.to_string()).style(Style::default().fg(Color::DarkGray)),
        ])
    });

    let t = Table::new(
        rows,
        [Constraint::Percentage(70), Constraint::Percentage(30)],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Brands ({}) ", app.org_model.brands.len())),
    )
    .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, &mut app.organization.table_state);
}

fn draw_org_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Details ");

    let text = match app.organization.pane {
        OrgPane::Persons => build_person_details(app),
        OrgPane::Companies => build_company_details(app),
        OrgPane::Brands => build_brand_details(app),
    };

    let details = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(details, area);
}

fn build_person_details(app: &App) -> Vec<Line<'static>> {
    let idx = match app.organization.table_state.selected() {
        Some(i) => i,
        None => return vec![Line::from("Select a person to see details")],
    };

    let person = match app.org_model.persons.get(idx) {
        Some(p) => p,
        None => return vec![Line::from("Person not found")],
    };

    vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{} {}", person.first_name, person.last_name)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Email: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(person.email.clone()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Phone: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(
                person
                    .phone
                    .clone()
                    .unwrap_or_else(|| "Not provided".to_string()),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(person.id.to_string(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                person.created_at.format("%Y-%m-%d %H:%M").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]
}

fn build_company_details(app: &App) -> Vec<Line<'static>> {
    let idx = match app.organization.table_state.selected() {
        Some(i) => i,
        None => return vec![Line::from("Select a company to see details")],
    };

    let company: &crate::org::CompanyRecord = match app.org_model.companies.get(idx) {
        Some(c) => c,
        None => return vec![Line::from("Company not found")],
    };

    let type_line = if company.is_external() {
        Line::from(vec![
            Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("External", Style::default().fg(Color::Yellow)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("Internal", Style::default().fg(Color::Green)),
        ])
    };

    vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(company.name.clone()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Country: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(
                "{} ({})",
                company.country.name(),
                company.country.alpha2()
            )),
        ]),
        Line::from(""),
        type_line,
        Line::from(""),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(company.id.to_string(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                company.created_at.format("%Y-%m-%d %H:%M").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]
}

fn build_brand_details(app: &App) -> Vec<Line<'static>> {
    let idx = match app.organization.table_state.selected() {
        Some(i) => i,
        None => return vec![Line::from("Select a brand to see details")],
    };

    let brand = match app.org_model.brands.get(idx) {
        Some(b) => b,
        None => return vec![Line::from("Brand not found")],
    };

    // Count companies and projects for this brand
    let company_ids: std::collections::HashSet<_> = app
        .org_model
        .companies
        .iter()
        .filter(|c| c.brand == Some(brand.id))
        .map(|c| c.id)
        .collect();

    let company_count = company_ids.len();
    let project_count = app
        .org_model
        .projects
        .iter()
        .filter(|p| company_ids.contains(&p.company))
        .count();

    vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(brand.name.clone()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Companies: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(company_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Projects: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(project_count.to_string()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(brand.id.to_string(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                brand.created_at.format("%Y-%m-%d %H:%M").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]
}
