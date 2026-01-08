use chrono::{Duration, Local, NaiveDate};
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
        let today = Local::now().date_naive();
        let cards = app.cards_in_column(*column);
        let items = cards
            .into_iter()
            .map(|card| {
                let due = card
                    .due_date
                    .map(|d| format!(" ({})", d.format("%b %-d")))
                    .unwrap_or_default();
                let blocked = if card.blocked { "! " } else { "" };
                let tag_hint = if card.tags.is_empty() { "" } else { " #" };
                let line = format!("{blocked}{}{due}{tag_hint}", card.title);
                let style = due_date_style(card.due_date, today);
                ListItem::new(Line::from(line)).style(style)
            })
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

fn due_date_style(due_date: Option<NaiveDate>, today: NaiveDate) -> Style {
    let base = Style::default().fg(theme::FG).bg(theme::BG);
    match due_date {
        Some(due) if due < today => base.fg(theme::DUE_OVERDUE),
        Some(due) if due == today => base.fg(theme::DUE_TODAY),
        Some(due) if due <= today + Duration::days(7) => base.fg(theme::DUE_SOON),
        _ => base,
    }
}
