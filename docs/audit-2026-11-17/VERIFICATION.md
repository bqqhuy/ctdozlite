# Kiểm chứng độc lập ngày 17/11/2026

Snapshot audit: `e75e3723c54e42191a2a81ee507f3e0f496c50fc`, nhánh `main`.
Khi bắt đầu, working tree sạch; sau `git fetch origin`, `main...origin/main` = `0 0`.
Audit này chỉ đọc code và tạo tài liệu/probe; không sửa source, commit, push hoặc tạo release.

## Gate mới, không sao chép kết quả lịch sử

Máy Windows x64, Rust pin `1.99.1`. Mỗi suite dùng `CTDOZ_DATA_DIR` tạm mới,
`CTDOZ_ALLOW_TEST_LOOPBACK=1`, `CMAKE_POLICY_VERSION_MINIMUM=3.5`.
Không dùng session hoặc database thật, không đăng nhập Zalo, không gọi micro/Hello thật.

| Kiểm tra | Kết quả |
|---|---|
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo check --all-targets --all-features --locked` | Exit 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Exit 0 |
| `cargo test --locked --no-fail-fast` | Exit 0; 932 passed, 0 failed, 13 ignored |
| `cargo test --locked --no-fail-fast --features experimental-e2ee` | Exit 0; 935 passed, 0 failed, 13 ignored |
| `python -I scripts/tests/test_audit_prepush.py` | Exit 0; 4 tests OK |
| `cargo audit --json` | Exit 0; 0 vulnerability được phân loại, 6 informational warnings |

Số test là tổng mọi dòng `test result:` của tất cả binary và doc-tests. Ignored không tính pass.
Suite mặc định có linker warning LNK4199 về delay-load Media Foundation ở test binary;
không được mô tả log test là tuyệt đối không có warning. Clippy mới vẫn exit 0.

Dependency warnings: `audiopus_sys 0.2.2`, `paste 1.0.15`, `proc-macro-error 1.0.4`,
`ttf-parser 0.25.1` unmaintained; `glib 0.18.5` và `rustix 1.1.5` unsound.
Advisory database commit `02e2517f457a359fc6d3440ea4560c8ea188fee8`, cập nhật 15/11/2026.
Chưa chứng minh các hàm unsound được gọi trên Windows ARM64; không suy ra có khai thác
trong sản phẩm chỉ từ cảnh báo dependency, cũng không gọi dependency hoàn toàn sạch.

## Probe bố cục trên binary hiện tại

Binary `target/debug/ctdozlite.exe` vừa được build từ snapshot audit; chạy bằng process
không console với data root tạm mới và các cờ smoke có sẵn:

```text
--dev --gui --ui-smoke-test <output.png>
--ui-smoke-size 480x360 --ui-smoke-scale 1.5
--ui-smoke-tab phone-sync --ui-smoke-phone-sync
```

Exit **3**: `send=[[240.0 155.0] - [288.0 189.0]]`,
`clip=[[122.0 0.0] - [320.0 148.0]]`.
Nút gửi nằm ngoài clip composer. Đây là kết quả hiện tại trên x64, không chỉ log cũ.
CI ARM64 tại `14743b6` cũng thất bại cùng smoke, exit 3:
[run 43913981053](https://github.com/bqqhuy/ctdozlite/actions/runs/43913981053).

CI snapshot e75e372 [run 43921501480](https://github.com/bqqhuy/ctdozlite/actions/runs/43921501480)
đã kết thúc **failure**. Đây là snapshot cũ, không phải HEAD f2b5e97 sau sửa.
Không lấy workflow của upstream `ctdoteam1110` làm bằng chứng cho fork `bqqhuy`.

## Probe texture sticker

[Probe Rust](probes/sticker_texture_probe.rs) dùng GIF asset thật, decoder `image` và
`egui::Context` hiện có. Ami #02 giải mã 20 frame, sprite ngang `12800×560`,
28.672.000 byte RGBA; nạp vào context có `max_texture_side=8192` gây panic được bắt
trong subprocess thử nghiệm. Probe không khởi động app bằng tài khoản thật.
Đây là tái hiện guard texture egui; không tuyên bố đã chạy renderer ARM64 thực tế.

Các log đầy đủ ở `local-logs/` được bỏ qua bởi Git theo quy tắc `*.log`; file thực thi,
PDB và ảnh probe chỉ là artifact local, không dùng làm source sản phẩm.

## Tái kiểm HEAD sau bản vá bên ngoài

Snapshot mới: `f2b5e9736c26dd81f4ea9342eaabf7594dac124c`. Sau fetch,
local `main...origin/main` vẫn `0 0`. Hai auditor review lại diff, không tái sử dụng
probe panic của source cũ để nói source mới còn lỗi đó.

| Kiểm tra mới | Kết quả |
|---|---|
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo check --all-targets --all-features --locked` | Exit 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Exit 0 |
| `cargo test --locked --no-fail-fast` lần đầu | Exit 101 trước khi chạy test: linker LNK1102 out of memory tại `desktop_backup_v2` |
| `cargo test --locked --no-fail-fast --features experimental-e2ee` | Exit 0; 937 passed, 0 failed, 13 ignored |
| `cargo test -j 1 --locked --no-fail-fast` retry | Exit 0; 934 passed, 0 failed, 13 ignored |
| `cargo test -j 1 --locked --no-fail-fast --features experimental-e2ee` | Exit 0; 937 passed, 0 failed, 13 ignored |
| Smoke local HEAD mới 480×360, scale 1.5, phone-sync | Exit 0; tạo PNG, đã xem ảnh: nút gửi nằm trong composer |
| CI Windows ARM64 HEAD mới | **Success**, gồm tests, native PE smoke và package |

