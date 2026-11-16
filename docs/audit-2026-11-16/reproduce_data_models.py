"""Offline SQLite models of audited control flow, NOT execution of Rust MessageStore.
Run: python docs/audit-2026-11-16/reproduce_data_models.py
Only in-memory DBs and synthetic data; no network/user data/source mutation.
"""
import json
import sqlite3

def deleted_snapshot_upsert():
    db = sqlite3.connect(":memory:")
    db.execute("CREATE TABLE messages(thread_id TEXT, msg_id TEXT, text TEXT, PRIMARY KEY(thread_id,msg_id))")
    db.execute("INSERT INTO messages VALUES('thread-1','message-1','synthetic')")
    # TUI DeleteDone removes visible row, then persist_snapshot loops remaining rows.
    visible = []
    for row in visible:
        db.execute("INSERT OR IGNORE INTO messages VALUES(?,?,?)", row)
    after = db.execute("SELECT msg_id FROM messages WHERE thread_id='thread-1'").fetchall()
    assert after == [("message-1",)]
    return {"name":"D1_upsert_snapshot_does_not_delete", "visible_after_delete":0, "rows_after_rehydrate":len(after), "reproduced":True, "kind":"SQLite control-flow model, not Rust execution"}

def ignored_alter_error():
    db = sqlite3.connect(":memory:")
    db.execute("CREATE TABLE threads(thread_id TEXT PRIMARY KEY)")
    db.execute("PRAGMA user_version=12")
    def authorize(action, arg1, arg2, database, trigger):
        return sqlite3.SQLITE_DENY if action == sqlite3.SQLITE_ALTER_TABLE else sqlite3.SQLITE_OK
    db.set_authorizer(authorize)
    # Equivalent to migration13 let _ = ALTER then let _ = PRAGMA.
    alter_failed = False
    try:
        db.execute("ALTER TABLE threads ADD COLUMN last_read_msg_id TEXT NOT NULL DEFAULT ''")
    except sqlite3.DatabaseError:
        alter_failed = True
    db.execute("PRAGMA user_version=13")
    version = db.execute("PRAGMA user_version").fetchone()[0]
    columns = [row[1] for row in db.execute("PRAGMA table_info(threads)")]
    assert alter_failed and version == 13 and "last_read_msg_id" not in columns
    return {"name":"D5_ignored_alter_can_advance_schema", "alter_failed":alter_failed, "version":version, "missing_column":True, "reproduced":True, "kind":"fault-injected SQLite model, not Rust execution"}

print(json.dumps([deleted_snapshot_upsert(), ignored_alter_error()],indent=2))
