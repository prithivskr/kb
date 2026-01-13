use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::domain::{Card, CardId, Column, validate_card_title};
use crate::storage::run_migrations;

const TODAY_WIP_LIMIT: i64 = 4;

#[derive(Debug, Clone)]
pub struct NewCard {
    pub title: String,
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
        let mut card = Card::new(
            CardId::new(),
            input.title,
            input.column,
            input.position,
            now,
        )?;
        card.due_date = input.due_date;

        if card.column == Column::Today {
            ensure_today_has_capacity_conn(&self.conn)?;
        }

        self.conn.execute(
            "INSERT INTO cards(
                id, title, column, position, due_date, created_at, updated_at, done_at, archived, blocked
            ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                card.id.to_string(),
                card.title,
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

        Ok(card)
    }

    pub fn insert_card_at(&mut self, input: NewCard) -> Result<Card> {
        let now = Utc::now();
        let mut card = Card::new(
            CardId::new(),
            input.title,
            input.column,
            input.position,
            now,
        )?;
        card.due_date = input.due_date;

        let tx = self
            .conn
            .transaction()
            .context("failed to begin insert-at transaction")?;

        if card.column == Column::Today {
            ensure_today_has_capacity_tx(&tx)?;
        }

        tx.execute(
            "UPDATE cards
             SET position = position + 1
             WHERE column = ?1 AND archived = 0 AND position >= ?2",
            params![card.column, card.position],
        )
        .context("failed shifting column positions for insert-at")?;

        tx.execute(
            "INSERT INTO cards(
                id, title, column, position, due_date, created_at, updated_at, done_at, archived, blocked
            ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                card.id.to_string(),
                card.title,
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
        .context("failed inserting card in insert-at")?;

        tx.commit()
            .context("failed to commit insert-at transaction")?;
        Ok(card)
    }

    pub fn get_card(&self, id: CardId) -> Result<Option<Card>> {
        let card = self
            .conn
            .query_row(
                "SELECT
                    id, title, column, position, due_date,
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
                    id, title, column, position, due_date,
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

    pub fn list_archived_cards(&self) -> Result<Vec<Card>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT
                    id, title, column, position, due_date,
                    created_at, updated_at, done_at, archived, blocked
                 FROM cards
                 WHERE archived = 1
                 ORDER BY COALESCE(done_at, updated_at) DESC, created_at DESC",
            )
            .context("failed to prepare list_archived_cards statement")?;

        let iter = stmt
            .query_map([], row_to_card)
            .context("failed listing archived cards")?;
        let cards: rusqlite::Result<Vec<Card>> = iter.collect();
        cards
            .context("failed parsing archived cards from query")?
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

    pub fn move_card(
        &mut self,
        id: CardId,
        target_column: Column,
        target_position: i64,
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("failed to begin move transaction")?;
        let (source_column, source_position) = fetch_card_location(&tx, id)?
            .with_context(|| format!("card not found for move: {id}"))?;
        let now = Utc::now().to_rfc3339();

        if source_column == target_column {
            reorder_within_column(
                &tx,
                id,
                source_column,
                source_position,
                target_position,
                &now,
            )?;
        } else {
            if target_column == Column::Today {
                ensure_today_has_capacity_tx(&tx)?;
            }

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

    pub fn delete_card(&mut self, id: CardId) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("failed to begin delete transaction")?;
        let (column, position) = fetch_card_location(&tx, id)?
            .with_context(|| format!("card not found for delete: {id}"))?;

        tx.execute("DELETE FROM cards WHERE id = ?1", [id.to_string()])
            .context("failed deleting card")?;

        tx.execute(
            "UPDATE cards
             SET position = position - 1
             WHERE column = ?1 AND archived = 0 AND position > ?2",
            params![column, position],
        )
        .context("failed compacting column after delete")?;

        tx.commit().context("failed to commit delete transaction")?;
        Ok(())
    }

    pub fn complete_card(&mut self, id: CardId, done_position: i64) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("failed to begin completion transaction")?;
        let (source_column, source_position) = fetch_card_location(&tx, id)?
            .with_context(|| format!("card not found for completion: {id}"))?;

        let now = Utc::now().to_rfc3339();
        if source_column == Column::Done {
            reorder_within_column(&tx, id, source_column, source_position, done_position, &now)?;
            tx.execute(
                "UPDATE cards SET done_at = COALESCE(done_at, ?1), updated_at = ?1 WHERE id = ?2",
                params![now, id.to_string()],
            )
            .context("failed updating done timestamp for already-done card")?;
        } else {
            tx.execute(
                "UPDATE cards
                 SET position = position - 1
                 WHERE column = ?1 AND archived = 0 AND position > ?2",
                params![source_column, source_position],
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
                params![Column::Done, done_position, now, now, id.to_string()],
            )
            .context("failed moving completed card to done")?;
        }

        tx.commit()
            .context("failed to commit completion transaction")?;
        Ok(())
    }

    pub fn set_tags(&mut self, id: CardId, tags: Vec<String>) -> Result<()> {
        let normalized_tags = normalize_tags(tags);
        let tx = self
            .conn
            .transaction()
            .context("failed to begin set_tags transaction")?;

        let exists = fetch_card_location(&tx, id)?.is_some();
        if !exists {
            anyhow::bail!("card not found: {id}");
        }

        tx.execute("DELETE FROM card_tags WHERE card_id = ?1", [id.to_string()])
            .context("failed to clear existing card tags")?;

        for tag_name in &normalized_tags {
            tx.execute(
                "INSERT INTO tags(name) VALUES(?1) ON CONFLICT(name) DO NOTHING",
                [tag_name],
            )
            .context("failed inserting tag")?;

            let tag_id: i64 = tx
                .query_row("SELECT id FROM tags WHERE name = ?1", [tag_name], |row| {
                    row.get(0)
                })
                .context("failed loading tag id")?;

            tx.execute(
                "INSERT INTO card_tags(card_id, tag_id) VALUES(?1, ?2)",
                params![id.to_string(), tag_id],
            )
            .context("failed linking tag to card")?;
        }

        tx.execute(
            "UPDATE cards SET updated_at = ?1 WHERE id = ?2",
            [Utc::now().to_rfc3339(), id.to_string()],
        )
        .context("failed to update card timestamp after setting tags")?;

        tx.commit()
            .context("failed to commit set_tags transaction")?;
        Ok(())
    }

    pub fn list_tags_in_use(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT t.name
                 FROM tags t
                 JOIN card_tags ct ON ct.tag_id = t.id
                 JOIN cards c ON c.id = ct.card_id
                 WHERE c.archived = 0
                 ORDER BY t.name ASC",
            )
            .context("failed to prepare list_tags_in_use statement")?;

        let iter = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .context("failed querying tags in use")?;
        let tags: rusqlite::Result<Vec<String>> = iter.collect();
        Ok(tags.context("failed reading tags in use")?)
    }

    pub fn archive_all_done(&mut self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let updated = self
            .conn
            .execute(
                "UPDATE cards
                 SET archived = 1, updated_at = ?1
                 WHERE column = 'Done' AND archived = 0",
                [now],
            )
            .context("failed archiving done cards")?;
        Ok(updated)
    }

    pub fn archive_done_older_than(&mut self, days: i64) -> Result<usize> {
        if days < 0 {
            anyhow::bail!("archive days must be >= 0");
        }

        let now = Utc::now();
        let cutoff = now - Duration::days(days);
        let updated = self
            .conn
            .execute(
                "UPDATE cards
                 SET archived = 1, updated_at = ?1
                 WHERE column = 'Done'
                   AND archived = 0
                   AND done_at IS NOT NULL
                   AND done_at < ?2",
                params![now.to_rfc3339(), cutoff.to_rfc3339()],
            )
            .context("failed auto-archiving old done cards")?;
        Ok(updated)
    }

    fn hydrate_card(&self, mut card: Card) -> Result<Card> {
        card.tags = fetch_tags_for_card_conn(&self.conn, card.id)?;
        Ok(card)
    }
}

fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
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
        NaiveDate::parse_from_str(&value, "%Y-%m-%d")
            .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
    })
    .transpose()
}

