use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction};

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

const MIGRATION_001_CARDS: Migration = Migration {
    version: 1,
    name: "create_cards",
    sql: "
        CREATE TABLE cards (
            id         TEXT PRIMARY KEY NOT NULL,
            title      TEXT NOT NULL CHECK(length(title) > 0 AND length(title) <= 200),
            notes      TEXT,
            column     TEXT NOT NULL CHECK(column IN ('Backlog', 'ThisWeek', 'Today', 'Done')),
            position   INTEGER NOT NULL,
            due_date   TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            done_at    TEXT,
            archived   INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1)),
            blocked    INTEGER NOT NULL DEFAULT 0 CHECK(blocked IN (0, 1))
        );

        CREATE INDEX idx_cards_column_position ON cards(column, position);
        CREATE INDEX idx_cards_due_date ON cards(due_date) WHERE due_date IS NOT NULL;
        CREATE INDEX idx_cards_archived ON cards(archived);
        CREATE INDEX idx_cards_done_at ON cards(done_at) WHERE done_at IS NOT NULL;
    ",
};
const MIGRATION_002_TAGS: Migration = Migration {
    version: 2,
    name: "create_tags",
    sql: "
        CREATE TABLE tags (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE CHECK(length(trim(name)) > 0)
        );

        CREATE TABLE card_tags (
            card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY(card_id, tag_id)
        );

        CREATE INDEX idx_card_tags_card_id ON card_tags(card_id);
        CREATE INDEX idx_card_tags_tag_id ON card_tags(tag_id);
    ",
};
const MIGRATION_003_RECURRENCE_RULES: Migration = Migration {
    version: 3,
    name: "create_recurrence_rules",
    sql: "
        CREATE TABLE recurrence_rules (
            card_id      TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
            frequency    TEXT NOT NULL CHECK(frequency IN ('Daily', 'Weekly', 'Monthly')),
            interval     INTEGER NOT NULL CHECK(interval >= 1),
            days_of_week TEXT,
            day_of_month INTEGER CHECK(day_of_month BETWEEN 1 AND 31),
            CHECK (
                (frequency = 'Daily' AND days_of_week IS NULL AND day_of_month IS NULL) OR
                (frequency = 'Weekly' AND days_of_week IS NOT NULL AND day_of_month IS NULL) OR
                (frequency = 'Monthly' AND days_of_week IS NULL AND day_of_month IS NOT NULL)
            )
        );

        CREATE INDEX idx_recurrence_frequency ON recurrence_rules(frequency);
    ",
};
const MIGRATIONS: &[Migration] = &[
    MIGRATION_001_CARDS,
    MIGRATION_002_TAGS,
    MIGRATION_003_RECURRENCE_RULES,
];

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    ensure_schema_migrations_table(conn)?;

    for migration in MIGRATIONS {
        if is_applied(conn, migration.version)? {
            continue;
        }
        apply_migration(conn, migration)?;
    }

    Ok(())
}

fn ensure_schema_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )
    .context("failed to create schema_migrations table")?;
    Ok(())
}

fn is_applied(conn: &Connection, version: i64) -> Result<bool> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = ?1",
            [version],
            |row| row.get(0),
        )
        .optional()
        .context("failed checking migration status")?;
    Ok(found.is_some())
}

fn apply_migration(conn: &mut Connection, migration: &Migration) -> Result<()> {
    let tx = conn
        .transaction()
        .context("failed to begin migration transaction")?;
    tx.execute_batch(migration.sql)
        .with_context(|| format!("failed applying migration {}", migration.version))?;
    record_migration(&tx, migration)?;
    tx.commit().context("failed to commit migration transaction")?;
    Ok(())
}

fn record_migration(tx: &Transaction<'_>, migration: &Migration) -> Result<()> {
    tx.execute(
        "INSERT INTO schema_migrations(version, name, applied_at) VALUES (?1, ?2, ?3)",
        (migration.version, migration.name, Utc::now().to_rfc3339()),
    )
    .with_context(|| format!("failed recording migration {}", migration.version))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::run_migrations;

    #[test]
    fn creates_schema_migrations_table_and_is_idempotent() {
        let mut conn = Connection::open_in_memory().expect("in-memory db should open");

        run_migrations(&mut conn).expect("first run should succeed");
        run_migrations(&mut conn).expect("second run should succeed");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(count, 1);

        let cards_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='cards'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(cards_table_count, 1);

        let applied_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .expect("query should succeed");
        assert_eq!(applied_count, 3);

        let tags_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='tags'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(tags_table_count, 1);

        let card_tags_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='card_tags'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(card_tags_table_count, 1);

        let recurrence_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='recurrence_rules'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(recurrence_table_count, 1);
    }
}
