# Phương án triển khai UI/UX, Settings và dữ liệu sẵn sàng — Codex

Ngày: 24/11/2026. Trạng thái: kế hoạch triển khai; chưa nghiệm thu các gói bên dưới.
Workspace: `C:\Projects\ZaloLite`. Đây là phương án do Codex đề xuất theo yêu cầu người dùng,
không phải bản duyệt nguyên văn kế hoạch Antigravity. Không sửa mã ứng dụng trong lượt lập kế hoạch.

## 1. Kết quả sản phẩm và phạm vi

Người dùng mở ứng dụng phải thấy dữ liệu cơ bản dùng được, hoặc trạng thái tải thiếu/offline
có giải thích. Settings có cấu trúc rõ, thao tác thật, không mở một popup tiện ích chứa nhiều
nghiệp vụ không liên quan. Composer, sidebar, timeline, drawer và dialog dùng cùng ngôn ngữ
thiết kế; thay một phần không làm mất draft, sai quyền, hỏng sync hoặc chặn input ở phần khác.

Ba kết quả phải bàn giao riêng: implementation, kiểm chứng offline, nghiệm thu native.
Không dùng tên file đã xóa, số tab hoặc tổng test passed làm thước đo thay cho hành vi.

Phạm vi gồm Settings và điều hướng các chức năng đang nằm sai chỗ; modal/input; startup và
consumer config; composer/file/voice/emoji trong các bề mặt liên quan. Không viết lại
transport, crypto, call engine, SQLite schema hoặc toàn bộ GUI chỉ để đổi hình thức.
Một gap thực ngoài phạm vi được ghi lại và chỉ xử lý khi cần cho lát chức năng đã chọn.

Tài liệu tham khảo Antigravity:
[bản audit cũ](2026-11-24-zalo-parity-ui-ux-and-settings-audit-master.md).
Các mệnh lệnh xóa file, đổi hash, command/route minh họa và kích thước cố định trong bản đó
không phải chỉ dẫn thực thi của phương án này.

## 2. Nguồn chuẩn, baseline và quyết định kiến trúc

Tuân thủ `AGENTS.md`, `GLOSSARY.md`, standards và ADR hiện hữu; tài liệu này điều phối công việc,
không thay định nghĩa invariant, schema hoặc lifecycle ticket. Giữ ID spec/task đang có.
Các mã UXS bên dưới chỉ là mã gói điều phối, phải map tới task hiện hữu trước khi sửa.

Đọc trước khi lập patch:

- `docs/standards/ui-ux-consistency.md` và standards thuộc miền sẽ sửa.
- `docs/ZALO_REFERENCE_ANALYSIS.md`, `docs/ZALO_GUI_UX_OVERHAUL_PLAN.md`,
  `docs/GUI_UX_HANDOVER_AI_GUIDE.md`; nhãn hoàn tất lịch sử không chứng nhận bản hiện tại.
- [Hồ sơ sửa gần nhất](2026-11-24-sync-startup-hidden-remediation.md).
- [Đặc tả preload/sync](2026-11-24-zalo-native-preload-and-sync-parity-spec.md),
  `re-notes/73-sync-origin-timeouts-and-startup-completion.md`.
- `specs/004-desktop-interaction/` và spec history/media/server-media liên quan;
  `docs/releases/v0.007-arm-plan.md` là bản đồ liên kết phạm vi, không tự mở release mới.

Baseline lúc viết: HEAD `a5ad2b14c6d5b6bcd3ca59ed385f75f4fbff6b0c`, working tree có sửa
chưa commit. Hash 197 file source/test/toolchain theo hồ sơ sửa:
`3fdcec24a5023039cbaf601fbcd2702ebc01893c1a80751b1941baf35d02085f`.
Library 1169 passed/11 ignored, backup 4 passed, stress 180 passed, Clippy/fmt qua.
Đây là bằng chứng của candidate cũ trong phạm vi sửa cũ, không được chép thành PASS cho patch mới.
Antigravity phải kiểm tra lại HEAD, diff và timestamp; không reset working tree về baseline này.