CI chính xác cho fork và SHA mới:
[run 43942850530](https://github.com/bqqhuy/ctdozlite/actions/runs/43942850530).
Job attach release bị skip đúng vì không tạo release. Không dùng CI này để chứng nhận
micro, Windows Hello hoặc Zalo live. Lỗi linker local là lỗi tài nguyên build;
không tính là test assertion fail, cũng không tính suite chưa chạy thành pass.
Ảnh smoke mới nằm ở `local-logs/f2b5-phone-sync.png`; không lấy ảnh/probe lỗi cũ làm
bằng chứng source mới còn lỗi composer. Smoke chỉ assert điều kiện được kiểm, không
chứng nhận mọi control hoặc mọi viewport đều hoàn hảo.

Kết quả source và phản biện cuối: [FINAL_CONSENSUS_AUDIT_2026-11-17.md](FINAL_CONSENSUS_AUDIT_2026-11-17.md).

## Tái kiểm `00542b6` và `339578e`

Máy Windows x64. Mỗi suite dùng `CTDOZ_DATA_DIR` tạm mới,
`CTDOZ_ALLOW_TEST_LOOPBACK=1`, `CMAKE_POLICY_VERSION_MINIMUM=3.5`.

**`00542b6`:** lượt gate local đầu tiên **không hợp lệ**. Trong lúc chạy, một agent
khác đang sửa source trong cùng working tree (6 file, sau này thành `e7d0ed4`).
Clippy và hai suite test lỗi compile E0382 ở `src/gui/worker.rs:5713` của code đang
sửa dở, không phải của `00542b6`. Chỉ `cargo fmt --check` và test Python (exit 0)
được ghi nhận. CI Windows ARM64 tại `00542b6`: **success**
([run 43989277111](https://github.com/bqqhuy/ctdozlite/actions/runs/43989277111)).

**`339578e`** (working tree sạch, `= origin/main`):

| Kiểm tra | Kết quả |
|---|---|
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Exit 0 |
| `cargo test -j 2 --locked --no-fail-fast` | Exit 0; 939 passed, 0 failed, 13 ignored |
| `cargo test -j 2 --locked --no-fail-fast --features experimental-e2ee` | Exit 0; 942 passed, 0 failed, 13 ignored |
| `python -I scripts/tests/test_audit_prepush.py` | Exit 0; 4 tests OK |
| CI Windows ARM64 | **Success** ([run 43996813687](https://github.com/bqqhuy/ctdozlite/actions/runs/43996813687)) |

Không chạy `cargo audit` và smoke bố cục ở lượt này. Kết quả source:
[REAUDIT_339578E.md](REAUDIT_339578E.md).
