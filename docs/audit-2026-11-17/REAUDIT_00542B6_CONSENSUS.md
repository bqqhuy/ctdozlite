# Đồng thuận tái kiểm toán `00542b6`

Ngày 17/11/2026. Snapshot: `main` @ `00542b68272496087f505443b4c4804bc93a2b38`
(= `origin/main`). Phạm vi diff: `f2b5e97..00542b6` (`50c1a4b`, `00542b6`).
Nguồn: [REAUDIT_00542B6_PRO_A.md](REAUDIT_00542B6_PRO_A.md),
[REAUDIT_00542B6_PRO_B.md](REAUDIT_00542B6_PRO_B.md). Agent chính (Claude Code,
tiếp quản lượt Codex dang dở) đã tự đọc lại source cho mọi finding P1/P2. Lượt
gate local tại snapshot này không hợp lệ (xem mục Gate).

## Kết luận

**CHANGES REQUIRED.** Pro A và Pro B đồng thuận độc lập. N01–N05 đều sửa được
root cause chính nhưng chưa đóng toàn bộ. Có một regression P1 phá hủy dữ liệu
gốc của người dùng. Không release từ `00542b6` cho đến khi P1 được sửa.

## Finding hợp nhất

| ID | Mức | Pro A | Pro B | Root xác minh source | Tóm tắt |
|---|---|---|---|---|---|
| R01 | **P1** | A01 | B3 | Có — `src/outgoing.rs:339–346` | Cleanup sau gửi xóa tệp nếu basename bắt đầu `clipboard_` và parent kết thúc `temp`. Không kiểm root app/owner/provenance. Tệp người dùng chọn qua picker/drop (cả HD qua `hd_source.path`) có thể bị xóa. |
| R02 | P2 | A02 | B1 | Có — `src/tui.rs:1286` `spawn_poll` không kiểm `logout_inflight`; gate event chỉ kiểm `RuntimeStamp` | Event poll/refresh/hydrate đến muộn có thể đưa hội thoại trở lại sau logout TUI; `LogoutDone(Ok)` không xóa lại. |
| R03 | P2 | A03 | B4 | Có — `src/gui/workspace.rs:5917–5918` | `is_thread_e2ee` (mutex global + SQLite) chạy trong render mỗi frame khi peer không có trong `trial_peers`. Vi phạm INV-3; lỗi DB bị đổi thành `false`. |
| R04 | P2 | B2 (A đồng ý) | B2 | Có — `src/gui.rs:4319–4323`, `src/gui/worker.rs:663–674`, `src/gui/messages.rs:49–50` | `StickerImageReady` muộn ghi đè texture atlas nhưng giữ `frames/delays` trong `sticker_sprites` → UV cắt sai. Preloader phát atlas legacy `{key}.png` như ảnh tĩnh; khi tắt animation không có đường migrate. |
| R05 | P2 | A04 | (dẫn A) | Theo báo cáo A, đã đối chiếu `src/gui/worker.rs:2139–2147` | Clipboard temp không có RAII/lease: hủy task hoặc drop event channel để lại PNG; sweep chỉ chạy một lần lúc khởi động. |
| R06 | P2 | A05 | B6 | Có — `tests/audit_regressions.rs:770–802`, `src/profiles.rs:167–173` | Test logout gọi `perform_account_logout` → `storage::remove_session()` global mà không tự cô lập data root. Bare `cargo test` không có `CTDOZ_DATA_DIR` có thể xóa `session.dat` thật và ghi registry thật. |
| R07 | P2 | N05 | B5 | Có — `src/history_sync.rs:233,284,354,377` | `FilteredMemo::load/save` (đọc file, `atomic_write`+fsync) vẫn đồng bộ trên async executor. |
| R08 | P3 | N05 | (dẫn A) | Theo báo cáo A, `src/gui/worker.rs:2564–2586` | Hai lần lưu QR session detached không có thứ tự; lỗi lưu chỉ log, không `StatusUpdate`. Chưa tái hiện race. |

## Trạng thái N01–N05

| Mục | Trạng thái | Còn lại |
|---|---|---|
| N01 logout TUI | Partial | R02 |
| N02 sticker cache | Partial | R04 |
| N03 temp lifecycle | Partial, **regression P1** | R01, R05 |
| N04 E2EE reset | Partial | R03; thông báo xác nhận nên nói rõ mất ratchet; thiếu test failure/restart |
| N05 blocking offload | Partial | R07, R08 |

## Thứ tự sửa đề xuất

1. **R01 (P1)** — mang provenance "app-owned temp" trong payload (ví dụ cờ/lease
   đặt tại nơi tạo clipboard PNG), chỉ xóa khi path canonical nằm trong
   `storage::account_temp_dir(owner)`. Không suy quyền xóa từ tên tệp. Test: tệp
   picker ngoài root có tên `clipboard_*.png` trong thư mục `temp`, cả nhánh HD và
   original, tệp của account khác — đều phải còn nguyên.
2. **R06** — cô lập test (subprocess với root tạm trước `ROOT: OnceLock`, hoặc
   fixture nhận root tường minh). Sửa trước khi viết thêm test logout.
3. **R02** — chặn khởi động/nhận event dữ liệu khi `logout_inflight`, xóa state
   tại `LogoutDone`, test hoán vị `PollDone/ThreadsRefreshed/Hydrated` quanh
   `LogoutDone` (Ok/Err/quit).
4. **R03** — worker tải snapshot policy theo owner/runtime/peer (có trạng thái
   unknown/error); render chỉ đọc RAM.
5. **R04** — `StickerImageReady` không được hạ cấp sprite đã có (hoặc xóa
   `sticker_sprites[key]` nhất quán); preloader chuẩn hóa atlas legacy kể cả khi tắt
   animation.
6. **R05, R07, R08** — lease cleanup clipboard; offload memo; tuần tự hóa lưu QR
   và báo lỗi qua `StatusUpdate`.

## Gate tại `00542b6`

Gate local không hợp lệ: một agent khác sửa source trong cùng working tree lúc
gate chạy, nên clippy/test lỗi compile trên code đang dở, không phải `00542b6`.
CI Windows ARM64 tại `00542b6` success. Chi tiết và gate hợp lệ tại `339578e`:
[VERIFICATION.md](VERIFICATION.md) mục "Tái kiểm `00542b6` và `339578e`".
Kết quả sửa R01–R08: [REAUDIT_339578E.md](REAUDIT_339578E.md).

## Giới hạn

Đây là kiểm toán source. Không chạy gửi thật, không tái hiện native, không đo
latency. Native ARM64, GPU/DPI, Hello/PIN, micro và Zalo live không được chứng
nhận. E2EE reset là policy cục bộ; hợp đồng Zalo vẫn Unknown — AMBIGUOUS.