Các quyết định cần giữ:

| Nội dung | Quyết định triển khai |
| --- | --- |
| Kiến trúc | Rust/egui, Tokio worker, SQLite account-local; tái sử dụng service và state hiện có |
| Settings | Một host điều hướng, mỗi nhóm nghiệp vụ có renderer nhỏ; tách file khi giảm coupling thật, không framework form tổng quát |
| Modal | Workspace < backdrop < dialog < child; dùng cơ chế đã sửa và kiểm tra consumer, không làm lại chỉ vì còn `egui::Window` |
| Dữ liệu | Worker giữ I/O và operation; UI nhận snapshot/result scoped rồi render; local view state rẻ vẫn ở UI |
| Scope | Owner/runtime và request generation; thêm operation/thread/revision khi nghiệp vụ cần, không dùng một ID thay mọi ý nghĩa |
| Startup | Tách session validity, local cache usability, baseline data readiness, enrichment; không thêm sleep để kéo dài loading |
| Giao thức | Giữ route/frame/cipher hiện có nếu chưa có bằng chứng mới; 551 là Signal 1–1, sync hiện tại 590/591/592 |
| PIN | Giữ PBKDF2-HMAC-SHA256 và migration hiện có; app lock, hidden-chat PIN và mã hóa dữ liệu là ba khái niệm riêng |
| Hoàn tất | Server ACK, readback nếu có, durable import và coverage theo nghiệp vụ; không suy từ toast hoặc progress 100% |

## 3. Quy chế bằng chứng và UI/UX

Mỗi kết luận phải có nhãn USER, UI-LIVE, E1, E2, CODE, FIXTURE hoặc E3/AMBIGUOUS theo
đặc tả preload. UI-LIVE không tự chứng minh giao thức. E2 ghi bundle/hash/marker; E1
ghi capture đã che bí mật và đúng bối cảnh. Không route-index -> schema/entitlement.

Quan sát Settings cần screenshot cùng viewport, DPI/scale, theme, version, tài khoản thử
và trạng thái feature gate; DOM/CSS đo được nếu có. Không đóng đinh 800×800 hoặc 10 tab
từ một ảnh. Phân biệt kích thước CSS/logical/physical; clamp theo vùng cửa sổ thực tế.
PC là nguồn hành vi chính, Web là đối chiếu riêng, không ghép hai nền tảng thành một UI giả định.

Mỗi bề mặt phải mô tả placement, hierarchy, token, spacing, text overflow, hit target,
focus và các trạng thái loading/empty/error/unsupported/pending/confirmed/cancel.
Đo text bằng font thật; không sửa copy/config hoặc tăng min-width toàn panel để che clipping.
Tái dùng theme/widget hiện hữu; ngoại lệ local phải có lý do. Dialog con hợp lệ được giữ,
không biến xác nhận phá hủy hoặc native folder picker thành một hàng toggle.

Một control cần I/O phải có đường đi đầy đủ. Lựa chọn tab/đóng preview không cần network.
Không thêm toggle mà backend chưa hỗ trợ. Unknown phải thể hiện trung thực; một nhãn giới
hạn có ích không phải stub. Không để placeholder chiếm chỗ như một tính năng đã hoàn thành.

## 4. Hợp đồng chung trước khi chia code

Điều phối viên ghi hợp đồng cho mỗi lát chức năng, dựa trên struct thật:

1. Intent, nguồn UI, phạm vi account/device/thread; nơi lưu, consumer và hiệu lực sau restart.
2. Command hiện có hoặc phần mở rộng tối thiểu; điều kiện admission, single-flight/dedup.
3. Event payload, scope, pending/terminal states, hủy và kết quả đến muộn.
4. Thành công được xác nhận ở đâu; lỗi persist/readback/unknown ACK được thể hiện thế nào.
5. Resource owner, lock order, timeout/budget và cleanup khi đóng/tab-switch/logout/runtime đổi.
6. Regression và native scenario; file được phép sửa; consumer ngoài file phải kiểm tra.

