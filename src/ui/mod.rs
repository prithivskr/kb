use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::input_parser::parse_task_input;
use crate::repo::{NewCard, SqliteRepository};
use crate::storage::open_default_connection;

mod app;
mod render;
mod theme;

pub fn run_ui() -> Result<()> {
    let conn = open_default_connection()?;
    let mut repo = SqliteRepository::new(conn)?;
    let mut app = load_board_state(&repo)?;
    let mut terminal = init_terminal()?;
    let result = run_event_loop(&mut terminal, &mut app, &mut repo);
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
    repo: &mut SqliteRepository,
) -> Result<()> {
    let mut pending_g = false;
    loop {
        terminal.draw(|frame| {
            render::render_board(frame, app);
        })?;

        if event::poll(Duration::from_millis(200))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };

            if app.has_insert_prompt() {
                handle_insert_prompt_key(key, app, repo)?;
                continue;
            }

            let action = map_key_to_action(key, &mut pending_g);
            if action == app::UiAction::None {
                continue;
            }
            match handle_action(action, app, repo) {
                Ok(should_quit) if should_quit => return Ok(()),
                Ok(_) => {}
                Err(err) => app.set_status_message(format!("error: {err}")),
            }
        }
    }
}

fn handle_action(
    action: app::UiAction,
    app: &mut app::AppState,
    repo: &mut SqliteRepository,
) -> Result<bool> {
    match action {
        app::UiAction::Insert => {
            app.disarm_delete();
            app.clear_status_message();
            app.start_insert_prompt(app::InsertPlacement::End);
            Ok(false)
        }
        app::UiAction::InsertBelow => {
            app.disarm_delete();
            app.clear_status_message();
            app.start_insert_prompt(app::InsertPlacement::BelowSelection);
            Ok(false)
        }
        app::UiAction::MoveLeft => {
            app.disarm_delete();
            app.clear_status_message();
            handle_move(repo, app, MoveDirection::Left)?;
            Ok(false)
        }
        app::UiAction::MoveRight => {
            app.disarm_delete();
            app.clear_status_message();
            handle_move(repo, app, MoveDirection::Right)?;
            Ok(false)
        }
        app::UiAction::ReorderUp => {
            app.disarm_delete();
            app.clear_status_message();
            handle_reorder(repo, app, ReorderDirection::Up)?;
            Ok(false)
        }
        app::UiAction::ReorderDown => {
            app.disarm_delete();
            app.clear_status_message();
            handle_reorder(repo, app, ReorderDirection::Down)?;
            Ok(false)
        }
        app::UiAction::Reload => {
            app.disarm_delete();
            app.clear_status_message();
            reload_board_state(repo, app)?;
            Ok(false)
        }
        app::UiAction::DeletePress => {
            handle_delete_press(repo, app)?;
            Ok(false)
        }
        app::UiAction::JumpBacklog => {
            app.disarm_delete();
            app.clear_status_message();
            app.jump_to_column(app::UiColumn::Backlog);
            Ok(false)
        }
        app::UiAction::JumpThisWeek => {
            app.disarm_delete();
            app.clear_status_message();
            app.jump_to_column(app::UiColumn::ThisWeek);
            Ok(false)
        }
        app::UiAction::JumpToday => {
            app.disarm_delete();
            app.clear_status_message();
            app.jump_to_column(app::UiColumn::Today);
            Ok(false)
        }
        app::UiAction::JumpDone => {
            app.disarm_delete();
            app.clear_status_message();
            app.jump_to_column(app::UiColumn::Done);
            Ok(false)
        }
        app::UiAction::JumpTop => {
            app.disarm_delete();
            app.clear_status_message();
            app.jump_top_active();
            Ok(false)
        }
        app::UiAction::JumpBottom => {
            app.disarm_delete();
            app.clear_status_message();
            app.jump_bottom_active();
            Ok(false)
        }
        _ => {
            app.disarm_delete();
            app.clear_status_message();
            if app.apply_action(action) {
                return Ok(true);
            }
            Ok(false)
        }
    }
}

