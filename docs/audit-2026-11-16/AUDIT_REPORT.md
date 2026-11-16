# Báo cáo audit mã nguồn và đồng thuận phản biện

> Bổ sung sau phản biện báo cáo Antigravity: xem [ANTIGRAVITY_REVIEW.md](ANTIGRAVITY_REVIEW.md), ghi nhận F18–F21 (4 P2). Phần audit ban đầu bên dưới giữ nguyên 17 finding; danh mục cộng gộp hiện là **21: 3 P1, 14 P2, 4 P3**. Không sửa code.

Ngày 16/11/2026 (Asia/Saigon). Nhánh `main`, HEAD `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`. Cây làm việc sạch lúc bắt đầu. Chỉ tạo tài liệu/log kiểm tra; không sửa, xóa, format mã nguồn, không commit/push/release. “Tối ưu thuật toán” và “xóa code rác” được xử lý dưới dạng đề xuất theo yêu cầu chỉ báo cáo.

## Kết luận

Có **17 phát hiện: 3 P1, 10 P2, 4 P3**. P1 là lỗi cần xử lý trước khi tin cậy các luồng riêng tư liên quan, P2 là lỗi chức năng/độ tin cậy hoặc rủi ro có điều kiện, P3 là vi phạm nguyên tắc/khả năng bảo trì có tác động thấp hơn. Không phát hiện P0 có đủ bằng chứng. Điều này không chứng nhận chương trình không có P0 khác.

Ba sub-agent audit độc lập: `security`, `data_logic`, `gui_quality`. Mỗi agent đọc nguồn trong mảng riêng trước khi nhận nhận định của bên khác, sau đó kiểm tra chéo, loại phản ví dụ và thống nhất mức độ. Báo cáo chi tiết, trigger và đề xuất kiểm thử nằm trong [security.md](security.md), [data-logic.md](data-logic.md), [gui-quality.md](gui-quality.md). Biên bản chấp thuận cuối nằm trong [CONSENSUS.md](CONSENSUS.md).

## Ma trận phát hiện hợp nhất

