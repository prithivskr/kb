use chrono::NaiveDate;

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
        }
    }
}
