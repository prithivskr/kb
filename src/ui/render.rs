use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Borders};

use crate::ui::app::{AppState, UiColumn};

pub fn render_board(frame: &mut Frame<'_>, _app: &AppState) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(frame.area());

    for (index, column) in UiColumn::ALL.iter().enumerate() {
        let block = Block::default().title(column.title()).borders(Borders::ALL);
        frame.render_widget(block, chunks[index]);
    }
}
