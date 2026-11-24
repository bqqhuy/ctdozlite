# Prompt bàn giao Antigravity — 24/11/2026

Prompt điều phối [kế hoạch V2](2026-11-24-zalo-full-parity-master-implementation-plan.md); không thay chính sách, đặc tả hoặc trạng thái nghiệm thu.

## Cách dùng

Mở workspace `C:\Projects\ZaloLite`. Nếu bản Antigravity đang dùng có Planning, dùng khi thiết kế. Đề xuất Pro cho thiết kế/review shared popup, input và ownership; Flash cho patch đã rõ. Đây là khuyến nghị của Codex theo rủi ro dự án, không phải bảo đảm hiệu năng từ Google. Kiểm tra model thực có trong UI; không giả định agent tự đổi được model.

Không cần chỉnh global config, cài workflow/hook hay tạo bộ luật Gemini riêng. Không cố áp tham số API như temperature vào IDE.

Nguồn chính thức đã đọc ngày 24/11/2026:

- [Gemini prompt design](https://aistudio.google.com/docs/prompting-strategies): chỉ dẫn trực tiếp, cấu trúc rõ, ngữ cảnh liên quan và định dạng đầu ra xác định. Prompt cần được kiểm chứng qua kết quả, không có bảo đảm triệt tiêu lỗi.
- [Antigravity models](https://antigravity.google/docs/models): model/thinking tùy phiên bản và quyền tài khoản; danh sách có thể đổi.
- [Antigravity rules](https://antigravity.google/docs/rules): hỗ trợ AGENTS/GEMINI trong workspace; vẫn chỉ định agent đọc file thật, không giả định include/link nào cũng đã nạp.
- [Antigravity artifacts](https://antigravity.google/docs/artifacts): plan/walkthrough/screenshot hỗ trợ review; không thay code diff và kiểm chứng thực.

Prompt dùng patch nhỏ và checkpoint để giảm drift ngữ cảnh; đây là lựa chọn triển khai cho repo. Không yêu cầu công khai suy luận nội bộ. Nguồn chuẩn vẫn là AGENTS, V2, spec và verification hiện hữu.

## Sao chép nguyên prompt dưới đây

```text
Bạn triển khai kế hoạch cải thiện UI/UX và rà sync/config của ZaloLite tại C:\Projects\ZaloLite, chuẩn bị kết quả để Codex audit. Làm code và kiểm chứng theo từng patch; không dừng ở lập kế hoạch. Giữ thay đổi khác trong workspace. Lượt bàn giao này chưa commit/push/tag/release.

1. NGUỒN CHUẨN — ĐỌC FILE THẬT

Đọc:
- AGENTS.md, GLOSSARY.md.
- docs/standards/ui-ux-consistency.md.
- docs/research/2026-11-24-zalo-full-parity-master-implementation-plan.md — EXEC-V2 hiện tại, gồm bổ sung composer attachment và cắt chữ rải rác.
- docs/assets/ui-ux-proposals-2026-11-24/README.md và cả ba ảnh minh họa.
- docs/research/2026-11-23-sync-config-audit-dossier.md — kết luận mới nhất.
- docs/ZALO_REFERENCE_ANALYSIS.md, standards/ADRs và spec/plan/tasks/verification liên quan từng patch theo AGENTS.

Các đường dẫn trên tương đối với C:\Projects\ZaloLite. V2 điều phối phạm vi, không thay spec/ADR hoặc bằng chứng giao thức. Brain cũ và ảnh sinh AI không phải hợp đồng server. Giữ đủ 17 mục và 15 capability composer trong inventory; không mặc định tất cả đều thiếu trong code.

2. CÁCH THỰC HIỆN

- Ghi model/mode nếu môi trường cung cấp; không bịa khả năng tự chuyển model. Pro phù hợp để lập kế hoạch và review thay đổi dùng chung; Flash thực hiện patch đã xác định rõ. Nếu chỉ có một model, giữ cùng quy trình; không tự cài/chuyển dịch vụ.
- Một patch có một mục tiêu hành vi. Đọc định nghĩa struct và producer/consumer thật trước khi sửa. Dùng rg và đọc có mục tiêu, không dump toàn repo lặp lại.
- Trước patch ghi ngắn: vấn đề đã quan sát, hành vi trước/sau, file/hàm, nguồn, guard cần giữ, regression và native steps. Nêu quyết định/căn cứ; không cần trình bày suy luận nội bộ.
- Sau kế hoạch cụ thể, tiếp tục công việc đã giao; không hỏi duyệt lại lựa chọn thường lệ.
- Nếu cùng lỗi kiểm chứng thất bại hai lần hoặc API khác giả định, dừng chiến thuật đó, đọc lỗi/source thật và sửa kế hoạch nhỏ trước khi tiếp tục. Không xóa guard/test, thêm allow hoặc chạy đơn luồng để che race.
- Trước đổi model/hết context: ghi checkpoint vào task/verification hiện hữu gồm baseline, patch hiện tại, diff, checks, finding và bước kế tiếp. Phiên sau đọc checkpoint và git diff; không tạo trạng thái riêng cho mỗi AI.

3. BASELINE TRƯỚC SỬA

Chạy git status --short, git rev-parse HEAD; xem diff và file untracked liên quan. Giữ tài liệu/ảnh Codex và mọi thay đổi có sẵn; không reset/clean/stash hay coi untracked là rác.

Đối chiếu 17 mục V2 với code thực. Ghi bảng:
Task/spec | Code đã có | Gap thực | Nguồn E1/E2/E3 hoặc UNKNOWN | File/hàm | Kiểm chứng.
Giữ ID task/spec hiện có. Phản hồi đầu tiên phải có baseline và kế hoạch patch có thứ tự, sau đó thực hiện; chưa gọi giả thuyết là nguyên nhân đã chứng minh.

4. ƯU TIÊN MP-00: POPUP/INPUT DÙNG CHUNG

Lỗi lịch sử test-0d62e22:
- Settings không nhận tab/X nhưng Escape đóng được.
- Labels mở lại → click ngoài → X làm dialog tối và không nhận click.
Đây là quan sát lịch sử; không báo đã tái hiện trên build mới nếu chưa thử.

Rà render_modal_backdrop, Settings/Labels và mọi consumer, is_any_modal_open/is_workspace_input_gated, preflight call, parent/child và thời điểm gate trong frame. Đọc source egui 0.29.1 đang pin: thứ tự gọi paint không bảo đảm layer cùng Foreground ổn định qua frame; set_sublayer có giới hạn/lifecycle phải tuân theo.

Bảo đảm workspace < backdrop < dialog < child popup theo ownership. Nền bị chặn đúng policy, dialog nhận input, đóng/hủy trả focus, không ghost blocker. Rà pointer press/release, wheel, keyboard/shortcut, IME, drop/paste.

Không bỏ hit-test backdrop gây click-through, đẩy mọi thứ lên Tooltip, move_to_top một lần, thêm sleep/repaint loop/thread hoặc nâng egui để né nguyên nhân. Context menu đã đúng không phải viết lại.

Regression nhiều frame:
open → idle → outside press/release → inside/tab/X → reopen;
parent → child → close child → parent;
không Send/WorkerCmd/đổi thread nền;
close/logout/relogin/resize không còn blocker.
Kiểm tra các consumer dùng chung và native riêng.

5. BANNER VÀ COMPOSER CÓ FILE

Preview tệp chờ gửi PHẢI nằm BÊN TRONG composer: toolbar, khay attachment, ô chú thích và Gửi cùng container/nền/content padding. Không giữ vùng preview rời trên timeline. Ảnh minh họa chỉ định hướng bố cục; không lấy nav/nút/thứ tự icon/kích thước ngẫu nhiên trong ảnh làm yêu cầu mới.

Tái dùng attachment draft và backend đang có. Có add/remove/clear-all, tên/dung lượng/preview và caption theo state thật; không mất draft khi lỗi hoặc đổi layout. Nhiều tệp có overflow/scroll và giới hạn chiều cao, ô nhập/Gửi luôn dùng được.

Đo text/galley, padding, icon, close và clip rect theo width/font/DPI. Tên dài ellipsis có chủ đích, giữ nhận diện đuôi và xem đầy đủ được; clip mất nét/cắt trái là lỗi.

Cắt chữ đã được báo ở cả banner sync và preview file. Rà từng chỗ và primitive dùng chung nếu có bằng chứng; không thêm lề 12px toàn giao diện hoặc đổi chuỗi server để che width sai.

Giữ caption đã có trong file_drop.rs và ADR-0029. Không xóa nguyên guard theo số dòng cũ: giữ path/dedup/budget/FileStamp, owner/runtime/thread, late preview, hit-test drop và chống gửi lặp. Composer drop staging và timeline drop giữ policy hiện hữu.

Banner dùng state/origin/gate/timeout thực; rà mọi kind đang hỗ trợ, configured copy, Start/Stop/cancel/dismiss/reopen. Last-run/progress phải có nguồn. Giữ các đường Sync2/V1/import/history hợp lệ.

6. CÁC PHẦN CÒN LẠI THEO V2

Sau nền popup và banner/composer, tiếp tục từng nhóm MP-UI có đủ bằng chứng: icon/emoji/avatar, timeline/voice/file/rich cards, sidebar/search/header/drawer, Settings presentation. Giữ đủ 15 capability trong inventory nhưng chỉ triển khai gap thật; thiếu backend/contract không dựng nút giả.

Không viết lại archive/unread/hidden/blocksharing/presence/mute như thể chưa có. Waveform dùng dữ liệu thật. Emoji giữ grapheme/ZWJ/selection/copy/offset. File-open giữ worker/policy hiện hữu. Quyền/role/entitlement có nguồn, unknown không thành false success. Responsive tính đủ rail/sidebar/timeline/drawer/padding trước khi đổi breakpoint.

MP-CFG phải rà:
field → nguồn/method/envelope → thời điểm load → validation/default/precedence → cache/store → consumer → effective state → owner/runtime → kiểm chứng.
Không dừng ở tải/lưu JSON. Tách remote policy, account preference có setter chứng minh và local/device preference.

Bảo toàn các ranh giới:
- auto_sync.enable là remote gate chưa chứng minh setter, không tự tạo toggle ghi vào nó. AUTO_SYNC/SETTING/BANNER khác nhau; timeout snapshot theo operation; giữ Legacy V1 deviation và cache-before-Ready/optional-settle.
- Không generic POST mọi AppSettings lên server. Privacy dùng typed read-before-write/ACK/readback; trường mới cần contract riêng.
- Retry array theo size bucket khác attempt backoff; inputColumnLayout 0/1 khác expanded ratio 2/3. Dùng bằng chứng V2, không hardcode lại số V1.
- Giữ auto-ZIP/error_code 204, FileStamp, clientId/idempotency và unknown ACK; không suy HTTP 204 là cùng lỗi, không tự giải nén phía nhận. ZIP-08 native còn pending nếu chưa thử thật.
- Manual unread marker khác cursor/mention/receipt; msgType=20 phải đúng namespace.
- VoIP Receive/Miss/Reject/Cancel/Timeout/AnsweredElsewhere khác nhau; đã có preflight/watchdog/actor checks. Không nới/thu hẹp identity hoặc dọn global ringtone từ stale event mà bỏ scope account/runtime/call.
- CMD551/552 dispatch không chứng minh E2EE interoperability; giữ opt-in/fail-closed, không plaintext fallback.
- zCloud usage không chứng minh backup upload/snapshot restore. Không sao chép 15 SQLite shards chỉ để giống native.

Đặc biệt giữ MetadataResultKind, request identity/cutoff, producer revision/privacy watermark, metadata admission, owner/runtime fences, privacy gate, monotonic cache merge, local delete/recall protections.

Contract chưa có bằng chứng thì ghi AMBIGUOUS và nghiên cứu nguồn; không bịa endpoint/schema/enum. Tiếp tục phần độc lập đã được giao.

7. KIỂM CHỨNG

Mỗi patch chạy regression quan sát hành vi và fmt/check phù hợp. Cuối candidate chạy, đọc kết quả và ghi exit code thực:

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -j1 -- -D warnings
cargo test --locked --lib -j1
cargo test --locked --test desktop_backup_v2 -j1
powershell scripts/audit-prepush.ps1

Không coi process đang chạy là PASS; ghi failed/ignored/timeout/lỗi môi trường. Không đổi assertion cho xanh hoặc lặp suite vô ích khi source chưa đổi.

Native UI theo quy chế: trước/sau cùng state; light/dark; width hẹp/thường/rộng, sidebar resize/drawer; DPI100/125/150/200; pointer/keyboard/IME/focus khi áp dụng. Ưu tiên multi-frame popup, nhiều file/tên dài/caption, clip sát biên, thread/account transition. Dùng synthetic data hoặc che dữ liệu cá nhân. Không chạy được ghi pending + bước tái hiện; fixture không thay native acceptance.

Không gửi tin/gọi/đổi cài đặt thật ngoài ủy quyền của người dùng. Thiếu quyền native không cản build/harness và công việc độc lập.

Tự review hai lượt cùng source/test fingerprint:
(1) data flow/ownership/state;
(2) phản ví dụ/error/lifecycle/UI integration.
Finding mới hoặc sửa source/test thì đếm lại. Gọi đúng là tự review, không giả hai reviewer độc lập hoặc kết luận toàn app hết lỗi.

8. HỒ SƠ BÀN GIAO CODEX

Giữ diff trong workspace, chưa commit/push/release. Cập nhật task/verification hiện hữu và walkthrough:
- HEAD nền, dirty files có trước, file patch, source/test fingerprint cuối và cách tính.
- Task → thay đổi → evidence → regression → native result/pending.
- Lệnh thực chạy, exit code, log path; test count lịch sử không phải kết quả patch mới.
- Ảnh trước/sau đã che dữ liệu, build/width/DPI/theme.
- Findings hai lượt tự review, phần đã sửa, AMBIGUOUS, rủi ro/rollback và bước Codex tái kiểm.

Ví dụ báo đúng: “Fixture popup nhiều frame PASS trên fingerprint X; DPI150 native chưa thử.” Không báo “popup đạt 100%” từ fixture.

BẮT ĐẦU NGAY: kiểm tra baseline, lập kế hoạch patch cụ thể rồi triển khai MP-00 → banner/composer → các phần độc lập theo V2. Không chỉ trả lại kế hoạch và lời hứa. Chỉ ghi hoàn tất cho phạm vi đã đủ bằng chứng; liệt kê rõ task thiếu contract hoặc native access.
```