| ID | Mức | Phát hiện và điều kiện | Nguồn chính | Hướng khắc phục đề xuất |
|---|---|---|---|---|
| F01 | P1 | Recorder/buffer audio không gắn owner/thread và không được hủy khi logout/chuyển profile. Audio thu bởi A có thể được gửi qua tài khoản B hoặc hội thoại đang chọn; micro cũng có thể tiếp tục thu. Cần thao tác gửi của người dùng, không phải tự động gửi. | `src/gui.rs:1020`, `:2921`; `src/gui/workspace.rs:6992`, `:7025` | Gắn account runtime + thread vào recording; hủy recorder/preview/buffer khi đổi owner; kiểm tra owner trước gửi. |
| F02 | P1 | TUI không áp dụng hidden-chat PIN display gate; hydrate/cache và filter/open có thể hiện hội thoại ẩn khi chưa unlock. Đây là bypass bảo vệ hiển thị trong app theo ADR-0034, không phải phá mã hóa DB. | `src/tui.rs:201`, `:258`; `docs/adr/0034-hidden-chat-pin-compatibility.md` | Dùng cùng hidden membership/gate cho list/search/open/history, fail closed khi chưa biết membership. |
| F03 | P1 | TUI báo xóa tin thành công nhưng chỉ bỏ khỏi vector rồi upsert phần còn lại. Hàng DB cũ vẫn còn; hydrate lần sau có thể đưa tin trở lại. | `src/tui.rs:1478`; `src/store.rs:1594`; `src/tui.rs:201` | Áp dụng delete-for-me/tombstone bền vững theo identity; chặn stale poll hồi sinh tin. |
| F04 | P2 | PollDone/ThreadsRefreshed đã xếp hàng trước logout được áp dụng mà không kiểm owner/generation, có thể hiện lại dữ liệu cũ sau logout. Guard lưu cache đã chặn owner rỗng; không kết luận ghi nhầm cache tài khoản. | `src/tui.rs:988`, `:1393`, `:1406` | Token owner/runtime cho events, bỏ event cũ và hủy/drain tác vụ khi logout. |
| F05 | P2 | TUI bỏ ACK gửi; lỗi chỉ có chuỗi status, không định danh tin để chuyển trạng thái thất bại. Hàng optimistic đã lưu có thể còn lại sau restart dù server từ chối. Timeout vẫn phải phân biệt kết quả chưa biết. | `src/tui.rs:804`, `:834`, `:1508` | Event mang client ID + ACK/typed outcome; reconcile/persist đúng hàng, giữ rejected khác unknown. |
| F06 | P2 | TUI thực hiện SQLite/save cache đồng bộ trên đường xử lý UI. Writer cạnh tranh/disk chậm có thể chặn render; busy timeout DB là 5 giây. | `src/tui.rs:477`, `:825`, `:1418`, `:1483`; `src/store.rs:87` | Queue single-writer ngoài UI, terminal events và xử lý lỗi. |
| F07 | P2 | Migration 13 nuốt lỗi introspection/ALTER/PRAGMA và không atomic. Nếu ALTER thất bại nhưng bump thành công, DB có thể bị đánh version 13 trong khi thiếu cột. Đây là nhánh lỗi có điều kiện, không kết luận migration bình thường luôn hỏng. | `src/store.rs:228`–247 | Transaction, propagate lỗi, xác nhận schema rồi mới bump version. |
| F08 | P2 | Bấm lại reaction hiện có xóa hiển thị local nhưng vẫn gửi reaction không rỗng; backend chỉ hiểu empty hoặc `/-clear` là remove. | `src/gui/workspace.rs:3050`; `src/chat.rs:4263` | Tính add/remove một lần, dùng đúng payload, reconcile/rollback trạng thái theo kết quả. |
| F09 | P2 | Voice buffer được clone cả vector mỗi repaint; mở thiết bị, resample và tạo WAV chạy trực tiếp trong UI. Recorder không có giới hạn thời lượng rõ ràng. Chi phí tăng theo audio, có nguy cơ giật UI/tăng RAM. | `src/gui/workspace.rs:3754`, `:6992`, `:7038`; `src/voice.rs` | Chỉ copy khi cần/Arc immutable, giới hạn thời lượng và chuyển device/encode sang worker. Chưa đo FPS. |
| F10 | P2 | Drawer E2EE đưa assurance vô điều kiện, hoặc “bảo vệ toàn diện” chỉ từ trusted_peers; không kiểm trial build/runtime, session, group hay phạm vi media. | `src/gui/workspace.rs:6669`; `src/e2ee_trial.rs:16`; `src/gui/worker.rs:1357` | Render từ trạng thái capability/policy/session và ghi rõ thử nghiệm/chưa bật/chưa xác minh. |
| F11 | P2 | Một số WebSocket send nằm trong selected branch không có timeout; write bị backpressure có thể ngăn polling liveness/cancellation và reconnect. Outer task không có timeout/abort handle bảo vệ. | `src/socket.rs:637`, `:857`, `:898`, `:974`, `:1017` | Deadline + cancellation cho mọi write, kiểm offline bằng sink bị Pending. |
| F12 | P2 | QR verbose in raw response 200 ký tự trước redaction; payload nhạy cảm nằm trong đoạn đầu có thể lọt terminal/log. Việc truncate không phải redaction; cookie cũng còn lộ prefix. | `src/qr.rs:404`–410 | Log metadata có cấu trúc, không raw auth body hay prefix token; test sentinel giả. Không đọc log thật. |
| F13 | P2 | Bundle E2EE chấp nhận signing_identity_key độc lập để xác minh SPK nhưng dùng identity_key khác cho DH/trust, không kiểm proof ràng buộc. Chỉ xác nhận lỗi validation local trong nhánh thử nghiệm; chưa chứng minh MITM, giải mã trái phép hay khai thác Zalo thật. | `src/crypto/prekey.rs:79`; `src/crypto/session.rs:316`, `:558` | Chốt binding từ bằng chứng giao thức, từ chối identity thay thế; regression bundle giả. Unknown — AMBIGUOUS về hợp đồng Zalo. |
| F14 | P3 | Nút mở diagnostics tạo thư mục + mở shell đồng bộ, bỏ kết quả/lỗi; có thể chặn frame khi storage/shell chậm. | `src/gui/tools_panel.rs:1224`; `src/platform.rs:49` | Worker/background event có kết quả và thông báo lỗi. |
| F15 | P3 | Salt đăng nhập dùng PRNG LCG seed thời gian/PID, trái chuẩn CSPRNG cho salt. Không kết luận riêng điểm này cho phép phá mật khẩu. | `src/login.rs:72`–91 | Dùng random helper CSPRNG sẵn có, giữ format theo evidence. |
| F16 | P3 | Fallback host sticker được hardcode ngoài endpoints, trái single-source-of-truth. Đây là nguồn runtime, không phải URL fixture trong test. | `src/chat.rs:3296` | Chuyển hằng fallback sang endpoints, giữ nguyên hành vi/protocol. |
| F17 | P3 | Script prepush gắn nhãn kiểm 4 điểm release nhưng bước 4 chỉ in PRODUCT_VERSION; stub heuristic chỉ warning rồi vẫn báo thành công. Không chứng minh release hiện tại lệch version, cũng không nói compilation gate bị bỏ qua. | `scripts/audit-prepush.ps1:45`–58 | Tách heuristic warning khỏi gate; thực sự đối chiếu 4 điểm khi applicable, kiểm lỗi thay vì quảng bá coverage chưa có. |