Không phát minh struct/field theo tên trong tài liệu. Đọc định nghĩa trước khi viết field access.
Không giữ lock xuyên network await; DB và công việc nặng chạy ngoài UI, account write permit
được giữ đúng phạm vi. Snapshot revision phải gắn thứ tự đọc/ghi thật, không gắn thứ tự enqueue.

Local settings phải phân loại device-wide/account-local, migration/default và thời điểm áp dụng.
Lưu lỗi không được biến thành “đã lưu”; không ghi disk trong mỗi frame/drag tick. Dùng đường
lưu hiện có nếu đúng hoặc coalesce qua worker nếu cần sửa I/O; không viết lại mọi settings.
Server settings chỉ hiển thị giá trị được biết; phản hồi thiếu không biến thành false/rỗng.
Mutation lỗi/ACK mơ hồ không tự retry; giữ draft và có cách refetch/khôi phục UI phù hợp.

## 5. Các gói triển khai cụ thể

### UXS-00 — Inventory và khóa hợp đồng (bắt buộc đầu tiên)

Đọc `gui.rs`, `modals.rs`, `workspace.rs`, `tools_panel.rs` và từng module thật liên quan.
Ghi bảng trong task/verification hiện hữu:
`requirement | entry point | state | command | backend | event | persistence/consumer |
gap | evidence | target surface | test | native status`.

Phân loại đã có / sai vị trí / hỏng / thiếu / unknown, không coi toàn bộ kế hoạch là chưa làm.
Ví dụ đã có: nút Aa, preview tệp trong composer, scoped sync Settings, modal chờ,
watermark hidden, revision preferences/pins và PIN PBKDF2. Kiểm tra lại thay vì xây bản thứ hai.

Đầu ra: dependency graph, contract từng patch đầu, bảng file owner, test baseline và
ba patch đầu trong mục 8. Hoàn tất khi mọi intent của patch có consumer và terminal state.

### UXS-01 — Settings host, modal và điều hướng

Giữ `SettingsTab` và host hiện có ở bước đầu. Đo layout, sửa container/scroll/heading và
chuyển nội dung theo nhóm nhỏ, không đồng thời đổi enum, state, worker và tất cả tab.
Đọc `AppSettings` trong `src/gui.rs`; không giả có `src/settings.rs`.

Chia vùng navigation/content; viewport nhỏ có navigation thích ứng và cuộn dọc; không
scroll ngang mặc định để che layout sai. Giữ lựa chọn tab, pending và focus khi resize/theme.
Parent chỉ bị khóa khi child sở hữu input; đóng child phải dùng lại parent được.
Không liên tục load thiết bị/network trong render; entry/tab activation chỉ phát một intent
khi chưa có kết quả/pending hợp lệ, retry là thao tác riêng.

Kiểm tra shared backdrop, workspace gating, PIN child và phone wait trước/sau thay đổi.
Không thay Foreground bằng Tooltip, bỏ hit-test backdrop hoặc nâng egui để né lỗi.

Đạt khi: mở -> idle -> outside press/release -> trong dialog -> X/Escape -> mở lại;
parent -> child -> close -> parent; không gửi tin/chuyển chat phía sau; không ghost blocker.

### UXS-02 — Di chuyển chức năng khỏi Tools, bảo toàn nghiệp vụ

Không xóa `tools_panel.rs` hoặc `self.tools` trước khi các dependency đã được chuyển.
Tách command/result khỏi renderer chỉ nếu cần để di chuyển caller; không thay hợp đồng
backend chỉ vì module có tên Tools. Mỗi dòng dưới là một migration riêng có fallback rõ:

