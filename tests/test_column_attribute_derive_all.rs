/// Test #[column("name")] attribute generates COLUMNS/COLUMNS_STR from derive_all
/// (i.e., from SqliteTemplate, PostgresTemplate, etc., NOT from standalone Columns derive)
/// This verifies the columns_derive() in mod.rs is called by derive_all().

use sqlx_template::SqliteTemplate;
use sqlx::FromRow;

// Test that SqliteTemplate generates COLUMNS and COLUMNS_STR automatically
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("events")]
pub struct Event {
    #[auto]
    pub id: i32,
    #[column("event_name")]
    pub name: String,
    #[column("event_type")]
    pub kind: String,
    pub priority: i32,
}

// Test without any #[column] - should still get COLUMNS from derive_all
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("logs")]
pub struct LogEntry {
    #[auto]
    pub id: i32,
    pub message: String,
    pub level: String,
}

// Test with all #[column] attributes
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("configs")]
pub struct Config {
    #[column("config_key")]
    pub key: String,
    #[column("config_value")]
    pub value: String,
    #[column("is_active")]
    pub active: bool,
}

#[test]
fn test_event_columns_from_sqlite_template() {
    assert_eq!(Event::COLUMNS, ["id", "event_name", "event_type", "priority"]);
    assert_eq!(Event::COLUMNS_STR, "id, event_name, event_type, priority");
}

#[test]
fn test_log_entry_columns_no_column_attr() {
    assert_eq!(LogEntry::COLUMNS, ["id", "message", "level"]);
    assert_eq!(LogEntry::COLUMNS_STR, "id, message, level");
}

#[test]
fn test_config_columns_all_column_attr() {
    assert_eq!(Config::COLUMNS, ["config_key", "config_value", "is_active"]);
    assert_eq!(Config::COLUMNS_STR, "config_key, config_value, is_active");
}

#[test]
fn test_columns_array_lengths_derive_all() {
    assert_eq!(Event::COLUMNS.len(), 4);
    assert_eq!(LogEntry::COLUMNS.len(), 3);
    assert_eq!(Config::COLUMNS.len(), 3);
}

#[test]
fn test_columns_str_consistency_derive_all() {
    for struct_columns in &[
        (&Event::COLUMNS[..], Event::COLUMNS_STR),
        (&LogEntry::COLUMNS[..], LogEntry::COLUMNS_STR),
        (&Config::COLUMNS[..], Config::COLUMNS_STR),
    ] {
        let joined = struct_columns.0.join(", ");
        assert_eq!(joined, struct_columns.1);
    }
}
