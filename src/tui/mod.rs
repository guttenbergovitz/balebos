pub mod app;
pub mod model;
pub mod ui;

use crate::storage::JsonlStorage;
use crate::tui::app::{App, InputMode};
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
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('r') => app.refresh()?,
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
                        KeyCode::Char(' ') | KeyCode::Enter => app.toggle_expand(),
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
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
                    },
                    InputMode::Filtering => match key.code {
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
                    },
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
