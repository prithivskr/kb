use chrono::{Duration, Local, NaiveDate};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph};

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
        frame.render_widget(block, board_chunks[index]);
        render_cards_in_column(frame, board_chunks[index], cards);
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

fn render_cards_in_column(frame: &mut Frame<'_>, area: Rect, cards: Vec<&crate::ui::app::UiCard>) {
    let today = Local::now().date_naive();
    let inner = area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });
    let mut y = inner.y;

    for card in cards {
        let card_height = 4;
        if y.saturating_add(card_height) > inner.y.saturating_add(inner.height) {
            break;
        }

        let due_text = card
            .due_date
            .map(|date| format!("Due {}", date.format("%b %-d")))
            .unwrap_or_else(|| "No due date".to_string());
        let tag_text = if card.tags.is_empty() {
            "No tags".to_string()
        } else {
            format!("#{}", card.tags.join(" #"))
        };
        let title = if card.blocked {
            format!("! {}", card.title)
        } else {
            card.title.clone()
        };

        let card_area = Rect::new(inner.x, y, inner.width, card_height);
        let card_widget = Paragraph::new(vec![
            Line::from(title).style(due_date_style(card.due_date, today)),
            Line::from(format!("{due_text}  {tag_text}")),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1))
                .border_style(Style::default().fg(theme::BORDER)),
        )
        .style(Style::default().fg(theme::FG).bg(theme::BG));
        frame.render_widget(card_widget, card_area);
        y = y.saturating_add(card_height);
    }
}