## Tối ưu thuật toán đề xuất — chưa thay đổi mã

1. **Voice repaint:** bỏ clone O(S) samples mỗi frame, chỉ chuyển/copy khi preview/send. Tác động lý thuyết từ O(F×S) dữ liệu copy sang O(S) theo hành động; đo allocation/frame, RAM và input latency với audio 1/10/30 phút. Dùng synthetic PCM; không ghi âm thật để benchmark.
2. **Danh sách tin GUI:** `workspace.rs:2601` quét O(N) và tạo visible_indices mỗi repaint rồi lấy tail, dù render limit thường 150. Duyệt ngược đủ K mục hoặc cache theo revision; thông thường O(K), worst-case filter vẫn O(N). Giữ đúng jump, album, call log, star, ordering.
3. **Merge TUI:** `tui.rs:1340`, `:1373` dùng position tuyến tính cho mỗi incoming thread, worst-case O(T×M), O(T²) khi T≈M. Dựng HashMap identity→index mỗi batch để O(T+M); không đổi ordering/identity.
4. **Persistence TUI:** `persist_async:502` clone toàn snapshot, cộng các đường sync khác. Dùng delta + single writer sau đo, giữ tombstone/recall/account isolation và shutdown flush có giới hạn.
5. **Audio callbacks:** F32/I16/U16 trùng cấu trúc có thể gom nhỏ sau khi xác nhận mono conversion/waveform/error semantics. Ưu tiên ownership và performance có đo hơn refactor hình thức.

Không đưa tỷ lệ tăng tốc khi chưa benchmark. Các benchmark có sẵn và kết quả thực chạy sẽ được ghi ở mục kiểm chứng; benchmark có sẵn không tự chứng minh hiệu năng các đề xuất trên.

## Đề xuất dọn code — chưa xóa gì

- Ứng viên nhỏ: wrapper method `ZaloGuiApp::sender_avatar_for` tại `workspace.rs:5052` không có caller nội bộ tìm thấy. Free function cùng tên tại 5039 có caller thật 2720/2723, phải giữ. Cần xét cfg/test/API trước xóa wrapper.
- Một số `allow(dead_code)` thừa trên field đang được dùng (`gui.rs:642`, `:870`, `:1044`) có thể bỏ suppression để compiler tiếp tục phát hiện code chết; không xóa field.
- Không suy “public API ít caller”, feature thử nghiệm, native Windows-only hay file vendor là code rác. Không đề nghị xóa hàng loạt.

## Nghi vấn bị bác bỏ hoặc hạ mức

- LoadOlder có dispatch backend LoadGroupHistory, không phải nút giả.
- MediaNeighbor kiểm owner/anchor và mở payload trả về. TODO album counter không chứng minh payload bị bỏ.
- server_pins có reset trong reset_sync_status, không xác nhận leak từ riêng field đó.
- AccountSwitcher/video_state có caller thật, không phải code chết.
- Upload checkpoint đã dùng prefix/sparse và binary search; không kết luận O(N²) từ comment cũ.
- E2EE binding giữ P2 experimental, hạ đề xuất P1 ban đầu vì chưa có bằng chứng tác động bảo mật mức P1.
- Stale event sau logout chỉ kết luận lộ lại UI; đã loại giả thuyết ghi cache anonymous vì guard thực tế chặn.

## Kiểm chứng và giới hạn

Log kèm trong thư mục này. Host thực chạy là Windows 11 Pro 10.0.26300, Intel i5-12400F, `x86_64-pc-windows-msvc`, rustc 1.99.1, profile debug; không phải nghiệm thu ARM64. Graphify index khớp HEAD nhưng truy vấn tổng quan trả partial/ungrounded nên chỉ dùng định hướng; các phát hiện được đọc nguồn trực tiếp.

