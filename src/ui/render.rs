use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::ui::app::{AppState, UiColumn};

pub fn render_board(frame: &mut Frame<'_>, app: &AppState) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(frame.area());

    for (index, column) in UiColumn::ALL.iter().enumerate() {
        let cards = app.cards_in_column(*column);
        let items = cards
            .into_iter()
            .map(|card| ListItem::new(Line::from(card.title.clone())))
            .collect::<Vec<_>>();

        let list = List::new(items).block(Block::default().title(column.title()).borders(Borders::ALL));
        frame.render_widget(list, chunks[index]);
    }
}
