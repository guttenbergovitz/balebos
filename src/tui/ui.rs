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
use crate::tui::tabs::{draw_findings, draw_inventory, draw_organization, draw_projects};

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &mut App) {
    // Clear old status messages
    app.clear_old_status();

    let show_filter = app.input_mode == InputMode::Filtering
        || !app.filter.is_empty()
        || app.inventory.hide_external_redirects
        || app.inventory.filter_tag.is_some();

    // Build constraints dynamically
    let mut constraints = vec![
        Constraint::Length(3), // Header/Stats
        Constraint::Length(3), // Tab bar
    ];

    if show_filter {
        constraints.push(Constraint::Length(3)); // Filter Bar
    }

    constraints.push(Constraint::Min(10)); // Main View (flexible)
    constraints.push(Constraint::Length(3)); // Status Bar (always visible - increased for better visibility)
    constraints.push(Constraint::Length(3)); // Footer/Help

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    let mut idx = 0;
    draw_header(f, app, chunks[idx]);
    idx += 1;
    draw_tab_bar(f, app, chunks[idx]);
    idx += 1;

    if show_filter {
        draw_filter_bar(f, app, chunks[idx]);
        idx += 1;
    }

    draw_main_content(f, app, chunks[idx]);
    idx += 1;
    draw_status_bar(f, app, chunks[idx]); // Always show status bar
    idx += 1;
    draw_footer(f, app, chunks[idx]);

    // Modal overlays
    if app.input_mode == InputMode::Editing {
        draw_input_modal(f, app);
    }
    if app.input_mode == InputMode::TagFilterSelecting {
        draw_tag_filter_select_modal(f, app);
    }
    if app.input_mode == InputMode::TagSelecting {
        draw_tag_select_modal(f, app);
    }
    if app.input_mode == InputMode::TagAdding {
        draw_tag_add_modal(f, app);
    }
    if app.input_mode == InputMode::TagRemovingSelecting {
        draw_tag_remove_select_modal(f, app);
    }
    if app.input_mode == InputMode::TagRemoving {
        draw_tag_remove_modal(f, app);
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
    if app.input_mode == InputMode::LifecycleEditing {
        draw_lifecycle_edit_modal(f, app);
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let stats = &app.model.stats;
    let is_scanning = *app.is_scanning.lock().unwrap_or_else(|p| p.into_inner());

    let scanning_span = if is_scanning {
        Span::styled(
            " ⟳ SCANNING... ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let storage_name = app.storage.name();
    let text = vec![Line::from(vec![
        Span::styled(
            format!(" Storage: {} ", storage_name),
            Style::default()
                .bg(if storage_name == "DuckDB" {
                    Color::Blue
                } else {
                    Color::DarkGray
                })
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            " Balebos v1.3.0 ",
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

    let header =
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Overview "));
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
        Tab::Findings => 3,
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

    if app.inventory.hide_external_redirects {
        spans.push(Span::styled(
            " [EXT REDIRECTS HIDDEN] ",
            Style::default()
                .fg(Color::Indexed(208))
                .add_modifier(Modifier::BOLD),
        ));
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Active Filters "),
    );
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
        Tab::Findings => draw_findings(f, app, area),
    }
}

// ---------------------------------------------------------------------------
// Status bar (for messages/toasts)
// ---------------------------------------------------------------------------

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    use crate::tui::quotes::get_random_quote;

    let scan_status_locked = app.scan_status.lock().unwrap_or_else(|p| p.into_inner());
    let scan_state = scan_status_locked.clone();
    drop(scan_status_locked);

    let is_scanning = *app.is_scanning.lock().unwrap_or_else(|p| p.into_inner());

    // Check if status expired (older than 5 seconds) or empty
    let now = chrono::Utc::now();
    let should_show_status = !scan_state.message.is_empty()
        && (is_scanning || now.signed_duration_since(scan_state.timestamp).num_seconds() < 5);

    let status_text = if should_show_status {
        if is_scanning && !scan_state.steps.is_empty() {
            // Show current step and summary of completed steps
            let done = scan_state.steps.len();
            let total = 8; // total recon steps
            format!("⟳ {} [{}/{} done]", scan_state.message, done, total)
        } else {
            scan_state.message
        }
    } else if let Some((ref msg, msg_time)) = app.status_message {
        if now.signed_duration_since(msg_time).num_seconds() < 5 {
            msg.clone()
        } else {
            get_random_quote().to_string()
        }
    } else {
        get_random_quote().to_string()
    };

    let status_style = if status_text.starts_with('✓') {
        Style::default().fg(Color::Green)
    } else if status_text.contains("error")
        || status_text.contains("failed")
        || status_text.starts_with('❌')
    {
        Style::default().fg(Color::Red)
    } else if status_text.starts_with('⟳') {
        Style::default().fg(Color::Cyan)
    } else {
        // Quotes are displayed in dim/gray
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
    };

    let status = Paragraph::new(format!(" {} ", status_text))
        .style(status_style)
        .block(Block::default().borders(Borders::TOP).title(" Status "));

    f.render_widget(status, area);
}

// ---------------------------------------------------------------------------
// Footer (context-sensitive help)
// ---------------------------------------------------------------------------

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.active_tab {
        Tab::Inventory => {
            " q: Quit | R: Refresh | 1-4: Tabs | /: Filter | t: Filter Tag | T: Add Tag | d: Remove Tag | L: Lifecycle | v: Risk | x: Ext | a: Add | s: Scan | m: Mark | o: Assign | i: Info "
        }
        Tab::Organization => {
            " q: Quit | R: Refresh | 1-4: Tabs | ←/→: Pane | a: Add | e: Edit | j/k: Nav "
        }
        Tab::Projects => {
            " q: Quit | R: Refresh | 1-4: Tabs | a: Add | e: Edit | r: Role | c: Contractor | Space: Toggle | j/k: Nav "
        }
        Tab::Findings => {
            " q: Quit | R: Refresh | 1-4: Tabs | a: Acknowledge | r: Resolve | s: Scan | f: Filter | Space: Toggle | j/k: Nav "
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

fn draw_tag_filter_select_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 50, f.area());

    let block = Block::default()
        .title(" Filter by Tag (↑/↓: navigate, Enter: select, c/x: clear filter, Esc: cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);

    // Inner area for list
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(4),
    };

    if app.available_tags.is_empty() {
        let text = vec![
            Line::from("No tags available."),
            Line::from(""),
            Line::from("Add tags to assets first with 'T'."),
        ];
        let paragraph = Paragraph::new(text);
        f.render_widget(paragraph, inner);
    } else {
        // Show current filter at top
        let mut lines = vec![];
        if let Some(ref current_tag) = app.inventory.filter_tag {
            lines.push(Line::from(vec![
                Span::styled(
                    "Current filter: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    current_tag.as_str(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(""));
        }

        // Show list of tags
        for (i, tag) in app.available_tags.iter().enumerate() {
            let style = if i == app.tag_select_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(format!("  {}", tag), style)));
        }

        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, inner);
    }
}

fn draw_tag_select_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 50, f.area());
    let asset_name = app
        .selected_asset()
        .map(|a| a.name.as_str())
        .unwrap_or("(none)");

    let block = Block::default()
        .title(format!(
            " Select Tag for '{}' (↑/↓: navigate, Enter: select, n: new, Esc: cancel) ",
            asset_name
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);

    // Inner area for list
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(4),
    };

    if app.available_tags.is_empty() {
        let text = vec![
            Line::from("No existing tags."),
            Line::from(""),
            Line::from("Press 'n' to create a new tag."),
        ];
        let paragraph = Paragraph::new(text);
        f.render_widget(paragraph, inner);
    } else {
        // Show list of tags
        let items: Vec<Line> = app
            .available_tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let style = if i == app.tag_select_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!("  {}", tag), style))
            })
            .collect();

        let paragraph = Paragraph::new(items);
        f.render_widget(paragraph, inner);
    }
}

fn draw_tag_add_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 40, f.area());
    let asset_name = app
        .selected_asset()
        .map(|a| a.name.as_str())
        .unwrap_or("(none)");

    // Get all unique tags
    let all_tags: Vec<String> = app
        .model
        .asset_tags
        .values()
        .flat_map(|tags| tags.iter())
        .map(|s| s.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    // Get current tags for this asset
    let current_tags = app
        .model
        .asset_tags
        .get(asset_name)
        .map(|tags| {
            tags.iter()
                .map(|t| format!("  • {}", t))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "  (none)".to_string());

    let available_tags = if all_tags.is_empty() {
        "  (no existing tags)".to_string()
    } else {
        all_tags
            .iter()
            .map(|t| format!("  • {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let text = format!(
        "Add tag to '{}':\n\nCurrent tags:\n{}\n\nAvailable tags:\n{}\n\nNew tag: {}_",
        asset_name, current_tags, available_tags, app.input
    );

    let block = Block::default()
        .title(" Add Tag (Enter to confirm, Esc to cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .block(block);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_tag_remove_select_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 50, f.area());
    let asset_name = app
        .selected_asset()
        .map(|a| a.name.as_str())
        .unwrap_or("(none)");

    let block = Block::default()
        .title(format!(
            " Remove Tag from '{}' (↑/↓: navigate, Enter: remove, Esc: cancel) ",
            asset_name
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);

    // Inner area for list
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(4),
    };

    if app.available_tags.is_empty() {
        let text = vec![
            Line::from("No tags on this asset."),
            Line::from(""),
            Line::from("Add tags first with 'T'."),
        ];
        let paragraph = Paragraph::new(text);
        f.render_widget(paragraph, inner);
    } else {
        // Show list of tags to remove
        let items: Vec<Line> = app
            .available_tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let style = if i == app.tag_select_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!("  {}", tag), style))
            })
            .collect();

        let paragraph = Paragraph::new(items);
        f.render_widget(paragraph, inner);
    }
}