| Kiểm tra | Kết quả |
|---|---|
| `cargo check --locked --all-targets` | PASS, exit 0 |
| `cargo test --locked` | PASS, exit 0; 856 passed, 0 failed, 13 ignored; 16 suite result blocks gồm main/doc tests |
| `cargo test --locked --features experimental-e2ee` | PASS, exit 0; 859 passed, 0 failed, 13 ignored; 16 suite result blocks gồm main/doc tests |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS, exit 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | PASS, exit 0 |
| `cargo fmt --all -- --check` | PASS, exit 0; không format mã |
| `cargo audit --json` | exit 0, 0 mục vulnerabilities; 6 informational warnings như dưới |
| Hai benchmark opt-in sẵn có | PASS, mỗi benchmark 1 test, chạy executable vừa build với `--ignored --nocapture` |
| `reproduce_data_models.py` | 2/2 mô hình SQLite offline tái hiện, xem `data-model-results.json`; không phải Rust integration test |

Hai suite Cargo có linker warning LNK4199: các `/DELAYLOAD:mf*.dll` bị bỏ qua vì không có imports tương ứng. Đây là cảnh báo build cần phân biệt với Rust compiler error; không suy ra Media Foundation trên ARM64 hoạt động hay hỏng từ warning x64 này. Clippy pass không đồng nghĩa mọi native linker warning đều không tồn tại.

13 ignored trong mỗi suite không được tính là passed. Hai benchmark được chạy riêng sau đó; các test ignored còn lại chưa được thực thi bằng lượt suite thường. Lần khởi động compilation đầu được dừng trước test execution để chạy lại với `CTDOZ_DATA_DIR` temp cô lập và `CTDOZ_ALLOW_TEST_LOOPBACK=1`. Không dùng dữ liệu session/DB thật. Không bật test codec yêu cầu tải asset bên ngoài.

Benchmark sẵn có (debug, có tải compilation nền nên chỉ là số tham khảo): upload 4096 chunk/7 vòng có avg legacy 1203.229 ms và compact 2.045 ms; retry synthetic 50 terminal failures + 1 visible image, 4 slot/5 ms job/10 vòng có avg legacy 202.411 ms và finite retry 15.675 ms. Các số này so thuật toán cũ mô phỏng với thuật toán hiện hữu, không phải cải thiện do lượt audit này; không dự báo hiệu năng Zalo thật. Chi tiết min/p50/p95/p99 trong `algorithm_benchmarks.log`, `media_retry_benchmark.log`.

`cargo audit --json` trả exit 0: 618 lock dependencies, vulnerabilities.count=0; advisory DB commit `02e2517f457a359fc6d3440ea4560c8ea188fee8` cập nhật 15/11/2026. Có **4 unmaintained** (`audiopus_sys`, `paste`, `proc-macro-error`, `ttf-parser`) và **2 unsound** (`glib`, `rustix`). Không được gọi là “không có cảnh báo bảo mật”. Truy ngược target Windows ARM64 không có glib/rustix1.1.5; các warning lockfile này không tự chứng minh runtime Windows bị ảnh hưởng. Cần theo dõi cả dependency theo nền tảng khi nâng cấp.

Đã đọc/rà các mảng GUI/worker/TUI, session/storage/DB/migrations, chat/message lifecycle, media/audio/call, socket, E2EE và tooling/release ở mức khác nhau. Đây là audit theo rủi ro, không phải chứng nhận từng dòng của toàn bộ source/vendor, không fuzz toàn parser, không kiểm chứng native ABI/driver, không chạy real account hay Zalo live. Các nhận định protocol server không có E1 không được nâng thành hợp đồng đã xác minh; unit/fixture test không chứng minh interoperability.

Các finding chủ yếu là chứng minh luồng từ source; kịch bản tái hiện được mô tả nhưng không ghi là đã chạy GUI/micro/Zalo thực tế. Không đọc session.json, DB hay capture riêng tư của người dùng. Data root test được cô lập theo thông tin log kiểm chứng. Kiểm tra cuối: HEAD giữ nguyên, `git diff --exit-code` và `git diff --cached --exit-code` đều exit 0; Git chỉ ghi nhận thư mục báo cáo mới `docs/audit-2026-11-16/` chưa commit.

## Thứ tự sửa đề xuất cho lượt được phép triển khai

1. F01/F02/F03 và F04: ranh giới owner, hidden gate, delete tombstone, stale events.
2. F05/F08/F10/F12: send outcome, reaction intent, assurance đúng trạng thái, log auth.
3. F07/F11/F13: migration atomic, write deadline, crypto binding có evidence.
4. F06/F09 rồi benchmark các tối ưu; cuối cùng F14–F17 và dọn wrapper/suppression.

Mỗi nhóm cần regression theo failure cụ thể và chạy compiler/test/lint; native ARM64 và Zalo interoperability phải nghiệm thu riêng. Không có sửa code nào được thực hiện trong lượt audit này.
