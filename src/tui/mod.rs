pub mod app;
pub mod model;
pub mod state;
pub mod tabs;
pub mod ui;
pub mod workflows;

use crate::storage::StorageEngine;
use std::sync::Arc;
use crate::tui::app::{App, InputMode, OrgAddType, Tab};
use crate::tui::state::OrgPane;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub fn run(storage: Arc<dyn StorageEngine>) -> crate::error::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run main loop
    let mut app = App::new(storage)?;
    let res = run_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_loop<B: ratatui::backend::Backend<Error = io::Error>>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> crate::error::Result<()> {
    let mut last_scan_state = false;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match &app.input_mode {
                    InputMode::Normal => handle_normal_mode(app, key.code)?,
                    InputMode::Editing => handle_editing_mode(app, key.code),
                    InputMode::Filtering => handle_filtering_mode(app, key.code),
                    InputMode::OrgAdding(_) | InputMode::OrgEditing(_, _) => {
                        handle_org_adding_mode(app, key.code)
                    }
                    InputMode::Assigning => handle_assigning_mode(app, key.code),
                    InputMode::RoleAssigning(_) => handle_role_assigning_mode(app, key.code),
                    InputMode::ContractorAssigning(_) => {
                        handle_contractor_assigning_mode(app, key.code)
                    }
                    InputMode::AssetDetails => handle_asset_details_mode(app, key.code),
                }
            }
        }

        // Detect transition from scanning to finished
        let is_scanning = *app.is_scanning.lock().unwrap_or_else(|p| p.into_inner());
        if last_scan_state && !is_scanning {
            // Scan just finished, auto-refresh
            let _ = app.refresh();
        }
        last_scan_state = is_scanning;

        if app.should_quit {
            return Ok(());
        }
    }
}

// ---------------------------------------------------------------------------
// Input mode handlers
// ---------------------------------------------------------------------------

fn handle_normal_mode(app: &mut App, code: KeyCode) -> crate::error::Result<()> {
    match code {
        // Global keys
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('R') => app.refresh()?,

        // Tab navigation
        KeyCode::Char('1') => app.set_tab(Tab::Inventory),
        KeyCode::Char('2') => app.set_tab(Tab::Organization),
        KeyCode::Char('3') => app.set_tab(Tab::Projects),
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),

        // Tab-specific keys
        _ => match app.active_tab {
            Tab::Inventory => handle_inventory_keys(app, code),
            Tab::Organization => handle_organization_keys(app, code),
            Tab::Projects => handle_projects_keys(app, code),
        },
    }
    Ok(())
}

fn handle_inventory_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('a') => {
            app.input_mode = InputMode::Editing;
        }
        KeyCode::Char('/') | KeyCode::Char('f') => {
            app.input_mode = InputMode::Filtering;
        }
        KeyCode::Char('x') => {
            app.toggle_external_filter();
        }
        KeyCode::Char('s') => {
            app.start_rescan();
        }
        KeyCode::Char('o') => {
            // Assign selected asset to a project
            app.start_assign_asset();
        }
        KeyCode::Char('m') => {
            app.toggle_mark();
        }
        KeyCode::Char('M') => {
            app.clear_marks();
        }
        KeyCode::Char(' ') | KeyCode::Enter => app.toggle_expand(),
        KeyCode::Char('i') => {
            if app.selected_asset().is_some() {
                app.asset_details_scroll = 0;
                app.input_mode = InputMode::AssetDetails;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        _ => {}
    }
}

