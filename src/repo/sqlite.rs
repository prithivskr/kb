use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::domain::{validate_card_title, Card, CardId, Column};
use crate::storage::run_migrations;

#[derive(Debug, Clone)]
pub struct NewCard {
    pub title: String,
    pub notes: Option<String>,
    pub column: Column,
    pub position: i64,
    pub due_date: Option<NaiveDate>,
}

pub struct SqliteRepository {
    conn: Connection,
}

impl SqliteRepository {
    pub fn new(mut conn: Connection) -> Result<Self> {
        run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn create_card(&mut self, input: NewCard) -> Result<Card> {
        let now = Utc::now();
        let mut card = Card::new(CardId::new(), input.title, input.column, input.position, now)?;
        card.notes = input.notes;
        card.due_date = input.due_date;

        self.conn.execute(
            "INSERT INTO cards(
                id, title, notes, column, position, due_date, created_at, updated_at, done_at, archived, blocked
            ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                card.id.to_string(),
                card.title,
                card.notes,
                card.column,
                card.position,
                card.due_date.map(|date| date.format("%Y-%m-%d").to_string()),
                card.created_at.to_rfc3339(),
                card.updated_at.to_rfc3339(),
                card.done_at.map(|dt| dt.to_rfc3339()),
                bool_to_int(card.archived),
                bool_to_int(card.blocked),
            ],
        )
        .context("failed to insert card")?;

        Ok(card)
    }

    pub fn get_card(&self, id: CardId) -> Result<Option<Card>> {
        self.conn
            .query_row(
                "SELECT
                    id, title, notes, column, position, due_date,
                    created_at, updated_at, done_at, archived, blocked
                 FROM cards
                 WHERE id = ?1",
                [id.to_string()],
                row_to_card,
            )
            .optional()
            .context("failed to fetch card")
    }

    pub fn list_cards_in_column(&self, column: Column) -> Result<Vec<Card>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT
                    id, title, notes, column, position, due_date,
                    created_at, updated_at, done_at, archived, blocked
                 FROM cards
                 WHERE column = ?1 AND archived = 0
                 ORDER BY position ASC, created_at ASC",
            )
            .context("failed to prepare list_cards_in_column statement")?;

        let iter = stmt
            .query_map([column], row_to_card)
            .context("failed listing cards in column")?;

        let cards: rusqlite::Result<Vec<Card>> = iter.collect();
        Ok(cards.context("failed parsing cards from query")?)
    }

    pub fn update_title(&mut self, id: CardId, title: impl Into<String>) -> Result<()> {
        let title = validate_card_title(title.into())?;
        let now = Utc::now().to_rfc3339();
        let updated = self
            .conn
            .execute(
                "UPDATE cards SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, id.to_string()],
            )
            .context("failed to update title")?;
        ensure_row_updated(updated, id)
    }

    pub fn update_notes(&mut self, id: CardId, notes: Option<String>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let updated = self
            .conn
            .execute(
                "UPDATE cards SET notes = ?1, updated_at = ?2 WHERE id = ?3",
                params![notes, now, id.to_string()],
            )
            .context("failed to update notes")?;
        ensure_row_updated(updated, id)
    }

    pub fn update_due_date(&mut self, id: CardId, due_date: Option<NaiveDate>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let due_date = due_date.map(|date| date.format("%Y-%m-%d").to_string());
        let updated = self
            .conn
            .execute(
                "UPDATE cards SET due_date = ?1, updated_at = ?2 WHERE id = ?3",
                params![due_date, now, id.to_string()],
            )
            .context("failed to update due date")?;
        ensure_row_updated(updated, id)
    }

    pub fn set_blocked(&mut self, id: CardId, blocked: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let updated = self
            .conn
            .execute(
                "UPDATE cards SET blocked = ?1, updated_at = ?2 WHERE id = ?3",
                params![bool_to_int(blocked), now, id.to_string()],
            )
            .context("failed to update blocked flag")?;
        ensure_row_updated(updated, id)
    }

    pub fn move_card(&mut self, id: CardId, target_column: Column, target_position: i64) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("failed to begin move transaction")?;
        let (source_column, source_position) = fetch_card_location(&tx, id)?
            .with_context(|| format!("card not found for move: {id}"))?;
        let now = Utc::now().to_rfc3339();

        if source_column == target_column {
            reorder_within_column(&tx, id, source_column, source_position, target_position, &now)?;
        } else {
            tx.execute(
                "UPDATE cards
                 SET position = position - 1
                 WHERE column = ?1 AND position > ?2",
                params![source_column, source_position],
            )
            .context("failed compacting source column positions")?;

            tx.execute(
                "UPDATE cards
                 SET position = position + 1
                 WHERE column = ?1 AND position >= ?2",
                params![target_column, target_position],
            )
            .context("failed opening target position in target column")?;

            let done_at = if target_column == Column::Done {
                Some(now.clone())
            } else {
                None
            };
            tx.execute(
                "UPDATE cards
                 SET column = ?1, position = ?2, updated_at = ?3, done_at = ?4
                 WHERE id = ?5",
                params![target_column, target_position, now, done_at, id.to_string()],
            )
            .context("failed moving card between columns")?;
        }

        tx.commit().context("failed to commit move transaction")?;
        Ok(())
    }
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