| Chức năng cũ | Đích sản phẩm | Hành vi cần giữ |
| --- | --- | --- |
| Search/OpenHit/OpenThread | Search toàn cục/trong chat; mở hidden theo gate | Pagination, generation, scroll tới hit, không lộ hidden |
| Import/export/backup | Settings dữ liệu/đồng bộ, workflow tiến trình phù hợp | Dry-run, account binding, dedup, cancel, báo lỗi, không ghi đè dữ liệu |
| Hẹn giờ | Composer và danh sách lịch gửi | Draft, sửa/hủy, worker scheduling, trạng thái lưu thật |
| Tự trả lời | Nhóm automation riêng nếu giữ chức năng fork | Rule thật, account scope, pause/resume; ghi rõ fork deviation |
| Giao việc/bình chọn | Ngữ cảnh nhóm; To-Do toàn cục chỉ khi có bằng chứng | Quyền nhóm, pending/mutation result; không tráo hai nghiệp vụ |
| Quick messages | Settings quản lý + composer áp dụng | CRUD có persistence, thao tác chèn không tự gửi |
| Media/My Documents | Drawer/kho nội dung tương ứng | Cursor, entitlement, expiry, viewer/download và lỗi |
| Thiết bị/bảo mật | Settings tương ứng | Không tạo 2FA toggle khi protocol chưa biết |
| Theme/sticker/config export | Appearance/Messages hoặc bề mặt nâng cao có lý do | Không đồng nhất cấu hình local với remote config |
| Doanh nghiệp/LAN | Theo capability đã xác minh | Unknown không triển khai giao thức mới; bỏ affordance không hữu ích sau inventory |

Điều kiện bỏ entry cũ: entry mới được kiểm tra end-to-end, không mất khả năng người dùng
đang dùng; nếu chủ đích bỏ tính năng thì ghi riêng quyết định phạm vi, không âm thầm xóa.
Chỉ xóa file/state sau khi rg tất cả caller, result handler, pending jobs, test và cleanup
không còn phụ thuộc; compiler pass chưa đủ chứng minh không làm mất chức năng.

### UXS-03 — Hoàn thiện từng nhóm Settings

Nhóm và nhãn cuối dựa trên bằng chứng, không bắt buộc đủ 10 tab. Thực hiện từng hàng:

| Nhóm | Hướng triển khai | Biên lỗi cần kiểm tra |
| --- | --- | --- |
| General | Ngôn ngữ, startup/tray, nhớ login, download dir qua adapter hiện có | OS action thất bại, path không ghi được, restart đọc lại, không giả áp dụng |
| Security | App lock/PIN/Hello theo capability thực, tách profile khỏi credential | PIN sai/legacy migration, Hello unavailable/cancel; không downgrade KDF |
| Privacy/hidden | Dùng module privacy/hidden; quản lý PIN ở Settings, unlock tại ngữ cảnh | Missing field, unsupported, readback lỗi, relock, runtime đổi, không lộ nội dung |
| Sync/backup | Dùng coordinator, origin/range đúng và backup workflow hiện có | Pending admission, double click, cancel/resume, partial coverage, unknown ACK |
| Storage | Thống kê account/cache/media/DB có provenance, thời điểm đo | Permission denied, file đổi trong scan, reparse point, cancel, account switch |
| Appearance | Tokens, theme mode, scale và preview thật | System theme đổi, DPI/window nhỏ, không mất draft/scroll |
| Notifications | Dùng notification policy, mute/hidden và audio backend | Khóa app, muted/hidden, mention policy, runtime cũ; preview không lộ nội dung |
| Messages/composer | Send-key policy, sticker/link suggestion, quick messages theo capability | IME commit, selection, attachment caption, draft theo thread; mỗi chế độ gửi rõ ràng |
| Calls/devices | Enumeration, lựa chọn, meter/test/preview qua resource owner | Hot-plug, permission, busy device, tab close, incoming call, logout |
| Utilities | Screen capture/hotkey hoặc chức năng có backend thật | Capture cancel, restore window, modal focus, không tự thêm global hook |