fn handle_organization_keys(app: &mut App, code: KeyCode) {
    match code {
        // Pane switching within Organization tab
        KeyCode::Left | KeyCode::Char('h') => {
            app.prev_org_pane();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.next_org_pane();
        }
        // Add entity based on current pane
        KeyCode::Char('a') => {
            let add_type = match app.organization.pane {
                OrgPane::Persons => OrgAddType::Person,
                OrgPane::Companies => OrgAddType::Company,
                OrgPane::Brands => OrgAddType::Brand,
            };
            app.start_org_add(add_type);
        }
        // Edit entity
        KeyCode::Char('e') => {
            if let Some(idx) = app.organization.table_state.selected() {
                let add_type = match app.organization.pane {
                    OrgPane::Persons => OrgAddType::Person,
                    OrgPane::Companies => OrgAddType::Company,
                    OrgPane::Brands => OrgAddType::Brand,
                };
                app.start_org_edit(add_type, idx);
            }
        }
        // Navigation
        KeyCode::Down | KeyCode::Char('j') => {
            let count = app.org_item_count();
            if count > 0 {
                let i = app.organization.table_state.selected().unwrap_or(0);
                let next = if i >= count - 1 { 0 } else { i + 1 };
                app.organization.table_state.select(Some(next));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let count = app.org_item_count();
            if count > 0 {
                let i = app.organization.table_state.selected().unwrap_or(0);
                let prev = if i == 0 { count - 1 } else { i - 1 };
                app.organization.table_state.select(Some(prev));
            }
        }
        _ => {}
    }
}

fn handle_projects_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('a') => {
            app.start_org_add(OrgAddType::Project);
        }
        KeyCode::Char('e') => {
            // Edit project (must resolve actual project index from selected row)
            if let Some(idx) = app.projects.table_state.selected() {
                let mut current_idx = 0;
                let mut project_idx = None;
                for (i, project) in app.org_model.project_views.iter().enumerate() {
                    if current_idx == idx {
                        project_idx = Some(i);
                        break;
                    }
                    current_idx += 1;
                    if app
                        .projects
                        .expanded_projects
                        .contains(&project.id.to_string())
                    {
                        if let Some(assets) = app.org_model.assets_by_project.get(&project.id) {
                            current_idx += assets.len();
                        } else {
                            current_idx += 1;
                        }
                    }
                }
                if let Some(p_idx) = project_idx {
                    app.start_org_edit(OrgAddType::Project, p_idx);
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.projects_next();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.projects_previous();
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            app.toggle_project_expand();
        }
        KeyCode::Char('r') => {
            app.start_role_assign();
        }
        KeyCode::Char('c') => {
            app.start_contractor_assign();
        }
        _ => {}
    }
}

fn handle_contractor_assigning_mode(app: &mut App, code: KeyCode) {
    use crate::org::ContractorRole;
    use crate::tui::app::ContractorAssignStep;

    match &app.input_mode {
        InputMode::ContractorAssigning(ContractorAssignStep::SelectCompany) => match code {
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            KeyCode::Down => app.org_select_next(),
            KeyCode::Up => app.org_select_prev(),
            KeyCode::Enter => {
                let _ = app.contractor_assign_pick_company();
            }
            KeyCode::Char(c) => {
                app.input.push(c);
                app.update_contractor_assign_matches();
            }
            KeyCode::Backspace => {
                app.input.pop();
                app.update_contractor_assign_matches();
            }
            _ => {}
        },
        InputMode::ContractorAssigning(ContractorAssignStep::SelectRole) => match code {
            KeyCode::Esc => {
                app.input_mode = InputMode::ContractorAssigning(ContractorAssignStep::SelectCompany)
            }
            KeyCode::Char('d') => {
                let _ = app.contractor_assign_submit(ContractorRole::Development);
            }
            KeyCode::Char('o') => {
                let _ = app.contractor_assign_submit(ContractorRole::DevOps);
            }
            KeyCode::Char('g') => {
                let _ = app.contractor_assign_submit(ContractorRole::Design);
            }
            KeyCode::Char('c') => {
                let _ = app.contractor_assign_submit(ContractorRole::Consultancy);
            }
            KeyCode::Char('s') => {
                let _ = app.contractor_assign_submit(ContractorRole::Security);
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_role_assigning_mode(app: &mut App, code: KeyCode) {
    use crate::org::Role;
    use crate::tui::app::RoleAssignStep;

    match &app.input_mode {
        InputMode::RoleAssigning(RoleAssignStep::SelectTarget) => match code {
            KeyCode::Esc => app.input_mode = InputMode::Normal,
            KeyCode::Down => app.org_select_next(),
            KeyCode::Up => app.org_select_prev(),
            KeyCode::Enter => {
                let _ = app.role_assign_pick_target();
            }
            KeyCode::Char(c) => {
                app.input.push(c);
                app.update_role_assign_matches();
            }
            KeyCode::Backspace => {
                app.input.pop();
                app.update_role_assign_matches();
            }
            _ => {}
        },
        InputMode::RoleAssigning(RoleAssignStep::SelectRole) => match code {
            KeyCode::Esc => app.input_mode = InputMode::RoleAssigning(RoleAssignStep::SelectTarget),
            KeyCode::Char('o') => {
                let _ = app.role_assign_submit(Role::Owner);
            }
            KeyCode::Char('m') => {
                let _ = app.role_assign_submit(Role::Manager);
            }
            KeyCode::Char('t') => {
                let _ = app.role_assign_submit(Role::Technician);
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_editing_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            app.start_scan();
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

fn handle_filtering_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Enter | KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) => {
            app.filter.push(c);
            app.update_visible_items();
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.update_visible_items();
        }
        _ => {}
    }
}

fn handle_org_adding_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.cancel_form();
        }
        KeyCode::Tab => {
            app.form_next_field();
        }
        KeyCode::BackTab => {
            app.form_prev_field();
        }
        KeyCode::Down => {
            if !app.form.select_matches.is_empty() {
                app.org_select_next();
            } else {
                app.form_next_field();
            }
        }
        KeyCode::Up => {
            if !app.form.select_matches.is_empty() {
                app.org_select_prev();
            } else {
                app.form_prev_field();
            }
        }
        KeyCode::Enter => {
            // Submit form or pick from list
            let _ = app.submit_org_form();
        }
        KeyCode::Char(c) => {
            app.form_push_char(c);
        }
        KeyCode::Backspace => {
            app.form_pop_char();
        }
        _ => {}
    }
}

fn handle_assigning_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input.clear();
            app.form.select_index = None;
            app.form.select_matches.clear();
        }
        KeyCode::Enter => {
            let _ = app.submit_asset_assignment();
        }
        KeyCode::Down => {
            if !app.form.select_matches.is_empty() {
                app.org_select_next();
            }
        }
        KeyCode::Up => {
            if !app.form.select_matches.is_empty() {
                app.org_select_prev();
            }
        }
        KeyCode::Char(c) => {
            app.assign_push_char(c);
        }
        KeyCode::Backspace => {
            app.assign_pop_char();
        }
        _ => {}
    }
}

fn handle_asset_details_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char('i') => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.asset_details_scroll = app.asset_details_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.asset_details_scroll = app.asset_details_scroll.saturating_sub(1);
        }
        KeyCode::PageDown => {
            app.asset_details_scroll = app.asset_details_scroll.saturating_add(10);
        }
        KeyCode::PageUp => {
            app.asset_details_scroll = app.asset_details_scroll.saturating_sub(10);
        }
        _ => {}
    }
}
