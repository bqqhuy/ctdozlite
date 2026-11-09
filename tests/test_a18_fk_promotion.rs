//! Regression test for audit A18: FK constraint failure on message ID promotion
//!
//! Reproduces the scenario where UPDATE messages.msg_id (local → server) fails
//! because message_expiry and restored_assets have FK constraints but no
//! ON UPDATE CASCADE.

use rusqlite::{params, Connection, Result};

#[test]
fn test_a18_fk_promotion_with_expiry() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON")?;

    // Schema v10 WITHOUT ON UPDATE CASCADE (the broken state)
    conn.execute_batch(
        "CREATE TABLE messages(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            PRIMARY KEY(thread_id, msg_id)
        );
        CREATE TABLE message_expiry(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            PRIMARY KEY(thread_id, msg_id),
            FOREIGN KEY(thread_id, msg_id) REFERENCES messages(thread_id, msg_id) ON DELETE CASCADE
        );",
    )?;

    // Insert pending message with expiry
    conn.execute(
        "INSERT INTO messages VALUES(?1, ?2)",
        params!["thread1", "local_123"],
    )?;
    conn.execute(
        "INSERT INTO message_expiry VALUES(?1, ?2, ?3)",
        params!["thread1", "local_123", 1700000000000i64],
    )?;

    // Simulate server echo: promote local → server ID
    // This SHOULD fail with FK constraint without ON UPDATE CASCADE
    let result = conn.execute(
        "UPDATE messages SET msg_id=?1 WHERE thread_id=?2 AND msg_id=?3",
        params!["server_456", "thread1", "local_123"],
    );

    // Expected: FK constraint failure
    assert!(
        result.is_err(),
        "UPDATE should fail without ON UPDATE CASCADE"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("FOREIGN KEY constraint failed"),
        "Expected FK constraint error, got: {err_msg}"
    );

    Ok(())
}

#[test]
fn test_a18_fk_promotion_with_restored_assets() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON")?;

    conn.execute_batch(
        "CREATE TABLE messages(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            PRIMARY KEY(thread_id, msg_id)
        );
        CREATE TABLE restored_assets(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            quality TEXT NOT NULL,
            name TEXT NOT NULL,
            PRIMARY KEY(thread_id, msg_id, quality),
            FOREIGN KEY(thread_id, msg_id) REFERENCES messages(thread_id, msg_id) ON DELETE CASCADE
        );",
    )?;

    conn.execute(
        "INSERT INTO messages VALUES(?1, ?2)",
        params!["thread1", "local_456"],
    )?;
    conn.execute(
        "INSERT INTO restored_assets VALUES(?1, ?2, ?3, ?4)",
        params!["thread1", "local_456", "thumb", "asset_789.jpg"],
    )?;

    // Promote ID → should fail
    let result = conn.execute(
        "UPDATE messages SET msg_id=?1 WHERE thread_id=?2 AND msg_id=?3",
        params!["server_789", "thread1", "local_456"],
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("FOREIGN KEY constraint failed"));

    Ok(())
}

#[test]
fn test_a18_fixed_with_cascade() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON")?;

    // Fixed schema WITH ON UPDATE CASCADE
    conn.execute_batch(
        "CREATE TABLE messages(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            PRIMARY KEY(thread_id, msg_id)
        );
        CREATE TABLE message_expiry(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            PRIMARY KEY(thread_id, msg_id),
            FOREIGN KEY(thread_id, msg_id) REFERENCES messages(thread_id, msg_id)
                ON DELETE CASCADE
                ON UPDATE CASCADE
        );
        CREATE TABLE restored_assets(
            thread_id TEXT NOT NULL,
            msg_id TEXT NOT NULL,
            quality TEXT NOT NULL,
            name TEXT NOT NULL,
            PRIMARY KEY(thread_id, msg_id, quality),
            FOREIGN KEY(thread_id, msg_id) REFERENCES messages(thread_id, msg_id)
                ON DELETE CASCADE
                ON UPDATE CASCADE
        );",
    )?;

    // Insert both expiry and asset
    conn.execute(
        "INSERT INTO messages VALUES(?1, ?2)",
        params!["thread1", "local_999"],
    )?;
    conn.execute(
        "INSERT INTO message_expiry VALUES(?1, ?2, ?3)",
        params!["thread1", "local_999", 1700000000000i64],
    )?;
    conn.execute(
        "INSERT INTO restored_assets VALUES(?1, ?2, ?3, ?4)",
        params!["thread1", "local_999", "orig", "file.mp4"],
    )?;

    // Promote ID → should succeed and cascade
    conn.execute(
        "UPDATE messages SET msg_id=?1 WHERE thread_id=?2 AND msg_id=?3",
        params!["server_final", "thread1", "local_999"],
    )?;

    // Verify cascade worked
    let expiry_id: String = conn.query_row(
        "SELECT msg_id FROM message_expiry WHERE thread_id=?1",
        params!["thread1"],
        |row| row.get(0),
    )?;
    assert_eq!(expiry_id, "server_final");

    let asset_id: String = conn.query_row(
        "SELECT msg_id FROM restored_assets WHERE thread_id=?1",
        params!["thread1"],
        |row| row.get(0),
    )?;
    assert_eq!(asset_id, "server_final");

    Ok(())
}