Không gắn UI hoàn tất cho một hàng khi chỉ lưu được bool. Cần kiểm tra consumer thật:
ví dụ đổi microphone phải ảnh hưởng session capture mới theo policy, không chỉ đổi label.
Không làm toàn bộ hàng cùng lúc; ưu tiên chức năng đã có backend và gap đã tái hiện.

### UXS-04 — Startup, sync và config consumer (ưu tiên lỗi dữ liệu trước đánh bóng)

Thực hiện gói A/B/C/F/G của đặc tả preload đã liên kết; không tạo coordinator thứ hai.
Trước đổi Ready, vẽ dependency từng nguồn: DB/cache, auth/config, baseline conversations,
preview, alias/hidden/privacy/unread/pin cần thiết, socket/queue và enrichment.
Truy thân manager và consumer để chốt phần bắt buộc; không đợi mọi group member/avatar/quota.

Khi nguồn lỗi: giữ cache hợp lệ, không biến parse/request lỗi thành empty/success. Snapshot
baseline được áp vào UI trước Ready; timeout chỉ mở degraded state có lý do, không báo đầy đủ.
Cold/warm/offline phải được chứng minh qua producer thật có barrier, không chỉ mock enum UI.
Metadata không sẵn sàng phải giữ privacy gate phù hợp; không mở hidden bằng dữ liệu mặc định.

Lập bảng config: key -> producer/thời điểm -> typed field -> owner/runtime/revision ->
consumer -> default/invalid handling -> operation snapshot/live effect -> test.
Bao gồm service routing, cipher gate, alias v2, zcinput, file restriction/autozip,
download domain swap và sync flags/timeouts/banners. Không coi tải JSON là dùng config xong.
URL/host đi qua resolver; thay config giữa transfer không tùy tiện đổi chính sách operation.
Giữ autozip/error204 regression; không thay phần đang chạy khi chỉ chỉnh file-card UI.

Banner Settings/auto/retry cùng state thật nhưng origin khác nhau. X là dismiss hay cancel
phải chốt theo state/bằng chứng và copy; không âm thầm đổi semantics. Chờ phone không resend
khi contract chưa xác minh. Completed round không đồng nghĩa coverage toàn tài khoản.
Chỉ báo hidden chỉ gợi ý trung tính, không tiết lộ danh tính/count/content hoặc mark-read.

### UXS-05 — Composer, file card, timeline và voice

Giữ toolbar -> attachment tray -> editor/caption/send trong cùng composer container.
Rà add/remove/clear, nhiều tệp, tên dài có extension, đổi chat, lỗi upload, draft restoration,
clipboard/drop và IME. Không tạo hàng tin thứ hai khi tiến độ/retry cập nhật optimistic row.

Bubble/header/avatar/grouping/receipt dùng token và payload thật. Đã gửi/nhận/xem dựa trên
ACK/receipt/privacy; không đổi state chỉ vì upload xong. File-card phân biệt queued,
zipping/hashing/uploading, sent, downloading, downloaded/openable, expired và failed.
Không auto-open file nhận được; Open/Reveal chỉ khi người dùng chọn và path/state hợp lệ.

Voice dùng player thật: duration, seek/progress/speed nếu backend hỗ trợ. Waveform chỉ lấy
từ metadata/decoded audio thật; khi chưa có thì dùng pending/placeholder có chủ đích,
không vẽ biên độ ngẫu nhiên. Decode/downsample/cache ngoài UI; đổi chat/đóng phải cleanup.
Không chốt 24 vạch, avatar seen 14px hay màu bubble từ tài liệu tham khảo khi chưa đo.

### UXS-06 — Emoji màu và tính đúng của text

Làm prototype đo trên egui/epaint 0.29.1 đang pin. Thêm color font không tự bảo đảm pipeline
outline hiện tại vẽ được màu. So sánh adapter texture/atlas với chi phí can thiệp rasterizer;
ưu tiên giải pháp nhỏ có dữ liệu/license rõ, không nâng egui trong cùng patch.

