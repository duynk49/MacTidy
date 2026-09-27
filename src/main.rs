#![allow(clippy::collapsible_if)]

use std::error::Error;
use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use MacTidy::app::{ActiveTab, App, AppMode};
use MacTidy::ui;

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Install Panic Hook to ensure terminal is safely restored on panic
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_panic_hook(panic_info);
    }));

    // 2. Initialize Terminal in Raw Mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. Initialize App State and Event Loop
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    // 4. Restore original Terminal state on exit
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("An error occurred: {:?}", err);
    }

    Ok(())
}

/// Main loop handling UI rendering and keyboard event polling
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        // Process messages from background scanner thread
        app.handle_scan_messages();

        // Render UI
        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            break;
        }

        // Poll keyboard events with 50ms timeout (ensures smooth UI and spinner animation)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events, ignore release and repeat
                if key.kind == KeyEventKind::Press {
                    handle_key_event(app, key.code);
                }
            }
        }
    }

    Ok(())
}

/// Handles key events depending on current app mode
fn handle_key_event(app: &mut App, key_code: KeyCode) {
    match app.mode {
        AppMode::Normal => match key_code {
            KeyCode::Char('q') => {
                app.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.move_down();
            }
            KeyCode::Char(' ') => {
                app.toggle_selection();
            }
            KeyCode::Char('a') => {
                app.toggle_all();
            }
            KeyCode::Tab => {
                app.switch_tab();
            }
            KeyCode::Char('1') => {
                app.active_tab = ActiveTab::SystemJunk;
            }
            KeyCode::Char('2') => {
                app.active_tab = ActiveTab::LargeFiles;
            }
            KeyCode::Enter => {
                app.enter_dry_run_preview();
            }
            KeyCode::Char('r') => {
                app.trigger_rescan();
            }
            _ => {}
        },
        AppMode::DryRunPreview => match key_code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.confirm_and_execute_cleaning();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.cancel_dry_run();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.move_down();
            }
            _ => {}
        },
        AppMode::Deleting => {
            // Deleting in progress; temporarily ignore keystrokes for safety
        }
        AppMode::ReportModal => match key_code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
                app.close_report_and_rescan();
            }
            _ => {}
        },
    }
}
