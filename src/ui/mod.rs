use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::repo::SqliteRepository;
use crate::storage::open_default_connection;

mod app;
mod render;
mod theme;

pub fn run_ui() -> Result<()> {
    let conn = open_default_connection()?;
    let repo = SqliteRepository::new(conn)?;
    let mut app = load_board_state(&repo)?;
    let mut terminal = init_terminal()?;
    let result = run_event_loop(&mut terminal, &mut app);
    restore_terminal(terminal)?;
    result
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut app::AppState,
) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            render::render_board(frame, app);
        })?;

        if event::poll(Duration::from_millis(200))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            let action = map_key_to_action(key);
            if app.apply_action(action) {
                return Ok(());
            }
        }
    }
}

fn map_key_to_action(key: KeyEvent) -> app::UiAction {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app::UiAction::Quit,
        KeyCode::Char('h') | KeyCode::BackTab => app::UiAction::ColumnPrev,
        KeyCode::Char('l') | KeyCode::Tab => app::UiAction::ColumnNext,
        KeyCode::Char('j') => app::UiAction::CursorDown,
        KeyCode::Char('k') => app::UiAction::CursorUp,
        _ => app::UiAction::None,
    }
}

fn load_board_state(repo: &SqliteRepository) -> Result<app::AppState> {
    let mut cards = Vec::new();
    for column in app::UiColumn::ALL {
        cards.extend(repo.list_cards_in_column(column.to_domain())?);
    }
    Ok(app::AppState::from_domain_cards(cards))
}

fn reload_board_state(repo: &SqliteRepository, app: &mut app::AppState) -> Result<()> {
    let mut cards = Vec::new();
    for column in app::UiColumn::ALL {
        cards.extend(repo.list_cards_in_column(column.to_domain())?);
    }
    app.replace_from_domain_cards(cards);
    Ok(())
}