Giai đoạn đầu chỉ đọc: tên sidebar/header và message text. Xử lý emoji theo grapheme/RGI
sequence, ZWJ, màu da, cờ, VS15/VS16; giữ Unicode gốc cho copy/search/accessibility.
Layout phải thống nhất baseline, đo width, line break, ellipsis và selection; không chèn
widget ảnh rời làm lệch hit-test/copy. Cache có budget, fallback cho sequence chưa hỗ trợ.
Editor là bước sau: mapping byte/char/grapheme/cursor, IME, undo/redo và clipboard phải rõ.
Không nhân rộng prototype sang mọi bề mặt trước khi đo glyph, CPU/RAM/GPU và native DPI.

### UXS-07 — OS/media lifecycle và storage cleanup

Tái dùng media/voice worker và device adapter hiện hữu trước khi thêm actor mới.
Preview có token owner/runtime/request, stop idempotent, quyền truy cập và lỗi terminal.
Meter dùng giá trị mới nhất, coalesce và repaint có giới hạn, không queue mọi audio callback.
Tab switch/close/logout/call start/hot-plug phải release thiết bị theo policy đã chốt;
không cướp microphone đang phục vụ cuộc gọi. AEC/noise reduction chỉ hiện nếu pipeline có tác dụng.

Storage scan bounded/cancellable, không follow reparse points ra ngoài root; phân loại
cache tái tạo, downloaded media, file người dùng lưu và DB/WAL. Cleanup phối hợp resource
đang dùng, không đụng session/DB/outbox/draft hoặc file ngoài managed root. Không xóa media
lâu ngày bằng một toggle chưa có policy/consent/khả năng khôi phục rõ.

### UXS-08 — Tích hợp và nghiệm thu

Theo ma trận mục 9. Không tăng phạm vi vì có thể làm thêm; gap unknown được nêu rõ,
các gói độc lập vẫn tiếp tục. Một lát hoàn chỉnh tốt hơn nhiều tab mới chỉ có UI.

## 6. Chia subagent và quyền sửa file

Không mặc định IDE có model tên cụ thể hoặc tự chuyển model được. Dùng tên model thật
có trong môi trường; Pro là vai trò advisor/reviewer, Flash là implementer theo yêu cầu.
Nếu chỉ có một model hoặc ít slot, chạy tuần tự cùng hợp đồng. Không thêm agent để bù kế hoạch mơ hồ.

| Vai trò | Công việc | Quyền |
| --- | --- | --- |
| Điều phối/integration | Inventory, dependency/contracts, lease file, tích hợp core, chạy build, giữ checkpoint | Chủ duy nhất của shared core và task/verification chính |
| Pro advisor | Truy bằng chứng PC/Web, review hợp đồng và diff rủi ro, phản biện phương án | Read-only; không sửa code, không tự thực hiện mutation tài khoản |
| Flash UI | UXS-01/02/03/05 theo patch được giao | Chỉ file được lease; thay đổi core gửi đề xuất cho integrator |
| Flash data/lifecycle | UXS-04/07 và consumer config theo patch | Chỉ file được lease; hợp đồng event/API core qua integrator |
| Reviewer/test | Tình huống đối nghịch, kiểm tra coverage, đề xuất regression và native scenarios | Read-only mặc định; test file chỉ được sửa khi lease riêng |

Đây là các vai trò theo giai đoạn, không yêu cầu 5 agent đồng thời. Mặc định tối đa hai
implementer chạy song song; advisor/reviewer luân phiên theo slot. Mọi writer dùng cùng
baseline đã đọc và patch mục tiêu nhỏ; không đưa toàn bộ lịch sử chat/capture nhạy cảm vào context.

