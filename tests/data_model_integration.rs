use std::path::PathBuf;

use chrono::{Duration, Utc};
use kb::domain::Column;
use kb::repo::{NewCard, SqliteRepository};
use kb::storage::{open_connection, run_migrations};

fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!("kb-it-{}.db", uuid::Uuid::new_v4()))
}

#[test]
fn migrations_are_idempotent() {
    let path = temp_db_path();
    let mut conn = open_connection(&path).expect("db should open");

    run_migrations(&mut conn).expect("first migration run should succeed");
    run_migrations(&mut conn).expect("second migration run should succeed");

    let migration_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("count query should succeed");
    assert_eq!(migration_count, 2);

    drop(conn);
    std::fs::remove_file(path).expect("temp db should be removable");
}

#[test]
fn completion_moves_card_to_done_and_preserves_tags() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db should open");
    let mut repo = SqliteRepository::new(conn).expect("repo should initialize");

    let today = Utc::now().date_naive();
    let original = repo
        .create_card(NewCard {
            title: "Daily planning".to_string(),
            column: Column::Today,
            position: 0,
            due_date: Some(today),
        })
        .expect("card create should succeed");

    repo.set_tags(original.id, vec!["routine".to_string(), "p1".to_string()])
        .expect("set_tags should succeed");

    repo.complete_card(original.id, 0)
        .expect("complete_card should succeed");

    let completed = repo
        .get_card(original.id)
        .expect("get card should succeed")
        .expect("card should still exist");
    assert_eq!(completed.column, Column::Done);
    assert!(completed.done_at.is_some());
    assert_eq!(
        completed.tags,
        vec!["p1".to_string(), "routine".to_string()]
    );
}

#[test]
fn archive_threshold_and_archive_all_work_together() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db should open");
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

    let old_done_at = (Utc::now() - Duration::days(8)).to_rfc3339();
    let fresh_done_at = Utc::now().to_rfc3339();
    repo.connection()
        .execute(
            "UPDATE cards SET done_at = ?1 WHERE id = ?2",
            rusqlite::params![old_done_at, old_done.id.to_string()],
        )
        .expect("update old done_at should succeed");
    repo.connection()
        .execute(
            "UPDATE cards SET done_at = ?1 WHERE id = ?2",
            rusqlite::params![fresh_done_at, fresh_done.id.to_string()],
        )
        .expect("update fresh done_at should succeed");

    let archived_by_age = repo
        .archive_done_older_than(7)
        .expect("archive_done_older_than should succeed");
    assert_eq!(archived_by_age, 1);

    let archived_all = repo
        .archive_all_done()
        .expect("archive_all_done should succeed");
    assert_eq!(archived_all, 1);

    let visible_done = repo
        .list_cards_in_column(Column::Done)
        .expect("list done should succeed");
    assert!(visible_done.is_empty());
}
