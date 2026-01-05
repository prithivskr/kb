//! Domain models for Kanban entities.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
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

#[cfg(test)]
mod tests {
    use super::{Column, Weekday};
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
}
