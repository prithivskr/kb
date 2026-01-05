//! Domain models for Kanban entities.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardId(pub Uuid);

impl CardId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CardId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for CardId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for CardId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Column {
    Backlog,
    ThisWeek,
    Today,
    Done,
}

impl Column {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::ThisWeek => "ThisWeek",
            Self::Today => "Today",
            Self::Done => "Done",
        }
    }
}

impl Display for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Column {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Backlog" => Ok(Self::Backlog),
            "ThisWeek" => Ok(Self::ThisWeek),
            "Today" => Ok(Self::Today),
            "Done" => Ok(Self::Done),
            _ => Err(format!("invalid column: {s}")),
        }
    }
}

impl ToSql for Column {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for Column {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let raw = value.as_str()?;
        raw.parse()
            .map_err(|_| FromSqlError::Other(format!("invalid column value: {raw}").into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mon => "Mon",
            Self::Tue => "Tue",
            Self::Wed => "Wed",
            Self::Thu => "Thu",
            Self::Fri => "Fri",
            Self::Sat => "Sat",
            Self::Sun => "Sun",
        }
    }
}

impl Display for Weekday {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Weekday {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Mon" => Ok(Self::Mon),
            "Tue" => Ok(Self::Tue),
            "Wed" => Ok(Self::Wed),
            "Thu" => Ok(Self::Thu),
            "Fri" => Ok(Self::Fri),
            "Sat" => Ok(Self::Sat),
            "Sun" => Ok(Self::Sun),
            _ => Err(format!("invalid weekday: {s}")),
        }
    }
}

impl ToSql for Weekday {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for Weekday {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let raw = value.as_str()?;
        raw.parse()
            .map_err(|_| FromSqlError::Other(format!("invalid weekday value: {raw}").into()))
    }
}

pub const MAX_TITLE_LEN: usize = 200;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("title is required")]
    EmptyTitle,
    #[error("title exceeds {MAX_TITLE_LEN} characters")]
    TitleTooLong,
    #[error("recurrence interval must be >= 1")]
    InvalidRecurrenceInterval,
    #[error("weekly recurrence requires at least one weekday and no day_of_month")]
    InvalidWeeklyRecurrence,
    #[error("monthly recurrence requires day_of_month in 1..=31 and no weekdays")]
    InvalidMonthlyRecurrence,
    #[error("daily recurrence cannot set weekdays or day_of_month")]
    InvalidDailyRecurrence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub title: String,
    pub notes: Option<String>,
    pub column: Column,
    pub position: i64,
    pub tags: Vec<String>,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub done_at: Option<DateTime<Utc>>,
    pub archived: bool,
    pub recurrence: Option<RecurrenceRule>,
    pub blocked: bool,
}

impl Card {
    pub fn new(
        id: CardId,
        title: impl Into<String>,
        column: Column,
        position: i64,
        now: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        let title = validate_title(title.into())?;

        Ok(Self {
            id,
            title,
            notes: None,
            column,
            position,
            tags: Vec::new(),
            due_date: None,
            created_at: now,
            updated_at: now,
            done_at: None,
            archived: false,
            recurrence: None,
            blocked: false,
        })
    }

    pub fn set_title(&mut self, title: impl Into<String>, now: DateTime<Utc>) -> Result<(), ValidationError> {
        self.title = validate_title(title.into())?;
        self.updated_at = now;
        Ok(())
    }

    pub fn set_notes(&mut self, notes: Option<String>, now: DateTime<Utc>) {
        self.notes = notes;
        self.updated_at = now;
    }

    pub fn set_due_date(&mut self, due_date: Option<NaiveDate>, now: DateTime<Utc>) {
        self.due_date = due_date;
        self.updated_at = now;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceFrequency {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceRule {
    pub frequency: RecurrenceFrequency,
    pub interval: i64,
    pub days_of_week: Option<Vec<Weekday>>,
    pub day_of_month: Option<u8>,
}

impl RecurrenceRule {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.interval < 1 {
            return Err(ValidationError::InvalidRecurrenceInterval);
        }
        match self.frequency {
            RecurrenceFrequency::Daily => {
                if self.days_of_week.is_some() || self.day_of_month.is_some() {
                    return Err(ValidationError::InvalidDailyRecurrence);
                }
            }
            RecurrenceFrequency::Weekly => {
                let Some(days) = &self.days_of_week else {
                    return Err(ValidationError::InvalidWeeklyRecurrence);
                };
                if days.is_empty() || self.day_of_month.is_some() {
                    return Err(ValidationError::InvalidWeeklyRecurrence);
                }
            }
            RecurrenceFrequency::Monthly => {
                if self.days_of_week.is_some() {
                    return Err(ValidationError::InvalidMonthlyRecurrence);
                }
                let Some(day) = self.day_of_month else {
                    return Err(ValidationError::InvalidMonthlyRecurrence);
                };
                if !(1..=31).contains(&day) {
                    return Err(ValidationError::InvalidMonthlyRecurrence);
                }
            }
        }
        Ok(())
    }
}

fn validate_title(raw: String) -> Result<String, ValidationError> {
    let title = raw.trim().to_owned();
    if title.is_empty() {
        return Err(ValidationError::EmptyTitle);
    }
    if title.chars().count() > MAX_TITLE_LEN {
        return Err(ValidationError::TitleTooLong);
    }
    Ok(title)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        Card, CardId, Column, RecurrenceFrequency, RecurrenceRule, ValidationError, Weekday,
    };
    use std::str::FromStr;

    #[test]
    fn column_round_trip_string() {
        for value in ["Backlog", "ThisWeek", "Today", "Done"] {
            let parsed = Column::from_str(value).expect("column should parse");
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn weekday_round_trip_string() {
        for value in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            let parsed = Weekday::from_str(value).expect("weekday should parse");
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn card_title_validation_rejects_empty() {
        let result = Card::new(CardId::new(), "   ", Column::Backlog, 0, Utc::now());
        assert_eq!(result.expect_err("title should fail"), ValidationError::EmptyTitle);
    }

    #[test]
    fn card_title_validation_rejects_over_200() {
        let title = "a".repeat(201);
        let result = Card::new(CardId::new(), title, Column::Backlog, 0, Utc::now());
        assert_eq!(
            result.expect_err("title should fail"),
            ValidationError::TitleTooLong
        );
    }

    #[test]
    fn weekly_recurrence_requires_days() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 1,
            days_of_week: Some(vec![]),
            day_of_month: None,
        };
        assert_eq!(
            rule.validate().expect_err("weekly rule should fail"),
            ValidationError::InvalidWeeklyRecurrence
        );
    }

    #[test]
    fn monthly_recurrence_requires_day_of_month() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            days_of_week: None,
            day_of_month: Some(32),
        };
        assert_eq!(
            rule.validate().expect_err("monthly rule should fail"),
            ValidationError::InvalidMonthlyRecurrence
        );
    }
}
