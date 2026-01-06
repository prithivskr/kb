//! SQLite storage, migrations, and connection management.

mod db;

pub use db::{
    default_db_path, ensure_db_parent_exists, open_connection, open_default_connection,
};