fn map_key_to_action(key: KeyEvent, pending_g: &mut bool) -> app::UiAction {
    if matches!(key.code, KeyCode::Char('g')) {
        if *pending_g {
            *pending_g = false;
            return app::UiAction::JumpTop;
        }
        *pending_g = true;
        return app::UiAction::None;
    }

    *pending_g = false;
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app::UiAction::Quit,
        KeyCode::Char('a') => app::UiAction::Insert,
        KeyCode::Char('i') => app::UiAction::InsertBelow,
        KeyCode::Char('H') => app::UiAction::MoveLeft,
        KeyCode::Char('L') => app::UiAction::MoveRight,
        KeyCode::Char('K') => app::UiAction::ReorderUp,
        KeyCode::Char('J') => app::UiAction::ReorderDown,
        KeyCode::Char('1') => app::UiAction::JumpBacklog,
        KeyCode::Char('2') => app::UiAction::JumpThisWeek,
        KeyCode::Char('3') => app::UiAction::JumpToday,
        KeyCode::Char('4') => app::UiAction::JumpDone,
        KeyCode::Char('G') => app::UiAction::JumpBottom,
        KeyCode::Char('R') => app::UiAction::Reload,
        KeyCode::Char('d') => app::UiAction::DeletePress,
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

fn handle_insert_with_title(
    repo: &mut SqliteRepository,
    app: &mut app::AppState,
    title: &str,
) -> Result<()> {
    let column = app.active_column;
    let parsed = parse_task_input(title, Local::now().date_naive());
    let input = NewCard {
        title: parsed.title,
        notes: None,
        column: column.to_domain(),
        position: i64::try_from(app.column_len(column)).expect("column length should fit i64"),
        due_date: parsed.due_date,
        recurrence: None,
    };
    let card = repo.create_card(input)?;
    if !parsed.tags.is_empty() {
        repo.set_tags(card.id, parsed.tags)?;
    }
    reload_board_state(repo, app)?;
    Ok(())
}

fn handle_insert_below_with_title(
    repo: &mut SqliteRepository,
    app: &mut app::AppState,
    title: &str,
) -> Result<()> {
    let column = app.active_column;
    let parsed = parse_task_input(title, Local::now().date_naive());
    let len = app.column_len(column);
    let selected = app.selected_index(column);
    let target = if len == 0 { 0 } else { (selected + 1).min(len) };
    let input = NewCard {
        title: parsed.title,
        notes: None,
        column: column.to_domain(),
        position: i64::try_from(target).expect("target position should fit i64"),
        due_date: parsed.due_date,
        recurrence: None,
    };
    let card = repo.insert_card_at(input)?;
    if !parsed.tags.is_empty() {
        repo.set_tags(card.id, parsed.tags)?;
    }
    app.set_selected_index(column, target);
    reload_board_state(repo, app)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReorderDirection {
    Up,
    Down,
}

fn handle_move(
    repo: &mut SqliteRepository,
    app: &mut app::AppState,
    direction: MoveDirection,
) -> Result<()> {
    let Some(card_id) = app.selected_card_id_active() else {
        return Ok(());
    };

    let current = app.active_column;
    let Some(target) = adjacent_column(current, direction) else {
        return Ok(());
    };

    let target_position =
        i64::try_from(app.column_len(target)).expect("column length should fit i64");
    if target == app::UiColumn::Done && current != app::UiColumn::Done {
        repo.complete_card(card_id, target_position)?;
    } else {
        repo.move_card(card_id, target.to_domain(), target_position)?;
    }

    reload_board_state(repo, app)?;
    app.active_column = target;
    let moved_index = app
        .cards_in_column(target)
        .iter()
        .position(|card| card.id == card_id);
    if let Some(index) = moved_index {
        app.set_selected_index(target, index);
    } else {
        app.jump_to_column(target);
    }
    Ok(())
}

fn adjacent_column(column: app::UiColumn, direction: MoveDirection) -> Option<app::UiColumn> {
    match (column, direction) {
        (app::UiColumn::Backlog, MoveDirection::Left) => None,
        (app::UiColumn::Done, MoveDirection::Right) => None,
        (col, MoveDirection::Left) => Some(app::UiColumn::from_index(col.to_index() - 1)),
        (col, MoveDirection::Right) => Some(app::UiColumn::from_index(col.to_index() + 1)),
    }
}

fn handle_reorder(
    repo: &mut SqliteRepository,
    app: &mut app::AppState,
    direction: ReorderDirection,
) -> Result<()> {
    let column = app.active_column;
    let len = app.column_len(column);
    if len < 2 {
        return Ok(());
    }
    let Some(card_id) = app.selected_card_id_active() else {
        return Ok(());
    };

    let current = app.selected_index(column);
    let target = match direction {
        ReorderDirection::Up => current.saturating_sub(1),
        ReorderDirection::Down => (current + 1).min(len - 1),
    };
    if target == current {
        return Ok(());
    }

    repo.move_card(
        card_id,
        column.to_domain(),
        i64::try_from(target).expect("selection index should fit i64"),
    )?;
    app.set_selected_index(column, target);
    reload_board_state(repo, app)?;
    Ok(())
}

fn handle_delete_press(repo: &mut SqliteRepository, app: &mut app::AppState) -> Result<()> {
    if !app.delete_armed {
        app.arm_delete();
        app.set_status_message("press d again to delete selected card");
        return Ok(());
    }

    app.disarm_delete();
    let Some(card_id) = app.selected_card_id_active() else {
        app.set_status_message("no selected card to delete");
        return Ok(());
    };
    repo.delete_card(card_id)?;
    app.clear_status_message();
    reload_board_state(repo, app)?;
    Ok(())
}

fn handle_insert_prompt_key(
    key: KeyEvent,
    app: &mut app::AppState,
    repo: &mut SqliteRepository,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.cancel_insert_prompt();
            app.set_status_message("insert canceled");
        }
        KeyCode::Backspace => {
            app.pop_insert_char();
        }
        KeyCode::Enter => {
            let Some((placement, title)) = app.submit_insert_prompt() else {
                return Ok(());
            };
            if title.is_empty() {
                app.start_insert_prompt(placement);
                app.set_status_message("title cannot be empty");
                return Ok(());
            }
            app.clear_status_message();
            match placement {
                app::InsertPlacement::End => handle_insert_with_title(repo, app, &title)?,
                app::InsertPlacement::BelowSelection => {
                    handle_insert_below_with_title(repo, app, &title)?
                }
            }
        }
        KeyCode::Char(ch) => {
            if !ch.is_control() {
                app.push_insert_char(ch);
            }
        }
        _ => {}
    }
    Ok(())
}
