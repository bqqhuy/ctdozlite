# .agent/TASKS.md - Trạng thái Task Vòng 4

- [x] Task 0: Trình ADVISOR phê duyệt kế hoạch Vòng 4 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 1: Tách SyncOrigin khỏi SyncMode, parse `auto_sync.enable` và áp đúng gate (`src/sync_coordinator.rs`, `src/server_config.rs`)
- [x] Task 2: Vòng đời auto-sync intent & chống phát lặp khi chờ ACK (`src/gui.rs:7534`, `src/gui/worker.rs`)
- [x] Task 3: Tách tín hiệu `StartupPrerequisitesSettled` và `StartupPreloadSettled` trong worker pipeline thực (`src/gui/worker.rs:1860`, `src/gui.rs:4117`)
- [x] Task 4: Review chéo độc lập Reviewer A & Reviewer B (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 5: Kiểm tra toàn bộ test suite, clippy, prepush và hoàn thiện dossier (HOÀN THÀNH TOÀN BỘ)
