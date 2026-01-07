use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::domain::{
    validate_card_title, Card, CardId, Column, RecurrenceFrequency, RecurrenceRule, Weekday,
};
use crate::storage::run_migrations;

#[derive(Debug, Clone)]
pub struct NewCard {
    pub title: String,
    pub notes: Option<String>,
    pub column: Column,
    pub position: i64,
    pub due_date: Option<NaiveDate>,
    pub recurrence: Option<RecurrenceRule>,
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
        card.recurrence = input.recurrence;

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
                card.due_date.map(format_date),
                card.created_at.to_rfc3339(),
                card.updated_at.to_rfc3339(),
                card.done_at.map(|dt| dt.to_rfc3339()),
                bool_to_int(card.archived),
                bool_to_int(card.blocked),
            ],
        )
        .context("failed to insert card")?;

        if let Some(rule) = &card.recurrence {
            upsert_recurrence_rule_conn(&self.conn, card.id, rule)?;
        }

        Ok(card)
    }

    pub fn get_card(&self, id: CardId) -> Result<Option<Card>> {
        let card = self
            .conn
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
            .context("failed to fetch card")?;

        card.map(|raw| self.hydrate_card(raw)).transpose()
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
        cards
            .context("failed parsing cards from query")?
            .into_iter()
            .map(|card| self.hydrate_card(card))
            .collect()
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
        let due_date = due_date.map(format_date);
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
                 WHERE column = ?1 AND archived = 0 AND position > ?2",
                params![source_column, source_position],
            )
            .context("failed compacting source column positions")?;

            tx.execute(
                "UPDATE cards
                 SET position = position + 1
                 WHERE column = ?1 AND archived = 0 AND position >= ?2",
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

    pub fn set_recurrence(&mut self, id: CardId, recurrence: Option<RecurrenceRule>) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("failed to begin recurrence transaction")?;

        let exists = fetch_card_location(&tx, id)?.is_some();
        if !exists {
            anyhow::bail!("card not found: {id}");
        }

        match recurrence {
            Some(rule) => upsert_recurrence_rule_tx(&tx, id, &rule)?,
            None => {
                tx.execute("DELETE FROM recurrence_rules WHERE card_id = ?1", [id.to_string()])
                    .context("failed clearing recurrence rule")?;
            }
        }

        tx.execute(
            "UPDATE cards SET updated_at = ?1 WHERE id = ?2",
            [Utc::now().to_rfc3339(), id.to_string()],
        )
        .context("failed updating card timestamp after recurrence change")?;

        tx.commit()
            .context("failed to commit recurrence transaction")?;
        Ok(())
    }

    pub fn complete_card(&mut self, id: CardId, done_position: i64) -> Result<Option<Card>> {
        let tx = self
            .conn
            .transaction()
            .context("failed to begin completion transaction")?;

        let seed = fetch_completion_seed(&tx, id)?
            .with_context(|| format!("card not found for completion: {id}"))?;
        let recurrence = fetch_recurrence_rule_tx(&tx, id)?;

        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let completed_on = now.date_naive();

        if seed.column == Column::Done {
            reorder_within_column(&tx, id, seed.column, seed.position, done_position, &now_rfc3339)?;
            tx.execute(
                "UPDATE cards SET done_at = COALESCE(done_at, ?1), updated_at = ?1 WHERE id = ?2",
                params![now_rfc3339, id.to_string()],
            )
            .context("failed updating done timestamp for already-done card")?;
        } else {
            tx.execute(
                "UPDATE cards
                 SET position = position - 1
                 WHERE column = ?1 AND archived = 0 AND position > ?2",
                params![seed.column, seed.position],
            )
            .context("failed compacting source column before completion")?;

            tx.execute(
                "UPDATE cards
                 SET position = position + 1
                 WHERE column = ?1 AND archived = 0 AND position >= ?2",
                params![Column::Done, done_position],
            )
            .context("failed shifting done column positions")?;

            tx.execute(
                "UPDATE cards
                 SET column = ?1, position = ?2, updated_at = ?3, done_at = ?4
                 WHERE id = ?5",
                params![
                    Column::Done,
                    done_position,
                    now_rfc3339,
                    now_rfc3339,
                    id.to_string()
                ],
            )
            .context("failed moving completed card to done")?;
        }

        let spawned_id = if let Some(rule) = recurrence {
            let base_date = seed.due_date.unwrap_or(completed_on);
            let mut next_due = rule.next_due_date(base_date)?;
            while next_due <= completed_on {
                next_due = rule.next_due_date(next_due)?;
            }

            let spawn_column = if next_due <= completed_on + Duration::days(7) {
                Column::ThisWeek
            } else {
                Column::Backlog
            };
            let spawn_position = next_position_in_column(&tx, spawn_column)?;
            let spawn_id = CardId::new();

            tx.execute(
                "INSERT INTO cards(
                    id, title, notes, column, position, due_date,
                    created_at, updated_at, done_at, archived, blocked
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, 0, 0)",
                params![
                    spawn_id.to_string(),
                    seed.title,
                    seed.notes,
                    spawn_column,
                    spawn_position,
                    format_date(next_due),
                    now_rfc3339,
                    now_rfc3339,
                ],
            )
            .context("failed creating spawned recurrence card")?;

            tx.execute(
                "INSERT INTO card_tags(card_id, tag_id)
                 SELECT ?1, tag_id FROM card_tags WHERE card_id = ?2",
                params![spawn_id.to_string(), id.to_string()],
            )
            .context("failed copying tags to spawned recurrence card")?;

            upsert_recurrence_rule_tx(&tx, spawn_id, &rule)?;
            Some(spawn_id)
        } else {
            None
        };

        tx.commit()
            .context("failed to commit completion transaction")?;

        spawned_id.map(|spawn_id| self.get_card(spawn_id)).transpose().map(|v| v.flatten())
    }

    fn hydrate_card(&self, mut card: Card) -> Result<Card> {
        card.tags = fetch_tags_for_card_conn(&self.conn, card.id)?;
        card.recurrence = fetch_recurrence_rule_conn(&self.conn, card.id)?;
        Ok(card)
    }
}

