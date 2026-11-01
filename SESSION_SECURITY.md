# Session Security Notice

The default session is now `session.dat` under the per-user data directory. On Windows it is protected with current-user DPAPI. On Linux/macOS it uses a private directory and a file created with mode 0600. SQLite history remains unencrypted at the application layer.

Never commit or share session files, history databases, media caches or diagnostic logs. DPAPI does not protect against malicious processes running as the same Windows user. Disabling remember-login removes the saved session and prevents subsequent authentication/refresh paths from persisting it.

Local logout removes the saved session, invalidates in-process session snapshots and separates the next account from the previous account's history. It does not revoke the server-side session. If credentials were exposed, revoke the affected session from the official Zalo mobile application's account security settings.

Legacy session.json and unscoped history are not imported automatically. Keep them private, reauthenticate, and verify account ownership before any manual migration.
