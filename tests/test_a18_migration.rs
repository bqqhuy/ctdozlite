//! Integration test for A18 migration: verify schema v12 fixes FK constraint on promotion.

use anyhow::Result;
use rusqlite::{params, Connection};

#[test]
fn test_a18_migration_fixes_fk_constraint() -> Result<()> {
    let temp = std::env::temp_dir().join(format!("a18_migration_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&temp);

    let conn = Connection::open(&temp)?;
    conn.execute_batch("PRAGMA foreign_keys=ON")?;

    // Create schema v10 WITHOUT ON UPDATE CASCADE (the broken state)
    conn.execute_batch(
        "CREATE TABLE messages(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            text TEXT,
            ts INTEGER,
            is_outgoing INTEGER,
            sender_id TEXT,
            sender_name TEXT,
            msg_type TEXT,
            raw_json TEXT,
            PRIMARY KEY(thread_id, msg_id)
        );
        CREATE TABLE message_expiry(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL CHECK(expires_at>0),
            PRIMARY KEY(thread_id, msg_id),
            FOREIGN KEY(thread_id, msg_id) REFERENCES messages(thread_id, msg_id) ON DELETE CASCADE
        );
        CREATE TABLE restored_assets(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            quality TEXT NOT NULL,
            name TEXT NOT NULL,
            PRIMARY KEY(thread_id, msg_id, quality),
            FOREIGN KEY(thread_id, msg_id) REFERENCES messages(thread_id, msg_id) ON DELETE CASCADE
        );
        PRAGMA user_version=10;",
    )?;

    // Insert test data
    conn.execute(
        "INSERT INTO messages(thread_id, msg_id, text, ts, is_outgoing, sender_id, sender_name, msg_type)
         VALUES(?1, ?2, ?3, ?4, 0, 'sender1', 'Sender', 'text')",
        params!["thread1", "local_abc", "test message", 1700000000000i64],
    )?;
    conn.execute(
        "INSERT INTO message_expiry VALUES(?1, ?2, ?3)",
        params!["thread1", "local_abc", 1700010000000i64],
    )?;
    conn.execute(
        "INSERT INTO restored_assets VALUES(?1, ?2, ?3, ?4)",
        params!["thread1", "local_abc", "thumb", "asset_123.jpg"],
    )?;

    // Verify promotion fails with v10 schema
    let result = conn.execute(
        "UPDATE messages SET msg_id=?1 WHERE thread_id=?2 AND msg_id=?3",
        params!["server_456", "thread1", "local_abc"],
    );
    assert!(result.is_err(), "v10 schema should fail FK constraint on promotion");

    // Run migration v12 — it preserves all data
    zalo_rust::message_lifecycle::migrate_v12(&conn)?;

    // Verify schema version updated
    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(version, 12);

    // Verify data was preserved
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM message_expiry WHERE thread_id=?1 AND msg_id=?2",
        params!["thread1", "local_abc"],
        |row| row.get(0),
    )?;
    assert_eq!(count, 1, "migration should preserve expiry data");

    // Promotion should now succeed with v12 schema
    conn.execute(
        "UPDATE messages SET msg_id=?1 WHERE thread_id=?2 AND msg_id=?3",
        params!["server_456", "thread1", "local_abc"],
    )?;

    // Verify cascade worked
    let expiry_id: String = conn.query_row(
        "SELECT msg_id FROM message_expiry WHERE thread_id=?1",
        params!["thread1"],
        |row| row.get(0),
    )?;
    assert_eq!(expiry_id, "server_456");

    let asset_id: String = conn.query_row(
        "SELECT msg_id FROM restored_assets WHERE thread_id=?1",
        params!["thread1"],
        |row| row.get(0),
    )?;
    assert_eq!(asset_id, "server_456");

    let _ = std::fs::remove_file(&temp);
    Ok(())
}