#[derive(Debug)]
struct CompletionSeed {
    title: String,
    notes: Option<String>,
    due_date: Option<NaiveDate>,
    column: Column,
    position: i64,
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

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
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
            "SELECT column, position FROM cards WHERE id = ?1 AND archived = 0",
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

fn fetch_completion_seed(tx: &Transaction<'_>, id: CardId) -> Result<Option<CompletionSeed>> {
    let result = tx
        .query_row(
            "SELECT title, notes, due_date, column, position
             FROM cards
             WHERE id = ?1 AND archived = 0",
            [id.to_string()],
            |row| {
                Ok(CompletionSeed {
                    title: row.get(0)?,
                    notes: row.get(1)?,
                    due_date: parse_optional_date(row.get(2)?)?,
                    column: row.get(3)?,
                    position: row.get(4)?,
                })
            },
        )
        .optional()
        .context("failed fetching completion seed")?;
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
             WHERE column = ?1 AND archived = 0 AND position > ?2 AND position <= ?3",
            params![column, source_position, target_position],
        )
        .context("failed shifting cards upward within column")?;
    } else {
        tx.execute(
            "UPDATE cards
             SET position = position + 1
             WHERE column = ?1 AND archived = 0 AND position >= ?2 AND position < ?3",
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

fn upsert_recurrence_rule_conn(conn: &Connection, card_id: CardId, rule: &RecurrenceRule) -> Result<()> {
    rule.validate()?;
    let days_of_week = rule
        .days_of_week
        .as_ref()
        .map(|days| serialize_weekdays(days))
        .transpose()?;

    conn.execute(
        "INSERT INTO recurrence_rules(card_id, frequency, interval, days_of_week, day_of_month)
         VALUES(?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(card_id) DO UPDATE SET
            frequency = excluded.frequency,
            interval = excluded.interval,
            days_of_week = excluded.days_of_week,
            day_of_month = excluded.day_of_month",
        params![
            card_id.to_string(),
            rule.frequency,
            rule.interval,
            days_of_week,
            rule.day_of_month,
        ],
    )
    .context("failed upserting recurrence rule")?;

    Ok(())
}

fn upsert_recurrence_rule_tx(tx: &Transaction<'_>, card_id: CardId, rule: &RecurrenceRule) -> Result<()> {
    rule.validate()?;
    let days_of_week = rule
        .days_of_week
        .as_ref()
        .map(|days| serialize_weekdays(days))
        .transpose()?;

    tx.execute(
        "INSERT INTO recurrence_rules(card_id, frequency, interval, days_of_week, day_of_month)
         VALUES(?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(card_id) DO UPDATE SET
            frequency = excluded.frequency,
            interval = excluded.interval,
            days_of_week = excluded.days_of_week,
            day_of_month = excluded.day_of_month",
        params![
            card_id.to_string(),
            rule.frequency,
            rule.interval,
            days_of_week,
            rule.day_of_month,
        ],
    )
    .context("failed upserting recurrence rule in transaction")?;

    Ok(())
}

fn fetch_recurrence_rule_conn(conn: &Connection, id: CardId) -> Result<Option<RecurrenceRule>> {
    let rule = conn
        .query_row(
            "SELECT frequency, interval, days_of_week, day_of_month
             FROM recurrence_rules
             WHERE card_id = ?1",
            [id.to_string()],
            recurrence_rule_from_row,
        )
        .optional()
        .context("failed fetching recurrence rule")?;
    Ok(rule)
}

fn fetch_recurrence_rule_tx(tx: &Transaction<'_>, id: CardId) -> Result<Option<RecurrenceRule>> {
    let rule = tx
        .query_row(
            "SELECT frequency, interval, days_of_week, day_of_month
             FROM recurrence_rules
             WHERE card_id = ?1",
            [id.to_string()],
            recurrence_rule_from_row,
        )
        .optional()
        .context("failed fetching recurrence rule")?;
    Ok(rule)
}

fn recurrence_rule_from_row(row: &Row<'_>) -> rusqlite::Result<RecurrenceRule> {
    let frequency: RecurrenceFrequency = row.get(0)?;
    let interval: i64 = row.get(1)?;
    let days_raw: Option<String> = row.get(2)?;
    let day_of_month: Option<u8> = row.get(3)?;
    let days_of_week = days_raw
        .map(|raw| deserialize_weekdays(&raw))
        .transpose()
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    err.to_string(),
                )),
            )
        })?;

    let rule = RecurrenceRule {
        frequency,
        interval,
        days_of_week,
        day_of_month,
    };
    rule.validate()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))?;
    Ok(rule)
}

