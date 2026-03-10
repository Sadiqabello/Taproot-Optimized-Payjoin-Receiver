pub mod app;
mod event;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::KeyCode;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, InputField, Screen};
use event::{AppEvent, is_quit, poll};

use crate::config::Config;

/// Run the interactive TUI.
pub async fn run(config: Config) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize app state
    let mut app = App::new(config);
    app.refresh_data();

    let tick_rate = Duration::from_millis(250);

    // Main loop
    loop {
        // Draw
        terminal.draw(|f| ui::render(f, &app))?;

        // Handle events
        match poll(tick_rate)? {
            AppEvent::Key(key) => {
                if is_quit(&key) && !app.editing {
                    break;
                }
                handle_key(&mut app, key.code).await;
            }
            AppEvent::Tick => {
                app.tick();
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_key(app: &mut App, key: KeyCode) {
    // If editing a form field, handle text input
    if app.editing {
        match key {
            KeyCode::Esc => {
                app.editing = false;
            }
            KeyCode::Enter => {
                app.editing = false;
                // Move to next field
                app.next_field();
            }
            KeyCode::Backspace => {
                app.delete_char();
            }
            KeyCode::Char(c) => {
                app.insert_char(c);
            }
            KeyCode::Tab => {
                app.editing = false;
                app.next_field();
            }
            KeyCode::BackTab => {
                app.editing = false;
                app.prev_field();
            }
            _ => {}
        }
        return;
    }

    // Normal mode key handling
    match app.screen {
        Screen::Dashboard => match key {
            KeyCode::Char('1') => app.screen = Screen::Dashboard,
            KeyCode::Char('2') => app.screen = Screen::NewSession,
            KeyCode::Char('3') => {
                app.refresh_sessions();
                app.screen = Screen::Sessions;
            }
            KeyCode::Char('?') => app.screen = Screen::Help,
            KeyCode::Char('r') => app.refresh_data(),
            _ => {}
        },
        Screen::NewSession => match key {
            KeyCode::Char('1') => app.screen = Screen::Dashboard,
            KeyCode::Char('2') => {}
            KeyCode::Char('3') => {
                app.refresh_sessions();
                app.screen = Screen::Sessions;
            }
            KeyCode::Char('?') => app.screen = Screen::Help,
            KeyCode::Char('r') => app.refresh_data(),
            KeyCode::Esc => app.screen = Screen::Dashboard,
            KeyCode::Tab | KeyCode::Down => app.next_field(),
            KeyCode::BackTab | KeyCode::Up => app.prev_field(),
            KeyCode::Enter => {
                if app.active_field == InputField::Amount && app.input_amount.is_empty() {
                    // Start editing the amount field
                    app.editing = true;
                } else if !app.input_amount.is_empty() {
                    // Start the session
                    app.start_session().await;
                } else {
                    app.editing = true;
                }
            }
            KeyCode::Char(c) => {
                // Start editing on any character input
                app.editing = true;
                app.insert_char(c);
            }
            _ => {}
        },
        Screen::Sessions => match key {
            KeyCode::Char('1') => app.screen = Screen::Dashboard,
            KeyCode::Char('2') => app.screen = Screen::NewSession,
            KeyCode::Char('3') => {}
            KeyCode::Char('?') => app.screen = Screen::Help,
            KeyCode::Char('r') => {
                app.refresh_sessions();
                app.refresh_data();
            }
            KeyCode::Esc => app.screen = Screen::Dashboard,
            _ => {}
        },
        Screen::Help => match key {
            KeyCode::Char('1') => app.screen = Screen::Dashboard,
            KeyCode::Char('2') => app.screen = Screen::NewSession,
            KeyCode::Char('3') => {
                app.refresh_sessions();
                app.screen = Screen::Sessions;
            }
            KeyCode::Esc | KeyCode::Char('?') => app.screen = Screen::Dashboard,
            _ => {}
        },
    }
}
