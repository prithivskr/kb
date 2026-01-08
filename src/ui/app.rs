use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

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
}

#[derive(Debug, Clone)]
pub struct UiCard {
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

impl AppState {
    pub fn seeded() -> Self {
        Self {
            cards: vec![
                UiCard {
                    title: "Buy groceries".to_string(),
                    column: UiColumn::Backlog,
                    tags: vec!["life".to_string()],
                    due_date: None,
                    blocked: false,
                },
                UiCard {
                    title: "Fix login bug".to_string(),
                    column: UiColumn::ThisWeek,
                    tags: vec!["work".to_string(), "p1".to_string()],
                    due_date: NaiveDate::from_ymd_opt(2026, 3, 8),
                    blocked: true,
                },
                UiCard {
                    title: "Write spec".to_string(),
                    column: UiColumn::Today,
                    tags: vec!["work".to_string()],
                    due_date: NaiveDate::from_ymd_opt(2026, 3, 7),
                    blocked: false,
                },
                UiCard {
                    title: "Update deps".to_string(),
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
        self.cards.iter().filter(|card| card.column == column).count()
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