Shared core mặc định gồm `src/gui.rs`, `src/gui/worker.rs`, `src/gui/workspace.rs`,
`src/gui/modals.rs`, theme/shared widgets, `src/store.rs`, `src/session.rs`, `src/socket.rs`,
`src/endpoints.rs`, Cargo files và task/verification chính. Chỉ integrator sửa các file này.
Subagent đưa patch đề xuất có context cho integrator áp dụng; không lén sửa core để “cho build qua”.
Nếu cả hai task cần cùng module feature, chạy tuần tự hoặc chuyển lease sau checkpoint.

Lease ghi: task, agent, exact paths, baseline hash, API contract, bắt đầu/kết thúc và kết quả.
Không hai writer cùng file. Không `cargo fmt --all` khi writer khác đang sửa; chỉ integrator
format tích hợp khi tất cả writer đã dừng. Chỉ một người chạy Cargo/build vào target chung.
Không tự tạo branch/worktree, reset/clean/stash, commit/push/release từ kế hoạch này.

Job giao subagent phải có:
`mục tiêu | ngoài phạm vi | evidence | đọc file/hàm | input/output contract | owned files |
guards | regression | native scenarios | deliverable | giới hạn/thao tác không được làm`.
Report phải nêu diff, caller bị ảnh hưởng, checks thật, failure/ignored và pending; không
trả “xong” chỉ vì có code. Advisor kiểm tra từng acceptance, không chỉ nói “LGTM”.

## 7. Dependency và thứ tự tích hợp

```text
UXS-00 inventory/contracts
  -> UXS-01 Settings host/input
  -> UXS-02 migration từng chức năng -> UXS-03 từng nhóm Settings
  -> UXS-04 readiness/config (không đợi hoàn thành toàn bộ Settings)
  -> UXS-05 composer/timeline và UXS-06 emoji prototype theo gap đã đo
UXS-03 Calls/Storage cần UXS-07 lifecycle trước khi thêm control
Mỗi patch -> kiểm tra tích hợp; toàn đợt -> UXS-08
```

Sau UXS-00, lane UI và data có thể phân tích song song. Coding chỉ song song khi file
và hợp đồng không chồng nhau; shared worker/model integration vẫn tuần tự. Không chia agent
theo “mỗi agent vài tab” nếu tất cả đều cần sửa modals.rs/gui.rs/worker.rs.

## 8. Ba patch đầu phải cụ thể

1. **P1 — Settings host và input:** inventory renderer, consumer/backdrop, map current tabs;
   sửa gap layout/input đã tái hiện bằng renderer hiện có. Không xóa Tools, thêm API hoặc
   thay PIN. Regression nhiều frame + parent/child + kích thước/theme. Nếu gap đã sửa thì
   ghi verification và chọn gap thực tiếp theo, không tạo diff chỉ để hoàn thành P1.
2. **P2 — Một migration hoàn chỉnh:** ưu tiên entry backup/import đang có sang nhóm Settings
   phù hợp bằng tái dùng workflow/worker. Giữ dry-run/cancel/account lock; không đổi format
   backup. Kiểm tra entry mới tới payload/progress/result; chỉ sau đó bỏ entry cũ tương ứng.
   Nếu kiểm tra cho thấy đã đúng vị trí thì chọn một dòng sai vị trí thật trong UXS-02.
3. **P3 — Một gap readiness/config có bằng chứng:** từ timeline cold/warm chọn dependency
   bắt buộc đang công bố muộn hoặc error đang bị coi empty; sửa producer -> snapshot -> UI
   trước Ready, kèm barrier/error/stale-runtime regression. Không gom tất cả manager và
   không thay scope lịch sử trong cùng patch. Nếu chưa đủ nguồn để chốt prerequisite,
   làm inventory/fixture đo đường đi, tiếp tục gap độc lập đã rõ thay vì đoán.

