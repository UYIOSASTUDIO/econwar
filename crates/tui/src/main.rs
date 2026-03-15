//! EconWar TUI Client
//!
//! A terminal-based dashboard for playing EconWar.
//! Connect to a running server and manage your economic empire.

mod app;
mod api;
mod ui;
mod commands;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, InputMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse server URL from args or default.
    let server_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    // Setup terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app.
    let mut app = App::new(server_url);
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
    }

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> anyhow::Result<()> {
    // Show welcome message.
    app.log("Welcome to EconWar! Type 'help' for commands, or 'login <user> <pass>' to start.");
    app.log("Press Tab to switch panels, Esc to unfocus, Ctrl+C to quit.");
    app.log("Scroll: PageUp/PageDown (any mode), Up/Down/Home/End (normal mode).");

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Poll for events with a timeout so we can do periodic refreshes.
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C always exits.
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
                {
                    return Ok(());
                }

                // PageUp/PageDown scroll the log in ANY mode.
                match key.code {
                    KeyCode::PageUp => { app.scroll_up_page(); continue; }
                    KeyCode::PageDown => { app.scroll_down_page(); continue; }
                    _ => {}
                }

                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char(':') | KeyCode::Char('/') => {
                            app.input_mode = InputMode::Command;
                        }
                        KeyCode::Tab => {
                            app.next_panel();
                        }
                        KeyCode::BackTab => {
                            app.prev_panel();
                        }
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('?') => {
                            commands::handle_command(app, "help").await;
                        }
                        KeyCode::Up => app.scroll_up(),
                        KeyCode::Down => app.scroll_down(),
                        KeyCode::Home => { app.log_scroll = 0; }
                        KeyCode::End => { app.scroll_to_bottom(); }
                        _ => {}
                    },
                    InputMode::Command => match key.code {
                        KeyCode::Enter => {
                            let input = app.input.drain(..).collect::<String>();
                            if !input.is_empty() {
                                app.command_history.push(input.clone());
                                app.history_index = app.command_history.len();
                                commands::handle_command(app, &input).await;
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            app.input.clear();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Up => {
                            if app.history_index > 0 {
                                app.history_index -= 1;
                                app.input = app.command_history[app.history_index].chars().collect();
                            }
                        }
                        KeyCode::Down => {
                            if app.history_index < app.command_history.len() {
                                app.history_index += 1;
                                if app.history_index < app.command_history.len() {
                                    app.input =
                                        app.command_history[app.history_index].chars().collect();
                                } else {
                                    app.input.clear();
                                }
                            }
                        }
                        _ => {}
                    },
                }
            }
        }

        // Periodic market refresh (every ~5 seconds).
        app.tick_counter += 1;
        if app.tick_counter % 20 == 0 && app.token.is_some() {
            app.refresh_markets().await;
        }
    }
}