fn row_to_card(row: &Row<'_>) -> rusqlite::Result<Card> {
    let id = parse_card_id(row.get::<_, String>(0)?)?;
    let title = row.get(1)?;
    let column = row.get(2)?;
    let position = row.get(3)?;
    let due_date = parse_optional_date(row.get(4)?)?;
    let created_at = parse_datetime(row.get(5)?)?;
    let updated_at = parse_datetime(row.get(6)?)?;
    let done_at = parse_optional_datetime(row.get(7)?)?;
    let archived = int_to_bool(row.get(8)?);
    let blocked = int_to_bool(row.get(9)?);

    Ok(Card {
        id,
        title,
        column,
        position,
        tags: Vec::new(),
        due_date,
        created_at,
        updated_at,
        done_at,
        archived,
        blocked,
    })
}

fn ensure_today_has_capacity_conn(conn: &Connection) -> Result<()> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE column = ?1 AND archived = 0",
            [Column::Today],
            |row| row.get(0),
        )
        .context("failed counting today cards")?;
    if count >= TODAY_WIP_LIMIT {
        anyhow::bail!("today column is full ({TODAY_WIP_LIMIT} tasks max)");
    }
    Ok(())
}

fn ensure_today_has_capacity_tx(tx: &Transaction<'_>) -> Result<()> {
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE column = ?1 AND archived = 0",
            [Column::Today],
            |row| row.get(0),
        )
        .context("failed counting today cards")?;
    if count >= TODAY_WIP_LIMIT {
        anyhow::bail!("today column is full ({TODAY_WIP_LIMIT} tasks max)");
    }
    Ok(())
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

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value = trimmed.to_owned();
        if !output.contains(&value) {
            output.push(value);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, Utc};
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
                column: Column::Backlog,
                position: 0,
                due_date: Some(NaiveDate::from_ymd_opt(2026, 3, 9).expect("valid date")),
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Card B".to_string(),
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
                column: Column::Backlog,
                position: 0,
                due_date: None,
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Second".to_string(),
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

    #[test]
    fn completing_card_moves_it_to_done() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let card = repo
            .create_card(NewCard {
                title: "Daily review".to_string(),
                column: Column::Today,
                position: 0,
                due_date: Some(Utc::now().date_naive()),
            })
            .expect("card create should succeed");

        repo.set_tags(card.id, vec!["p1".to_string()])
            .expect("set_tags should succeed");
        repo.complete_card(card.id, 0)
            .expect("completion should succeed");

        let completed = repo
            .get_card(card.id)
            .expect("get should succeed")
            .expect("card should exist");
        assert_eq!(completed.column, Column::Done);
        assert!(completed.done_at.is_some());
        assert_eq!(completed.tags, vec!["p1".to_string()]);
    }

    #[test]
    fn tag_management_lists_tags_in_use() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let first = repo
            .create_card(NewCard {
                title: "Card one".to_string(),
                column: Column::Backlog,
                position: 0,
                due_date: None,
            })
            .expect("card create should succeed");
        let second = repo
            .create_card(NewCard {
                title: "Card two".to_string(),
                column: Column::Backlog,
                position: 1,
                due_date: None,
            })
            .expect("card create should succeed");

        repo.set_tags(
            first.id,
            vec!["p1".to_string(), "backend".to_string(), "p1".to_string()],
        )
        .expect("set_tags should succeed");
        repo.set_tags(second.id, vec!["p1".to_string()])
            .expect("set_tags should succeed");

        let tags = repo.list_tags_in_use().expect("list tags should succeed");
        assert_eq!(tags, vec!["backend".to_string(), "p1".to_string()]);
    }

    #[test]
    fn archive_operations_mark_done_cards_as_archived() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let old_done = repo
            .create_card(NewCard {
                title: "Old done".to_string(),
                column: Column::Done,
                position: 0,
                due_date: None,
            })
            .expect("card create should succeed");
        let fresh_done = repo
            .create_card(NewCard {
                title: "Fresh done".to_string(),
                column: Column::Done,
                position: 1,
                due_date: None,
            })
            .expect("card create should succeed");

        let ten_days_ago = (Utc::now() - Duration::days(10)).to_rfc3339();
        let now = Utc::now().to_rfc3339();
        repo.connection()
            .execute(
                "UPDATE cards SET done_at = ?1 WHERE id = ?2",
                rusqlite::params![ten_days_ago, old_done.id.to_string()],
            )
            .expect("old done timestamp update should succeed");
        repo.connection()
            .execute(
                "UPDATE cards SET done_at = ?1 WHERE id = ?2",
                rusqlite::params![now, fresh_done.id.to_string()],
            )
            .expect("fresh done timestamp update should succeed");

        let auto_archived = repo
            .archive_done_older_than(7)
            .expect("auto-archive should succeed");
        assert_eq!(auto_archived, 1);

        let archived_old: i64 = repo
            .connection()
            .query_row(
                "SELECT archived FROM cards WHERE id = ?1",
                [old_done.id.to_string()],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(archived_old, 1);

        let archived_rest = repo.archive_all_done().expect("archive-all should succeed");
        assert_eq!(archived_rest, 1);
    }

    #[test]
    fn list_archived_cards_returns_only_archived_in_descending_recency() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        let active = repo
            .create_card(NewCard {
                title: "Active".to_string(),
                column: Column::Backlog,
                position: 0,
                due_date: None,
            })
            .expect("card create should succeed");
        let older = repo
            .create_card(NewCard {
                title: "Older archived".to_string(),
                column: Column::Done,
                position: 0,
                due_date: None,
            })
            .expect("card create should succeed");
        let newer = repo
            .create_card(NewCard {
                title: "Newer archived".to_string(),
                column: Column::Done,
                position: 1,
                due_date: None,
            })
            .expect("card create should succeed");

        repo.connection()
            .execute(
                "UPDATE cards SET archived = 1, done_at = ?1 WHERE id = ?2",
                rusqlite::params![
                    (Utc::now() - Duration::days(5)).to_rfc3339(),
                    older.id.to_string()
                ],
            )
            .expect("older archive update should succeed");
        repo.connection()
            .execute(
                "UPDATE cards SET archived = 1, done_at = ?1 WHERE id = ?2",
                rusqlite::params![
                    (Utc::now() - Duration::days(1)).to_rfc3339(),
                    newer.id.to_string()
                ],
            )
            .expect("newer archive update should succeed");

        let archived = repo
            .list_archived_cards()
            .expect("list archived cards should succeed");
        assert_eq!(archived.len(), 2);
        assert_eq!(archived[0].id, newer.id);
        assert_eq!(archived[1].id, older.id);
        assert!(archived.iter().all(|card| card.archived));
        assert!(!archived.iter().any(|card| card.id == active.id));
    }

    #[test]
    fn today_column_hard_limit_blocks_create_insert_and_move() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

        for index in 0..4 {
            repo.create_card(NewCard {
                title: format!("Today {index}"),
                column: Column::Today,
                position: index,
                due_date: None,
            })
            .expect("seed today cards should succeed");
        }

        let create_error = repo
            .create_card(NewCard {
                title: "Overflow create".to_string(),
                column: Column::Today,
                position: 4,
                due_date: None,
            })
            .expect_err("create beyond limit should fail");
        assert!(create_error.to_string().contains("today column is full"));

        let insert_error = repo
            .insert_card_at(NewCard {
                title: "Overflow insert".to_string(),
                column: Column::Today,
                position: 0,
                due_date: None,
            })
            .expect_err("insert beyond limit should fail");
        assert!(insert_error.to_string().contains("today column is full"));

        let backlog = repo
            .create_card(NewCard {
                title: "Backlog card".to_string(),
                column: Column::Backlog,
                position: 0,
                due_date: None,
            })
            .expect("backlog create should succeed");
        let move_error = repo
            .move_card(backlog.id, Column::Today, 0)
            .expect_err("move into full today should fail");
        assert!(move_error.to_string().contains("today column is full"));

        let today_cards = repo
            .list_cards_in_column(Column::Today)
            .expect("today list should succeed");
        assert_eq!(today_cards.len(), 4);
        let backlog_cards = repo
            .list_cards_in_column(Column::Backlog)
            .expect("backlog list should succeed");
        assert_eq!(backlog_cards.len(), 1);
        assert_eq!(backlog_cards[0].id, backlog.id);
    }
}