fn serialize_weekdays(days: &[Weekday]) -> Result<String> {
    serde_json::to_string(days).context("failed serializing weekdays")
}

fn deserialize_weekdays(raw: &str) -> Result<Vec<Weekday>> {
    serde_json::from_str(raw).context("failed deserializing weekdays")
}

fn fetch_tags_for_card_conn(conn: &Connection, id: CardId) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN card_tags ct ON ct.tag_id = t.id
             WHERE ct.card_id = ?1
             ORDER BY t.name ASC",
        )
        .context("failed preparing fetch tags statement")?;

    let iter = stmt
        .query_map([id.to_string()], |row| row.get::<_, String>(0))
        .context("failed querying card tags")?;
    let tags: rusqlite::Result<Vec<String>> = iter.collect();
    Ok(tags.context("failed parsing card tags")?)
}

fn next_position_in_column(tx: &Transaction<'_>, column: Column) -> Result<i64> {
    let position = tx
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM cards WHERE column = ?1 AND archived = 0",
            [column],
            |row| row.get(0),
        )
        .context("failed computing next column position")?;
    Ok(position)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, Utc};
    use rusqlite::Connection;

    use crate::domain::{Column, RecurrenceFrequency, RecurrenceRule};

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
                recurrence: None,
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Card B".to_string(),
                notes: Some("note".to_string()),
                column: Column::Backlog,
                position: 1,
                due_date: None,
                recurrence: None,
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
                recurrence: None,
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Second".to_string(),
                notes: None,
                column: Column::Backlog,
                position: 1,
                due_date: None,
                recurrence: None,
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

    #[test]
    fn completing_recurring_card_spawns_next_card() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let today = Utc::now().date_naive();
        let original = repo
            .create_card(NewCard {
                title: "Daily review".to_string(),
                notes: Some("Keep this short".to_string()),
                column: Column::Today,
                position: 0,
                due_date: Some(today),
                recurrence: Some(RecurrenceRule {
                    frequency: RecurrenceFrequency::Daily,
                    interval: 1,
                    days_of_week: None,
                    day_of_month: None,
                }),
            })
            .expect("card create should succeed");

        repo.connection()
            .execute("INSERT INTO tags(name) VALUES (?1)", ["p1"])
            .expect("tag insert should succeed");
        let tag_id: i64 = repo
            .connection()
            .query_row("SELECT id FROM tags WHERE name = ?1", ["p1"], |row| row.get(0))
            .expect("tag query should succeed");
        repo.connection()
            .execute(
                "INSERT INTO card_tags(card_id, tag_id) VALUES (?1, ?2)",
                rusqlite::params![original.id.to_string(), tag_id],
            )
            .expect("card_tags insert should succeed");

        let spawned = repo
            .complete_card(original.id, 0)
            .expect("completion should succeed")
            .expect("recurrence should spawn a card");

        let completed = repo
            .get_card(original.id)
            .expect("get should succeed")
            .expect("original should exist");
        assert_eq!(completed.column, Column::Done);
        assert!(completed.done_at.is_some());

        assert_eq!(spawned.title, original.title);
        assert_eq!(spawned.notes, original.notes);
        assert_eq!(spawned.recurrence, original.recurrence);
        assert_eq!(spawned.due_date, Some(today + Duration::days(1)));
        assert_eq!(spawned.column, Column::ThisWeek);

        let tag_count: i64 = repo
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM card_tags WHERE card_id = ?1",
                [spawned.id.to_string()],
                |row| row.get(0),
            )
            .expect("tag count should succeed");
        assert_eq!(tag_count, 1);
    }
}