fn draw_tag_remove_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, f.area());
    let asset_name = app
        .selected_asset()
        .map(|a| a.name.as_str())
        .unwrap_or("(none)");

    // Show current tags
    let current_tags = app
        .selected_asset()
        .and_then(|a| app.model.asset_tags.get(&a.name))
        .map(|tags| {
            tags.iter()
                .map(|t| format!("  • {}", t))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "  (no tags)".to_string());

    let text = format!(
        "Remove tag from '{}':\n\nCurrent tags:\n{}\n\nTag to remove: {}_",
        asset_name, current_tags, app.input
    );

    let block = Block::default()
        .title(" Remove Tag (Enter to confirm, Esc to cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .block(block);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
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

    let asset_info = if !app.inventory.marked_assets.is_empty() {
        format!("{} marked items", app.inventory.marked_assets.len())
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

    if app.form.select_matches.is_empty() {
        project_lines.push(Line::from(Span::styled(
            "  No matching projects found.",
            Style::default().fg(Color::Red),
        )));
    } else {
        for (i, (display, id)) in app.form.select_matches.iter().enumerate().take(8) {
            let is_selected = Some(i) == app.form.select_index;
            let style = if is_selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            project_lines.push(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(format!("{:<30}", display), style),
                Span::styled(format!(" [{}]", id), Style::default().fg(Color::DarkGray)),
            ]));
        }

        if app.form.select_matches.len() > 8 {
            project_lines.push(Line::from(Span::styled(
                format!("  ... and {} more", app.form.select_matches.len() - 8),
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
    let has_selection = !app.form.select_matches.is_empty();

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
    let field_constraints: Vec<Constraint> = fields.iter().map(|_| Constraint::Length(3)).collect();

    let field_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(field_constraints)
        .split(main_chunks[0]);

    for (i, field_name) in fields.iter().enumerate() {
        let is_active = i == app.form.field_index;
        let value = app.form.fields.get(i).map(|s| s.as_str()).unwrap_or("");

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
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ));

        let text = Line::from(vec![
            Span::styled(
                value,
                if is_active {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            if is_active {
                Span::styled(
                    "_",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::SLOW_BLINK),
                )
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
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))];

        for (i, (name, id)) in app.form.select_matches.iter().enumerate().take(5) {
            let is_selected = Some(i) == app.form.select_index;
            let style = if is_selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
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

        if app.form.select_matches.len() > 5 {
            list_items.push(Line::from(Span::styled(
                format!("  ... and {} more", app.form.select_matches.len() - 5),
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

    let hint_area = if has_selection {
        main_chunks[3]
    } else {
        main_chunks[1]
    };
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
                Span::styled(
                    "Search Person: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.input),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            f.render_widget(Paragraph::new(search_line), chunks[0]);

            let mut list_items = vec![Line::from(Span::styled(
                "Select person:",
                Style::default().fg(Color::Cyan),
            ))];
            for (i, (name, id)) in app.form.select_matches.iter().enumerate().take(10) {
                let is_selected = Some(i) == app.form.select_index;
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
        RoleAssignStep::SelectRole => {
            let text = vec![
                Line::from("Select a role for this person:"),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[o]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Owner (Business Responsibility)"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[m]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Manager (Project Oversight)"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[t]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Technician (Technical Access)"),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Esc to go back",
                    Style::default().fg(Color::DarkGray),
                )),
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
                Span::styled(
                    "Search Company: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.input),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            f.render_widget(Paragraph::new(search_line), chunks[0]);

            let mut list_items = vec![Line::from(Span::styled(
                "Select external company:",
                Style::default().fg(Color::Cyan),
            ))];
            for (i, (name, id)) in app.form.select_matches.iter().enumerate().take(10) {
                let is_selected = Some(i) == app.form.select_index;
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
                    Span::styled(
                        "[d]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Development"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[o]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - DevOps"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[g]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Design"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[c]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Consultancy"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[s]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Security"),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Esc to go back",
                    Style::default().fg(Color::DarkGray),
                )),
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
        let text = crate::tui::tabs::inventory::build_details_text(asset, app);
        let details = Paragraph::new(text)
            .block(block)
            .scroll((app.asset_details_scroll, 0))
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(details, area);
    }
}

use crate::tui::app::{ContractorAssignStep, RoleAssignStep};

fn draw_lifecycle_edit_modal(f: &mut Frame, app: &App) {
    use crate::inventory::AssetLifecycle;
    
    let area = centered_rect(50, 40, f.area());
    let block = Block::default()
        .title(" Set Asset Lifecycle State (Esc to cancel) ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    f.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("1. ", Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::styled("Active", Style::default().fg(Color::Green)),
            Span::raw(" - Normal operations"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("2. ", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled("Under Review", Style::default().fg(Color::Yellow)),
            Span::raw(" - Being evaluated"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("3. ", Style::default().fg(Color::Magenta)),
            Span::raw(" "),
            Span::styled("Deprecated", Style::default().fg(Color::Magenta)),
            Span::raw(" - Scheduled for removal"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("4. ", Style::default().fg(Color::Gray)),
            Span::raw(" "),
            Span::styled("Decommissioned", Style::default().fg(Color::Gray)),
            Span::raw(" - Removed but tracked"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("5. ", Style::default().fg(Color::Red)),
            Span::raw(" "),
            Span::styled("Abandoned", Style::default().fg(Color::Red)),
            Span::raw(" - No owner, stale"),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Current: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            if let Some(asset) = app.selected_asset() {
                let state = crate::inventory::get_current_lifecycle(&app.model.lifecycle_events, &asset.name);
                match state {
                    Some(s) => Span::styled(
                        format!("{} {}", s.icon(), s),
                        Style::default().fg(s.color()),
                    ),
                    None => Span::styled("Not set", Style::default().fg(Color::DarkGray)),
                }
            } else {
                Span::raw("No asset selected")
            },
        ]),
    ];

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}
