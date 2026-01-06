//! SQLite storage, migrations, and connection management.

mod db;
mod migrations;

pub use db::{
    default_db_path, ensure_db_parent_exists, open_connection, open_default_connection,
};
pub use migrations::{run_migrations, Migration};
