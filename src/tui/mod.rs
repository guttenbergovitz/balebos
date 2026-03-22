pub mod app;
pub mod model;
pub mod tabs;
pub mod ui;

use crate::storage::JsonlStorage;
use crate::tui::app::{App, InputMode, Tab, OrgPane, OrgAddType};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub fn run(storage: JsonlStorage) -> Result<(), io::Error> {
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
) -> io::Result<()> {
    let mut last_scan_state = false;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match &app.input_mode {
                    InputMode::Normal => handle_normal_mode(app, key.code)?,
                    InputMode::Editing => handle_editing_mode(app, key.code),
                    InputMode::Filtering => handle_filtering_mode(app, key.code),
                    InputMode::OrgAdding(_) => handle_org_adding_mode(app, key.code),
                    InputMode::Assigning => handle_assigning_mode(app, key.code),
                }
            }
        }

        // Detect transition from scanning to finished
        let is_scanning = *app.is_scanning.lock().unwrap();
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

fn handle_normal_mode(app: &mut App, code: KeyCode) -> io::Result<()> {
    match code {
        // Global keys
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('r') => app.refresh()?,

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
        KeyCode::Char(' ') | KeyCode::Enter => app.toggle_expand(),
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        _ => {}
    }
}

fn handle_organization_keys(app: &mut App, code: KeyCode) {
    match code {
        // Pane switching within Organization tab
        KeyCode::Left | KeyCode::Char('h') => {
            // Cycle through panes backwards (simplified: just use next for now)
            app.next_org_pane();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.next_org_pane();
        }
        // Add entity based on current pane
        KeyCode::Char('a') => {
            let add_type = match app.org_pane {
                OrgPane::Persons => OrgAddType::Person,
                OrgPane::Companies => OrgAddType::Company,
                OrgPane::Brands => OrgAddType::Brand,
            };
            app.start_org_add(add_type);
        }
        // Navigation
        KeyCode::Down | KeyCode::Char('j') => {
            let count = app.org_item_count();
            if count > 0 {
                let i = app.org_table_state.selected().unwrap_or(0);
                let next = if i >= count - 1 { 0 } else { i + 1 };
                app.org_table_state.select(Some(next));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let count = app.org_item_count();
            if count > 0 {
                let i = app.org_table_state.selected().unwrap_or(0);
                let prev = if i == 0 { count - 1 } else { i - 1 };
                app.org_table_state.select(Some(prev));
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
        KeyCode::Down | KeyCode::Char('j') => {
            let count = app.org_model.project_views.len();
            if count > 0 {
                let i = app.projects_table_state.selected().unwrap_or(0);
                let next = if i >= count - 1 { 0 } else { i + 1 };
                app.projects_table_state.select(Some(next));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let count = app.org_model.project_views.len();
            if count > 0 {
                let i = app.projects_table_state.selected().unwrap_or(0);
                let prev = if i == 0 { count - 1 } else { i - 1 };
                app.projects_table_state.select(Some(prev));
            }
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            // Toggle expand project (show assigned assets)
            if let Some(idx) = app.projects_table_state.selected() {
                if let Some(project) = app.org_model.project_views.get(idx) {
                    let id_str = project.id.to_string();
                    if app.expanded_projects.contains(&id_str) {
                        app.expanded_projects.remove(&id_str);
                    } else {
                        app.expanded_projects.insert(id_str);
                    }
                }
            }
        }
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
        KeyCode::Tab | KeyCode::Down => {
            app.form_next_field();
        }
        KeyCode::BackTab | KeyCode::Up => {
            app.form_prev_field();
        }
        KeyCode::Enter => {
            // Submit form - if validation fails, form stays open
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
        }
        KeyCode::Enter => {
            let _ = app.submit_asset_assignment();
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        _ => {}
    }
}
