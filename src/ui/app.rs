use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc, Weekday};

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
pub struct ArchivedUiCard {
    pub title: String,
    pub tags: Vec<String>,
    pub due_date: Option<NaiveDate>,
    pub done_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ArchivedPopupState {
    pub cards: Vec<ArchivedUiCard>,
    pub scroll: usize,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub all_cards: Vec<UiCard>,
    pub cards: Vec<UiCard>,
    pub active_column: UiColumn,
    pub selected_by_column: [usize; 4],
    pub status_message: Option<String>,
    pub delete_armed: bool,
    pub insert_prompt: Option<InsertPromptState>,
    pub search_prompt: Option<SearchPromptState>,
    pub search_query: Option<String>,
    pub archived_popup: Option<ArchivedPopupState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPlacement {
    End,
    BelowSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertPromptState {
    pub placement: InsertPlacement,
    pub buffer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPromptState {
    pub buffer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    Quit,
    ClearSearch,
    ArchiveDone,
    OpenArchivedPopup,
    ColumnPrev,
    ColumnNext,
    CursorUp,
    CursorDown,
    Search,
    Insert,
    InsertBelow,
    MoveLeft,
    MoveRight,
    ReorderUp,
    ReorderDown,
    DeletePress,
    JumpBacklog,
    JumpThisWeek,
    JumpToday,
    JumpDone,
    JumpTop,
    JumpBottom,
    None,
}

impl AppState {
    pub fn from_domain_cards(cards: Vec<Card>) -> Self {
        let mapped = map_domain_cards(cards);
        let all_cards = mapped.clone();

        Self {
            all_cards,
            cards: mapped,
            active_column: UiColumn::Today,
            selected_by_column: [0, 0, 0, 0],
            status_message: None,
            delete_armed: false,
            insert_prompt: None,
            search_prompt: None,
            search_query: None,
            archived_popup: None,
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

    pub fn this_week_wip_count(&self) -> usize {
        self.cards_in_column(UiColumn::ThisWeek).len()
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
                self.switch_active_column(UiColumn::from_index(next));
                false
            }
            UiAction::ColumnNext => {
                let current = self.active_column.to_index();
                let next = (current + 1) % UiColumn::ALL.len();
                self.switch_active_column(UiColumn::from_index(next));
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
            UiAction::Insert
            | UiAction::Search
            | UiAction::ClearSearch
            | UiAction::ArchiveDone
            | UiAction::OpenArchivedPopup
            | UiAction::InsertBelow
            | UiAction::MoveLeft
            | UiAction::MoveRight
            | UiAction::ReorderUp
            | UiAction::ReorderDown => false,
            UiAction::DeletePress => false,
            UiAction::JumpBacklog
            | UiAction::JumpThisWeek
            | UiAction::JumpToday
            | UiAction::JumpDone
            | UiAction::JumpTop
            | UiAction::JumpBottom => false,
            UiAction::None => false,
        }
    }

    pub fn replace_from_domain_cards(&mut self, cards: Vec<Card>) {
        self.all_cards = map_domain_cards(cards);
        self.apply_search_filter();
        self.reconcile_selection();
    }

    pub fn reconcile_selection(&mut self) {
        for column in UiColumn::ALL {
            let len = self.column_len(column);
            let clamped = if len == 0 {
                0
            } else {
                self.selected_index(column).min(len - 1)
            };
            self.set_selected_index(column, clamped);
        }
    }

    pub fn selected_card_id_active(&self) -> Option<CardId> {
        let column = self.active_column;
        let selected = self.selected_index(column);
        self.cards_in_column(column)
            .get(selected)
            .map(|card| card.id)
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

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    pub fn arm_delete(&mut self) {
        self.delete_armed = true;
    }

    pub fn disarm_delete(&mut self) {
        self.delete_armed = false;
    }

    pub fn start_insert_prompt(&mut self, placement: InsertPlacement) {
        self.insert_prompt = Some(InsertPromptState {
            placement,
            buffer: String::new(),
        });
    }

    pub fn cancel_insert_prompt(&mut self) {
        self.insert_prompt = None;
    }

    pub fn has_insert_prompt(&self) -> bool {
        self.insert_prompt.is_some()
    }

    pub fn push_insert_char(&mut self, ch: char) {
        if let Some(prompt) = &mut self.insert_prompt {
            prompt.buffer.push(ch);
        }
    }

    pub fn pop_insert_char(&mut self) {
        if let Some(prompt) = &mut self.insert_prompt {
            prompt.buffer.pop();
        }
    }

    pub fn submit_insert_prompt(&mut self) -> Option<(InsertPlacement, String)> {
        let prompt = self.insert_prompt.take()?;
        let title = prompt.buffer.trim().to_string();
        Some((prompt.placement, title))
    }

    pub fn insert_prompt_line(&self) -> Option<String> {
        let prompt = self.insert_prompt.as_ref()?;
        let mode = match prompt.placement {
            InsertPlacement::End => "add-end",
            InsertPlacement::BelowSelection => "add-below",
        };
        Some(format!("{} title: {}_", mode, prompt.buffer))
    }

    pub fn start_search_prompt(&mut self) {
        self.search_prompt = Some(SearchPromptState {
            buffer: self.search_query.clone().unwrap_or_default(),
        });
    }

    pub fn cancel_search_prompt(&mut self) {
        self.search_prompt = None;
    }

    pub fn has_search_prompt(&self) -> bool {
        self.search_prompt.is_some()
    }

    pub fn push_search_char(&mut self, ch: char) {
        if let Some(prompt) = &mut self.search_prompt {
            prompt.buffer.push(ch);
        }
    }

    pub fn pop_search_char(&mut self) {
        if let Some(prompt) = &mut self.search_prompt {
            prompt.buffer.pop();
        }
    }

    pub fn submit_search_prompt(&mut self) -> Option<String> {
        let prompt = self.search_prompt.take()?;
        Some(prompt.buffer)
    }

    pub fn search_prompt_line(&self) -> Option<String> {
        let prompt = self.search_prompt.as_ref()?;
        Some(format!(
            "search (/): {}_  (Enter apply, Esc cancel, empty clears)",
            prompt.buffer
        ))
    }

    pub fn set_search_query(&mut self, query: String) {
        let trimmed = query.trim().to_string();
        self.search_query = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        self.apply_search_filter();
        self.reconcile_selection();
    }

    pub fn active_search_label(&self) -> Option<&str> {
        self.search_query.as_deref()
    }

    pub fn open_archived_popup(&mut self, cards: Vec<Card>) {
        self.archived_popup = Some(ArchivedPopupState {
            cards: map_archived_cards(cards),
            scroll: 0,
        });
    }

    pub fn close_archived_popup(&mut self) {
        self.archived_popup = None;
    }

    pub fn has_archived_popup(&self) -> bool {
        self.archived_popup.is_some()
    }

    pub fn archived_popup(&self) -> Option<&ArchivedPopupState> {
        self.archived_popup.as_ref()
    }

    pub fn scroll_archived_popup_up(&mut self) {
        if let Some(popup) = &mut self.archived_popup {
            popup.scroll = popup.scroll.saturating_sub(1);
        }
    }

    pub fn scroll_archived_popup_down(&mut self) {
        if let Some(popup) = &mut self.archived_popup {
            let max_scroll = popup.cards.len().saturating_sub(1);
            popup.scroll = (popup.scroll + 1).min(max_scroll);
        }
    }

    pub fn jump_to_column(&mut self, column: UiColumn) {
        self.switch_active_column(column);
    }

    pub fn jump_top_active(&mut self) {
        self.set_selected_index(self.active_column, 0);
    }

    pub fn jump_bottom_active(&mut self) {
        let len = self.column_len(self.active_column);
        if len == 0 {
            self.set_selected_index(self.active_column, 0);
            return;
        }
        self.set_selected_index(self.active_column, len - 1);
    }

    fn switch_active_column(&mut self, target: UiColumn) {
        let source_row = self.selected_index(self.active_column);
        self.active_column = target;
        let len = self.column_len(target);
        let clamped = if len == 0 { 0 } else { source_row.min(len - 1) };
        self.set_selected_index(target, clamped);
    }

    fn apply_search_filter(&mut self) {
        let Some(query) = self.search_query.as_deref() else {
            self.cards = self.all_cards.clone();
            return;
        };

        let terms = query_terms(query);
        if terms.is_empty() {
            self.cards = self.all_cards.clone();
            return;
        }

        self.cards = self
            .all_cards
            .iter()
            .filter(|card| {
                let searchable = searchable_text(card);
                terms.iter().all(|term| fuzzy_match(&searchable, term))
            })
            .cloned()
            .collect();
    }
}

fn map_domain_cards(cards: Vec<Card>) -> Vec<UiCard> {
    cards
        .into_iter()
        .map(|card| UiCard {
            id: card.id,
            title: card.title,
            column: UiColumn::from_domain(card.column),
            tags: card.tags,
            due_date: card.due_date,
            blocked: card.blocked,
        })
        .collect()
}

fn map_archived_cards(cards: Vec<Card>) -> Vec<ArchivedUiCard> {
    cards
        .into_iter()
        .map(|card| ArchivedUiCard {
            title: card.title,
            tags: card.tags,
            due_date: card.due_date,
            done_at: card.done_at,
        })
        .collect()
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split_whitespace()
        .map(|term| term.trim_start_matches('#').to_string())
        .filter(|term| !term.is_empty())
        .collect()
}

fn searchable_text(card: &UiCard) -> String {
    let due = card
        .due_date
        .map(|date| {
            format!(
                "{} {} {} {}",
                date.format("%Y-%m-%d"),
                date.format("%b %-d"),
                date.format("%B %-d"),
                date.format("%Y%m%d"),
            )
            .to_lowercase()
        })
        .unwrap_or_default();
    let tags = if card.tags.is_empty() {
        String::new()
    } else {
        format!(" {}", card.tags.join(" ").to_lowercase())
    };

    format!("{} {}{}", card.title.to_lowercase(), due, tags)
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    let mut needle_chars = needle.chars();
    let Some(mut current) = needle_chars.next() else {
        return true;
    };

    for ch in haystack.chars() {
        if ch == current {
            if let Some(next) = needle_chars.next() {
                current = next;
            } else {
                return true;
            }
        }
    }

    false
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

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{AppState, UiColumn};
    use crate::domain::{Card, CardId, Column};

    fn card(title: &str, tags: Vec<&str>, due_date: Option<chrono::NaiveDate>) -> Card {
        Card {
            id: CardId::new(),
            title: title.to_string(),
            column: Column::Today,
            position: 0,
            tags: tags.into_iter().map(ToString::to_string).collect(),
            due_date,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            done_at: None,
            archived: false,
            blocked: false,
        }
    }

    #[test]
    fn fuzzy_search_matches_title() {
        let cards = vec![
            card("Write weekly review", vec!["planning"], None),
            card("Buy groceries", vec!["home"], None),
        ];
        let mut app = AppState::from_domain_cards(cards);

        app.set_search_query("wrrv".to_string());

        assert_eq!(app.column_len(UiColumn::Today), 1);
        assert_eq!(
            app.cards_in_column(UiColumn::Today)[0].title,
            "Write weekly review"
        );
    }

    #[test]
    fn fuzzy_search_matches_tags() {
        let cards = vec![
            card("Fix parser", vec!["backend", "urgent"], None),
            card("Email professor", vec!["school"], None),
        ];
        let mut app = AppState::from_domain_cards(cards);

        app.set_search_query("urg".to_string());

        assert_eq!(app.column_len(UiColumn::Today), 1);
        assert_eq!(app.cards_in_column(UiColumn::Today)[0].title, "Fix parser");
    }

    #[test]
    fn fuzzy_search_matches_due_date() {
        let cards = vec![
            card(
                "Pay rent",
                vec!["finance"],
                Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 12).expect("valid date")),
            ),
            card("Water plants", vec!["home"], None),
        ];
        let mut app = AppState::from_domain_cards(cards);

        app.set_search_query("2026-03-12".to_string());

        assert_eq!(app.column_len(UiColumn::Today), 1);
        assert_eq!(app.cards_in_column(UiColumn::Today)[0].title, "Pay rent");
    }

    #[test]
    fn this_week_wip_count_is_scoped_to_this_week_column() {
        let mut first = card("Plan sprint", vec!["planning"], None);
        first.column = Column::ThisWeek;
        let second = card("Review PR", vec!["backend"], None);
        let mut third = card("Retro notes", vec!["team"], None);
        third.column = Column::ThisWeek;

        let app = AppState::from_domain_cards(vec![first, second, third]);
        assert_eq!(app.this_week_wip_count(), 2);
    }
}
