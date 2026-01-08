use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::ui::app::{AppState, UiColumn};
use crate::ui::theme;

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

        let border_style = if *column == app.active_column {
            Style::default().fg(theme::ACTIVE_BORDER)
        } else {
            Style::default().fg(theme::BORDER)
        };
        let block = Block::default()
            .title(Line::from(column.title()).style(theme::title_style()))
            .borders(Borders::ALL)
            .style(Style::default().fg(theme::FG).bg(theme::BG))
            .border_style(border_style);
        let list = List::new(items).block(block);
        frame.render_widget(list, chunks[index]);
    }
}