Mỗi patch ghi trước/sau, exact files, contract, giữ gì, test và native case trước khi code.
Pro review kế hoạch là checkpoint nội bộ, không phải yêu cầu người dùng duyệt lại công việc
đã giao. Sau checkpoint đạt, tiếp tục triển khai các gói còn lại theo ưu tiên gap thực.

## 9. Kiểm chứng, vòng audit và báo cáo

| Nhóm | Ca bắt buộc khi liên quan |
| --- | --- |
| Layout/text | Sidebar 190/240/320pt; Settings viewport 640×480, 1000×800, màn rộng; chuỗi Việt dài/liền, tên file/emoji, light/dark |
| Native scale | Windows 100/125/150/200%; screen rect thực sau scale; không suy native từ mô phỏng pixels_per_point |
| Input | Outside/inside/X/Escape/reopen, parent/child, wheel, focus restore, IME preedit/commit, hotkey và clipboard/drop |
| Lifecycle | Close/tab-switch/cancel/error/retry, logout/account switch/relogin cùng UID runtime mới, kết quả sai thứ tự |
| Data | Cold/warm/offline, nguồn chậm/lỗi/empty/malformed, server thiếu quyền, snapshot cũ, cache persist failure |
| Sync/media | Origin/range đúng, chống lặp, unknown ACK, partial coverage, autozip, optimistic row, URL expired/player/device busy |
| Destructive local | Dry-run, managed root/reparse path, active download, DB/WAL/draft/outbox còn nguyên |

Trong patch chạy regression có ý nghĩa cho hành vi; không viết test chỉ lặp implementation.
Sau tích hợp: `cargo fmt --all -- --check`, `cargo check --locked --all-targets`,
`cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` và các integration
liên quan. Dùng toolchain repo và CMake setting theo AGENTS. Build/check không thay test;
test không thay native. Không tái chạy toàn suite vô ích nếu code không đổi và kết quả đã đủ.

Vòng audit: (1) truy dữ liệu/caller/ownership và toàn bộ patch, (2) phản ví dụ/input/lifecycle
và regression phù hợp. Nếu phát hiện lỗi thì sửa, chạy checks bị ảnh hưởng, reset bộ đếm;
chỉ chốt sau hai lượt liên tiếp không finding mới trên cùng source/test fingerprint.
Hai reviewer khác nhau chỉ được ghi độc lập nếu thực sự có; cùng model hai lượt phải nói rõ.
Pending native không bị đổi thành PASS vì audit code sạch. Nếu chưa có điều kiện xác minh,
bàn giao cụ thể build/scenario/expected/pending, không tạo vòng lặp vô hạn hoặc tự đóng task.

Log/timeline chỉ ghi phase, elapsed, count/bytes đã tổng hợp, outcome và request generation
cần thiết; không cookies/PIN/IMEI/raw message. Screenshot che dữ liệu. Không tự gửi tin/gọi/
đổi cài đặt tài khoản thật chỉ vì kế hoạch có native case; bám ủy quyền cụ thể của phiên làm việc.

Đầu ra cuối: baseline/diff, bảng requirement -> code -> checks -> native evidence, config
consumer map, migration table, ảnh cùng trạng thái/scale, resource cleanup cases, hai audit
records, rủi ro/unknown và bước tiếp theo. Cập nhật task/verification hiện hữu, không tạo
trạng thái hoàn tất riêng trong brain. Phase Tracking Matrix chỉ cập nhật merged khi đã
merged thật; kế hoạch này không ủy quyền commit/merge/push/tag/release.

## 10. Cơ sở kỹ thuật tham khảo

- [egui 0.29.1](https://docs.rs/crate/egui/0.29.1): immediate mode, repaint và async integration.
- [Unicode Emoji](https://www.unicode.org/reports/tr51/): emoji sequences và presentation.
- Evidence PC/Web và giới hạn server contract phải truy từ hồ sơ repo, không suy từ hai link này.

Prompt bàn giao: [Antigravity implementation prompt](2026-11-24-antigravity-settings-parity-execution-prompt.md).