fn parse_card_id(raw: String) -> rusqlite::Result<CardId> {
    CardId::from_str(&raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}

fn parse_datetime(raw: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}

fn parse_optional_datetime(raw: Option<String>) -> rusqlite::Result<Option<DateTime<Utc>>> {
    raw.map(parse_datetime).transpose()
}

fn parse_optional_date(raw: Option<String>) -> rusqlite::Result<Option<NaiveDate>> {
    raw.map(|value| {
        NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err))
        })
    })
    .transpose()
}

fn row_to_card(row: &Row<'_>) -> rusqlite::Result<Card> {
    let id = parse_card_id(row.get::<_, String>(0)?)?;
    let title = row.get(1)?;
    let notes = row.get(2)?;
    let column = row.get(3)?;
    let position = row.get(4)?;
    let due_date = parse_optional_date(row.get(5)?)?;
    let created_at = parse_datetime(row.get(6)?)?;
    let updated_at = parse_datetime(row.get(7)?)?;
    let done_at = parse_optional_datetime(row.get(8)?)?;
    let archived = int_to_bool(row.get(9)?);
    let blocked = int_to_bool(row.get(10)?);

    Ok(Card {
        id,
        title,
        notes,
        column,
        position,
        tags: Vec::new(),
        due_date,
        created_at,
        updated_at,
        done_at,
        archived,
        recurrence: None,
        blocked,
    })
}

fn ensure_row_updated(updated: usize, id: CardId) -> Result<()> {
    if updated == 1 {
        Ok(())
    } else {
        anyhow::bail!("card not found: {id}");
    }
}

fn fetch_card_location(tx: &Transaction<'_>, id: CardId) -> Result<Option<(Column, i64)>> {
    let result = tx
        .query_row(
            "SELECT column, position FROM cards WHERE id = ?1",
            [id.to_string()],
            |row| {
                let column: Column = row.get(0)?;
                let position: i64 = row.get(1)?;
                Ok((column, position))
            },
        )
        .optional()
        .context("failed fetching card location")?;
    Ok(result)
}

fn reorder_within_column(
    tx: &Transaction<'_>,
    id: CardId,
    column: Column,
    source_position: i64,
    target_position: i64,
    now: &str,
) -> Result<()> {
    if target_position == source_position {
        return Ok(());
    }

    if target_position > source_position {
        tx.execute(
            "UPDATE cards
             SET position = position - 1
             WHERE column = ?1 AND position > ?2 AND position <= ?3",
            params![column, source_position, target_position],
        )
        .context("failed shifting cards upward within column")?;
    } else {
        tx.execute(
            "UPDATE cards
             SET position = position + 1
             WHERE column = ?1 AND position >= ?2 AND position < ?3",
            params![column, target_position, source_position],
        )
        .context("failed shifting cards downward within column")?;
    }

    tx.execute(
        "UPDATE cards SET position = ?1, updated_at = ?2 WHERE id = ?3",
        params![target_position, now, id.to_string()],
    )
    .context("failed updating moved card position")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rusqlite::Connection;

    use crate::domain::Column;

    use super::{NewCard, SqliteRepository};

    #[test]
    fn create_get_and_list_cards() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let first = repo
            .create_card(NewCard {
                title: "Card A".to_string(),
                notes: None,
                column: Column::Backlog,
                position: 0,
                due_date: Some(NaiveDate::from_ymd_opt(2026, 3, 9).expect("valid date")),
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Card B".to_string(),
                notes: Some("note".to_string()),
                column: Column::Backlog,
                position: 1,
                due_date: None,
            })
            .expect("card create should succeed");

        let got = repo
            .get_card(first.id)
            .expect("get should succeed")
            .expect("card should exist");
        assert_eq!(got.title, "Card A");

        let listed = repo
            .list_cards_in_column(Column::Backlog)
            .expect("list should succeed");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[1].id, second.id);
    }

    #[test]
    fn update_and_move_card_operations() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let first = repo
            .create_card(NewCard {
                title: "First".to_string(),
                notes: None,
                column: Column::Backlog,
                position: 0,
                due_date: None,
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Second".to_string(),
                notes: None,
                column: Column::Backlog,
                position: 1,
                due_date: None,
            })
            .expect("card create should succeed");

        repo.update_title(second.id, "Updated title")
            .expect("title update should succeed");
        repo.update_due_date(
            second.id,
            Some(NaiveDate::from_ymd_opt(2026, 3, 10).expect("valid date")),
        )
        .expect("due date update should succeed");
        repo.set_blocked(second.id, true)
            .expect("blocked update should succeed");
        repo.move_card(second.id, Column::Backlog, 0)
            .expect("reorder should succeed");

        let backlog = repo
            .list_cards_in_column(Column::Backlog)
            .expect("list should succeed");
        assert_eq!(backlog[0].id, second.id);
        assert_eq!(backlog[1].id, first.id);

        repo.move_card(second.id, Column::Done, 0)
            .expect("move to done should succeed");
        let done_card = repo
            .get_card(second.id)
            .expect("get should succeed")
            .expect("card should exist");
        assert_eq!(done_card.column, Column::Done);
        assert!(done_card.done_at.is_some());
        assert!(done_card.blocked);
        assert_eq!(done_card.title, "Updated title");
    }
}
