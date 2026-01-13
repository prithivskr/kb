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

pub const MAX_TITLE_LEN: usize = 200;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("title is required")]
    EmptyTitle,
    #[error("title exceeds {MAX_TITLE_LEN} characters")]
    TitleTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub title: String,
    pub column: Column,
    pub position: i64,
    pub tags: Vec<String>,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub done_at: Option<DateTime<Utc>>,
    pub archived: bool,
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
            column,
            position,
            tags: Vec::new(),
            due_date: None,
            created_at: now,
            updated_at: now,
            done_at: None,
            archived: false,
        })
    }

    pub fn set_title(
        &mut self,
        title: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<(), ValidationError> {
        self.title = validate_title(title.into())?;
        self.updated_at = now;
        Ok(())
    }

    pub fn set_due_date(&mut self, due_date: Option<NaiveDate>, now: DateTime<Utc>) {
        self.due_date = due_date;
        self.updated_at = now;
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

pub fn validate_card_title(raw: impl Into<String>) -> Result<String, ValidationError> {
    validate_title(raw.into())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use std::str::FromStr;

    use super::{Card, CardId, Column, ValidationError};

    #[test]
    fn column_round_trip_string() {
        for value in ["Backlog", "ThisWeek", "Today", "Done"] {
            let parsed = Column::from_str(value).expect("column should parse");
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn card_title_validation_rejects_empty() {
        let result = Card::new(CardId::new(), "   ", Column::Backlog, 0, Utc::now());
        assert_eq!(
            result.expect_err("title should fail"),
            ValidationError::EmptyTitle
        );
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
}
