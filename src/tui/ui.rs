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
use crate::tui::tabs::{build_details_text, draw_inventory, draw_organization, draw_projects};

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
    if let InputMode::OrgAdding(add_type) | InputMode::OrgEditing(add_type, _) = &app.input_mode {
        draw_org_add_modal(f, app, *add_type);
    }
    if app.input_mode == InputMode::Assigning {
        draw_assign_modal(f, app);
    }
    if let InputMode::RoleAssigning(step) = &app.input_mode {
        draw_role_assign_modal(f, app, step);
    }
    if let InputMode::ContractorAssigning(step) = &app.input_mode {
        draw_contractor_assign_modal(f, app, step);
    }
    if app.input_mode == InputMode::AssetDetails {
        draw_asset_details_modal(f, app);
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
            " q: Quit | R: Refresh | 1-3: Tabs | /: Filter | x: Ext | a: Add | s: Scan | m: Mark | o: Assign | i: Info | Space/Enter: Toggle "
        }
        Tab::Organization => {
            " q: Quit | R: Refresh | 1-3: Tabs | ←/→: Pane | a: Add | e: Edit | j/k: Nav "
        }
        Tab::Projects => {
            " q: Quit | R: Refresh | 1-3: Tabs | a: Add | e: Edit | r: Role | c: Contractor | Space: Toggle | j/k: Nav "
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
    let area = fixed_size_centered_rect(60, 20, f.area());

    let block = Block::default()
        .title(" Assign to Project ")
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
            Constraint::Length(1), // Asset info
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Project ID input
            Constraint::Length(1), // Spacer
            Constraint::Min(3),    // Available projects list
            Constraint::Length(1), // Hint
        ])
        .split(inner);

    let asset_info = if !app.marked_assets.is_empty() {
        format!("{} marked items", app.marked_assets.len())
    } else {
        app.selected_asset()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    };

    // Asset info
    let asset_line = Paragraph::new(Line::from(vec![
        Span::styled("Assigning: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(asset_info, Style::default().fg(Color::Cyan)),
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
        " Search Projects (Type to filter, Arrows to navigate, Enter to pick):",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))];

    if app.org_select_matches.is_empty() {
        project_lines.push(Line::from(Span::styled(
            "  No matching projects found.",
            Style::default().fg(Color::Red),
        )));
    } else {
        for (i, (display, id)) in app.org_select_matches.iter().enumerate().take(8) {
            let is_selected = Some(i) == app.org_select_index;
            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            project_lines.push(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(format!("{:<30}", display), style),
                Span::styled(format!(" [{}]", id), Style::default().fg(Color::DarkGray)),
            ]));
        }
        
        if app.org_select_matches.len() > 8 {
            project_lines.push(Line::from(Span::styled(
                format!("  ... and {} more", app.org_select_matches.len() - 8),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let projects_p = Paragraph::new(project_lines);
    f.render_widget(projects_p, chunks[4]);

    // Hint
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::raw(": Pick/Assign | "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(": Cancel"),
    ]));
    f.render_widget(hint, chunks[5]);
}

fn draw_org_add_modal(f: &mut Frame, app: &App, add_type: OrgAddType) {
    let fields = add_type.fields();
    let has_selection = !app.org_select_matches.is_empty();
    
    let title = match app.input_mode {
        InputMode::OrgEditing(_, _) => format!(" Edit {} ", add_type.title().replace("Add ", "")),
        _ => format!(" {} ", add_type.title()),
    };
    
    // Calculate total height: 3 lines per field + header/footer + selection list
    let field_area_height = fields.len() as u16 * 3;
    let selection_height = if has_selection { 8 } else { 0 };
    let total_inner_height = field_area_height + selection_height + 2; // +2 for hints
    
    let area = fixed_size_centered_rect(60, total_inner_height + 4, f.area());

    let block = Block::default()
        .title(title)
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

    let constraints = if has_selection {
        vec![
            Constraint::Length(field_area_height),
            Constraint::Length(1), // Spacer
            Constraint::Min(5),    // Selection list
            Constraint::Length(1), // Hint
        ]
    } else {
        vec![
            Constraint::Length(field_area_height),
            Constraint::Min(1), // space for hint
        ]
    };

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Render Fields
    let field_constraints: Vec<Constraint> = fields
        .iter()
        .map(|_| Constraint::Length(3))
        .collect();
    
    let field_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(field_constraints)
        .split(main_chunks[0]);

    for (i, field_name) in fields.iter().enumerate() {
        let is_active = i == app.form_field_index;
        let value = app.form_fields.get(i).map(|s| s.as_str()).unwrap_or("");

        let border_style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let field_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                format!(" {} ", field_name),
                if is_active {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ));

        let text = Line::from(vec![
            Span::styled(value, if is_active { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) }),
            if is_active {
                Span::styled("_", Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK))
            } else {
                Span::raw("")
            },
        ]);

        let p = Paragraph::new(text).block(field_block);
        f.render_widget(p, field_chunks[i]);
    }

    // Render Selection List if active
    if has_selection {
        let mut list_items: Vec<Line> = vec![Line::from(Span::styled(
            " Select from list (Arrows to navigate, Enter to pick):",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))];

        for (i, (name, id)) in app.org_select_matches.iter().enumerate().take(5) {
            let is_selected = Some(i) == app.org_select_index;
            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let name_span = Span::styled(format!("{:<20}", name), style);
            let id_span = Span::styled(format!(" ({})", id), Style::default().fg(Color::DarkGray));

            list_items.push(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                name_span,
                id_span,
            ]));
        }

        if app.org_select_matches.len() > 5 {
            list_items.push(Line::from(Span::styled(
                format!("  ... and {} more", app.org_select_matches.len() - 5),
                Style::default().fg(Color::DarkGray),
            )));
        }

        let selection_p = Paragraph::new(list_items);
        f.render_widget(selection_p, main_chunks[2]);
    }

    // Hint at bottom
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::DarkGray)),
        Span::raw(": Next field | "),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::raw(": Submit/Pick | "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(": Cancel"),
    ]));
    
    let hint_area = if has_selection { main_chunks[3] } else { main_chunks[1] };
    f.render_widget(hint, hint_area);
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

