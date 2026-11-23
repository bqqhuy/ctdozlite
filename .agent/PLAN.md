# .agent/PLAN.md - Round 4 Audit Remediation Plan

Tài liệu chi tiết xem tại: `docs/research/2026-11-23-audit-round3-remediation-plan.md`

## 1. Mục tiêu Vòng 4
Khắc phục 3 phát hiện còn lại từ Codex Audit Vòng 4:
1. Chống phát lặp auto-sync trong khi chờ ACK (`src/gui.rs:7534`, lifecycle Pending -> AwaitingAcceptance -> Accepted/Rejected, correlate intent_id).
2. Tách nguồn kích hoạt (SyncOrigin: Setting, AutoSync, Banner) khỏi loại sync (SyncMode: Full, Recent, Rescan) và áp đúng gate `auto_sync.enable` (`src/server_config.rs:236`).
3. Nối pha optional preload thực tế (`src/gui.rs:4117`, `src/gui/worker.rs:1860`, tách tín hiệu `StartupPrerequisitesSettled` và `StartupPreloadSettled`).

## 2. Contracts & Invariants (Ma trận bảo toàn)
- Baseline `f33f43a`, toàn bộ sửa đổi Vòng 1, 2, 3 được bảo toàn nguyên vẹn.
- Không commit, không push, không checkout/reset.
- Không có I/O hoặc crypto trên UI thread.
- Ký signkey, payload password login (encrypted có salt, unencrypted không salt).
- Session runtime isolation cho error 19000.
- Operation fencing: ID=0 không đè operation đang chạy.
- Desktop backup WAL narrow permit & fail-fast concurrency test.
