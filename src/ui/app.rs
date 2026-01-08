use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

use crate::domain::{Card, CardId, Column};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiColumn {
    Backlog,
    ThisWeek,
    Today,
    Done,
}

impl UiColumn {
    pub const ALL: [UiColumn; 4] = [
        UiColumn::Backlog,
        UiColumn::ThisWeek,
        UiColumn::Today,
        UiColumn::Done,
    ];

    pub fn title(self) -> &'static str {
        match self {
            UiColumn::Backlog => "Backlog",
            UiColumn::ThisWeek => "This Week",
            UiColumn::Today => "Today",
            UiColumn::Done => "Done",
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            UiColumn::Backlog => 0,
            UiColumn::ThisWeek => 1,
            UiColumn::Today => 2,
            UiColumn::Done => 3,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => UiColumn::Backlog,
            1 => UiColumn::ThisWeek,
            2 => UiColumn::Today,
            _ => UiColumn::Done,
        }
    }

    pub fn to_domain(self) -> Column {
        match self {
            UiColumn::Backlog => Column::Backlog,
            UiColumn::ThisWeek => Column::ThisWeek,
            UiColumn::Today => Column::Today,
            UiColumn::Done => Column::Done,
        }
    }

    pub fn from_domain(column: Column) -> Self {
        match column {
            Column::Backlog => UiColumn::Backlog,
            Column::ThisWeek => UiColumn::ThisWeek,
            Column::Today => UiColumn::Today,
            Column::Done => UiColumn::Done,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiCard {
    pub id: CardId,
    pub title: String,
    pub column: UiColumn,
    pub tags: Vec<String>,
    pub due_date: Option<NaiveDate>,
    pub blocked: bool,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub cards: Vec<UiCard>,
    pub active_column: UiColumn,
    pub selected_by_column: [usize; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    Quit,
    ColumnPrev,
    ColumnNext,
    CursorUp,
    CursorDown,
    None,
}

impl AppState {
    pub fn seeded() -> Self {
        Self {
            cards: vec![
                UiCard {
                    id: CardId::new(),
                    title: "Buy groceries".to_string(),
                    column: UiColumn::Backlog,
                    tags: vec!["life".to_string()],
                    due_date: None,
                    blocked: false,
                },
                UiCard {
                    id: CardId::new(),
                    title: "Fix login bug".to_string(),
                    column: UiColumn::ThisWeek,
                    tags: vec!["work".to_string(), "p1".to_string()],
                    due_date: NaiveDate::from_ymd_opt(2026, 3, 8),
                    blocked: true,
                },
                UiCard {
                    id: CardId::new(),
                    title: "Write spec".to_string(),
                    column: UiColumn::Today,
                    tags: vec!["work".to_string()],
                    due_date: NaiveDate::from_ymd_opt(2026, 3, 7),
                    blocked: false,
                },
                UiCard {
                    id: CardId::new(),
                    title: "Update deps".to_string(),
                    column: UiColumn::Done,
                    tags: vec![],
                    due_date: None,
                    blocked: false,
                },
                UiCard {
                    id: CardId::new(),
                    title: "Update more deps".to_string(),
                    column: UiColumn::Done,
                    tags: vec![],
                    due_date: None,
                    blocked: false,
                },
            ],
            active_column: UiColumn::Today,
            selected_by_column: [0, 0, 0, 0],
        }
    }

    pub fn from_domain_cards(cards: Vec<Card>) -> Self {
        let mapped = cards
            .into_iter()
            .map(|card| UiCard {
                id: card.id,
                title: card.title,
                column: UiColumn::from_domain(card.column),
                tags: card.tags,
                due_date: card.due_date,
                blocked: card.blocked,
            })
            .collect();

        Self {
            cards: mapped,
            active_column: UiColumn::Today,
            selected_by_column: [0, 0, 0, 0],
        }
    }

    pub fn cards_in_column(&self, column: UiColumn) -> Vec<&UiCard> {
        self.cards
            .iter()
            .filter(|card| card.column == column)
            .collect()
    }

    pub fn today_wip_count(&self) -> usize {
        self.cards_in_column(UiColumn::Today).len()
    }

    pub fn selected_index(&self, column: UiColumn) -> usize {
        self.selected_by_column[column.to_index()]
    }

    pub fn set_selected_index(&mut self, column: UiColumn, index: usize) {
        let idx = column.to_index();
        self.selected_by_column[idx] = index;
    }

    pub fn column_len(&self, column: UiColumn) -> usize {
        self.cards
            .iter()
            .filter(|card| card.column == column)
            .count()
    }

    pub fn move_selection_down_active(&mut self) {
        let column = self.active_column;
        let len = self.column_len(column);
        if len == 0 {
            return;
        }

        let current = self.selected_index(column);
        let next = (current + 1).min(len - 1);
        self.set_selected_index(column, next);
    }

    pub fn move_selection_up_active(&mut self) {
        let column = self.active_column;
        let len = self.column_len(column);
        if len == 0 {
            return;
        }

        let current = self.selected_index(column);
        let next = current.saturating_sub(1);
        self.set_selected_index(column, next);
    }

    pub fn apply_action(&mut self, action: UiAction) -> bool {
        match action {
            UiAction::Quit => true,
            UiAction::ColumnPrev => {
                let current = self.active_column.to_index();
                let next = (current + 3) % UiColumn::ALL.len();
                self.active_column = UiColumn::from_index(next);
                false
            }
            UiAction::ColumnNext => {
                let current = self.active_column.to_index();
                let next = (current + 1) % UiColumn::ALL.len();
                self.active_column = UiColumn::from_index(next);
                false
            }
            UiAction::CursorUp => {
                self.move_selection_up_active();
                false
            }
            UiAction::CursorDown => {
                self.move_selection_down_active();
                false
            }
            UiAction::None => false,
        }
    }

    pub fn week_range_label(&self) -> String {
        let today = Local::now().date_naive();
        let offset = i64::from(weekday_from_monday(today.weekday()) - 1);
        let week_start = today - Duration::days(offset);
        let week_end = week_start + Duration::days(6);
        format!(
            "{}-{}, {}",
            week_start.format("%b %-d"),
            week_end.format("%b %-d"),
            week_start.year()
        )
    }
}

fn weekday_from_monday(day: Weekday) -> u32 {
    match day {
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
        Weekday::Sun => 7,
    }
}