fn fixed_size_centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1]
}

fn draw_role_assign_modal(f: &mut Frame, app: &App, step: &RoleAssignStep) {
    let area = fixed_size_centered_rect(60, 20, f.area());
    let block = Block::default()
        .title(" Assign Person to Project ")
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

    match step {
        RoleAssignStep::SelectTarget => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Search
                    Constraint::Length(1), // Spacer
                    Constraint::Min(5),    // List
                    Constraint::Length(1), // Hint
                ])
                .split(inner);

            let search_line = Line::from(vec![
                Span::styled("Search Person: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&app.input),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            f.render_widget(Paragraph::new(search_line), chunks[0]);

            let mut list_items = vec![Line::from(Span::styled("Select person:", Style::default().fg(Color::Cyan)))];
            for (i, (name, id)) in app.org_select_matches.iter().enumerate().take(10) {
                let is_selected = Some(i) == app.org_select_index;
                let style = if is_selected {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };
                list_items.push(Line::from(vec![
                    Span::styled(if is_selected { "> " } else { "  " }, style),
                    Span::styled(format!("{:<30}", name), style),
                    Span::styled(format!(" [{}]", id), Style::default().fg(Color::DarkGray)),
                ]));
            }
            f.render_widget(Paragraph::new(list_items), chunks[2]);
            
            f.render_widget(Paragraph::new("Arrows to navigate, Enter to pick, Esc to cancel"), chunks[3]);
        }
        RoleAssignStep::SelectRole => {
            let text = vec![
                Line::from("Select a role for this person:"),
                Line::from(""),
                Line::from(vec![
                    Span::styled("[o]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Owner (Business Responsibility)"),
                ]),
                Line::from(vec![
                    Span::styled("[m]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Manager (Project Oversight)"),
                ]),
                Line::from(vec![
                    Span::styled("[t]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Technician (Technical Access)"),
                ]),
                Line::from(""),
                Line::from(Span::styled("Esc to go back", Style::default().fg(Color::DarkGray))),
            ];
            f.render_widget(Paragraph::new(text), inner);
        }
    }
}

fn draw_contractor_assign_modal(f: &mut Frame, app: &App, step: &ContractorAssignStep) {
    let area = fixed_size_centered_rect(60, 20, f.area());
    let block = Block::default()
        .title(" Assign External Contractor ")
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

    match step {
        ContractorAssignStep::SelectCompany => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(5),
                    Constraint::Length(1),
                ])
                .split(inner);

            let search_line = Line::from(vec![
                Span::styled("Search Company: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&app.input),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            f.render_widget(Paragraph::new(search_line), chunks[0]);

            let mut list_items = vec![Line::from(Span::styled(
                "Select external company:",
                Style::default().fg(Color::Cyan),
            ))];
            for (i, (name, id)) in app.org_select_matches.iter().enumerate().take(10) {
                let is_selected = Some(i) == app.org_select_index;
                let style = if is_selected {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };
                list_items.push(Line::from(vec![
                    Span::styled(if is_selected { "> " } else { "  " }, style),
                    Span::styled(format!("{:<30}", name), style),
                    Span::styled(format!(" [{}]", id), Style::default().fg(Color::DarkGray)),
                ]));
            }
            f.render_widget(Paragraph::new(list_items), chunks[2]);

            f.render_widget(
                Paragraph::new("Arrows to navigate, Enter to pick, Esc to cancel"),
                chunks[3],
            );
        }
        ContractorAssignStep::SelectRole => {
            let text = vec![
                Line::from("Select a contractor role:"),
                Line::from(""),
                Line::from(vec![
                    Span::styled("[d]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Development"),
                ]),
                Line::from(vec![
                    Span::styled("[o]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - DevOps"),
                ]),
                Line::from(vec![
                    Span::styled("[g]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Design"),
                ]),
                Line::from(vec![
                    Span::styled("[c]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Consultancy"),
                ]),
                Line::from(vec![
                    Span::styled("[s]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" - Security"),
                ]),
                Line::from(""),
                Line::from(Span::styled("Esc to go back", Style::default().fg(Color::DarkGray))),
            ];
            f.render_widget(Paragraph::new(text), inner);
        }
    }
}

fn draw_asset_details_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(90, 90, f.area());
    let block = Block::default()
        .title(" Full Asset Details (Esc/Enter to close, j/k to scroll) ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);

    if let Some(asset) = app.selected_asset() {
        let text = build_details_text(asset);
        let details = Paragraph::new(text)
            .block(block)
            .scroll((app.asset_details_scroll, 0))
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(details, area);
    }
}

use crate::tui::app::{ContractorAssignStep, RoleAssignStep};

