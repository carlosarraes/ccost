use rusqlite::Connection;
use anyhow::{Result, Context};

pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("../../migrations/001_initial.sql"),
    },
];

pub fn get_schema_version(connection: &Connection) -> Result<i32> {
    // Create schema_version table if it doesn't exist
    connection.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY)",
        [],
    ).context("Failed to create schema_version table")?;

    // Get current version, default to 0 if no rows
    let version = connection.query_row(
        "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
        [],
        |row| row.get::<_, i32>(0)
    ).unwrap_or(0);

    Ok(version)
}

pub fn apply_migrations(connection: &Connection) -> Result<()> {
    let current_version = get_schema_version(connection)?;
    
    for migration in MIGRATIONS {
        if migration.version > current_version {
            println!("Applying migration {}: {}", migration.version, migration.name);
            
            // Execute migration SQL
            connection.execute_batch(migration.sql)
                .with_context(|| format!("Failed to apply migration {}", migration.version))?;
            
            // Update schema version
            connection.execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
                [migration.version],
            ).with_context(|| format!("Failed to update schema version to {}", migration.version))?;
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_connection() -> Connection {
        Connection::open(":memory:").unwrap()
    }

    #[test]
    fn test_initial_schema_version() {
        let conn = setup_test_connection();
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 0);
    }

    #[test]
    fn test_migration_application() {
        let conn = setup_test_connection();
        
        // Apply migrations
        apply_migrations(&conn).unwrap();
        
        // Verify schema version is updated
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 1);
        
        // Verify tables exist
        let table_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('processed_messages', 'exchange_rates', 'model_pricing', 'usage_summary')",
            [],
            |row| row.get(0)
        ).unwrap();
        
        assert_eq!(table_count, 4);
    }

    #[test]
    fn test_idempotent_migrations() {
        let conn = setup_test_connection();
        
        // Apply migrations twice
        apply_migrations(&conn).unwrap();
        apply_migrations(&conn).unwrap();
        
        // Should still be at version 1
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 1);
        
        // Tables should still exist
        let table_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('processed_messages', 'exchange_rates', 'model_pricing', 'usage_summary')",
            [],
            |row| row.get(0)
        ).unwrap();
        
        assert_eq!(table_count, 4);
    }

    #[test]
    fn test_migration_order() {
        // Verify migrations are ordered by version
        for i in 1..MIGRATIONS.len() {
            assert!(MIGRATIONS[i].version > MIGRATIONS[i-1].version);
        }
    }
}