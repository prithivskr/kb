use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::ui::app::{AppState, UiColumn};
use crate::ui::theme;

pub fn render_board(frame: &mut Frame<'_>, app: &AppState) {
    let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(frame.area());
    let board_chunks = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(layout[0]);

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
        frame.render_widget(list, board_chunks[index]);
    }

    let status = format!(
        "[/] search  [t] tags  [?] help  |  Today: {}/3  |  week: {}",
        app.today_wip_count(),
        app.week_range_label()
    );
    let status_bar = Paragraph::new(status).style(Style::default().fg(theme::FG).bg(theme::BG));
    frame.render_widget(status_bar, layout[1]);
}
