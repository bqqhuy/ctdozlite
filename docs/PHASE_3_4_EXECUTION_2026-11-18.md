# Phase 3–4: checklist thực thi và bằng chứng — 18/11/2026

## Quy tắc kết luận

Nguồn yêu cầu: `ZALO_PC_MASTER_ACTION_PLAN_AND_GUIDE.md`, MAP-10–29. Bảng dưới kiểm kê toàn bộ mục của Phase 3 và 4, không dùng số phase của overhaul cũ. “Đã triển khai” chỉ mô tả source và đường xử lý; không đồng nghĩa native/live đã nghiệm thu. Hai auditor GPT-6.1 Sol độc lập `/root/phase4_fresh_a` và `/root/phase4_fresh_b` đã rà lại cả hai phase và phản biện chéo, đồng thuận không còn finding source xác nhận chưa sửa. Kết luận phụ thuộc checks cuối bên dưới.

Sai sót điều phối trước đây: báo phase xong khi còn task, dùng compile/source thay nghiệm thu và bỏ sót nhánh dùng chung. Lần này mỗi mục ghi code, verification và phần còn lại; không coi push là bằng chứng phase hoàn tất toàn bộ. Không build binary/release, không tạo tag, không thử gửi/xóa/báo xấu/mời nhóm trên tài khoản thật.

## Phase 3

| Checklist | Triển khai / kiểm chứng source-fixture | Nghiệm thu còn lại |
| --- | --- | --- |
| MAP-15.1 routing, diagnostics | Builder giữ URL logical canonical; Session chọn discovered/remembered candidate. Typed custom DNS, map/restore/account reset; timing HMAC không lộ nội dung/UID. Regression actual Session/DNS/map replacement. | Host/config effective và DNS ngoài loopback trên thiết bị thật chưa đo. |
| MAP-15.2 preview | Debounce theo thread, cache theo session/origin; E2EE policy trước cache/preview. Payload Text có send-time 2s rồi endpoint text rõ, không scrape lần hai. Payload quote/format/mention giữ metadata nhưng đi endpoint riêng và hiện bỏ qua bounded preview. | PC E2 cho preview composer chung, nhưng chưa có caller-chain bằng chứng quote/mention URL được preview. Giữ là scope AMBIGUOUS, không đổi protocol theo suy đoán. Text/link thật và preview native chưa thử. |
| MAP-15.3 failover/reconciliation | Chỉ typed DNS/refusal chắc chắn trước gửi được mutation failover. HTTP/reset/body stall/timeout không replay. Pending khôi phục thành DeliveryUnknown; late ACK/echo không nhân tin hoặc hạ Sent. Cùng client ID từ enqueue. | Delivery/echo trên account thật chưa chứng nhận. |
| MAP-15.4 budgets/status | Queue5s, policy5s, preview2s, request10s, total20s gồm queue. Upload không bị áp deadline text. Queue/Policy/Preview/Attempt/BodyAck/HTTP/Total timings, canceled body guard; taxonomy auth/policy/rate/protocol/transport. | Đây là budget fork được fixture kiểm tra, chưa có live latency tuning. |
| MAP-15.5 fixtures/native/live | Actual Session loopback 401/403/404/429/500/503, nonzero envelope, DNS preflight, reset và body-stall; một mutation. Cache/deadline/echo regressions. | Native/live CHƯA CHẠY; không sửa hosts/DNS thật hoặc tự gửi lại tin Henry cũ. |
| MAP-10 E2EE UI | Bỏ nút/tab/badge thử nghiệm tại header/drawer/settings/tools; giữ engine và fail-closed policy. | Native kiểm tra còn glyph/label thử nghiệm ở mọi kích thước chưa chạy. |
| MAP-11 sync banner | Banner mỏng trên đầu, đóng cả running/error, không tự mở lại cùng chu kỳ; success tự ẩn sau3s. | Native geometry/theme/progress chưa chạy. |
| MAP-12 responsive/quote/reaction | Drawer overlay ở compact<780, Info button và transition; quote strip xanh/thumbnail/cancel; sáu reaction qua backend. Compact Area bị chặn khi modal. | Native hẹp/DPI/keyboard, quote ảnh và reaction ACK thật chưa thử. |

Chi tiết evidence/budget/privacy: `../re-notes/44-map15-network-send.md`.

## Phase 4

| Checklist | Triển khai / kiểm chứng source-fixture | Nghiệm thu / giới hạn còn lại |
| --- | --- | --- |
| MAP-13 rich composer/picker | Ctrl+Shift+X bật/tắt metadata thật; B/I/U/strike cả tin. Sticker/Emoji/GIF chung picker neo, insert/send thật. | Selection-range formatting không được tuyên bố hỗ trợ. Native anchor/keyboard/GIF chưa thử. |
| MAP-14 settings10 trang | Matrix tại MAP14_SETTINGS_COVERAGE.md: persistence/consumer, server privacy, async mic/speaker/camera enumeration, persisted selectors consumed by media, incoming-call notification toggle, autostart worker, wallpaper avatar cache. PC 26.11.12 E2 ghi nhận preload mặc định off, file cap 30 MiB, video preload off/cap 100 MiB và gate theo auto-download từng account. Fork chưa có predownload consumer/size authoritative; không thêm toggle giả. | Phonebook không-Zalo không có contract: UI chỉ nói bạn bè/người đã liên hệ. Priority là local pinned split. Hardware/registry/Hello/notification/privacy live chưa thử. Fork đã có Ctrl+Alt+S capture và camera enable/disable trong active call; chưa xác nhận persistent inline-video mute/capture scope hoặc native operation. |
| MAP-16 pins | Danh sách pin server riêng, loading/error/retry, dedup, count authoritative, account/thread/generation. Không áp reminder pagination. Jump tải DB context quanh tin cũ. | Pin list/ACK thật chưa thử. |
| MAP-17 reminder/important/urgent | Reminder mở form board thật giữ draft; urgency -1/1/2 và textProperties styles/ver theo PC 26.11.12 E2; mentions/quote đi cùng metadata và identity. PC builder tách styles trong `textProperties` khỏi urgency trong `metaData`. | Định dạng/urgency trên media/voice/sticker/schedule chưa có contract nên chặn rõ, giữ draft; không gửi rơi metadata. Native/live chưa thử. |
| MAP-18 labels | Header dưới tên/avatar mở panel nhãn dùng chung nguồn sidebar, worker và refresh kết quả. | Live labels chưa thử. |
| MAP-19 edit name/avatar | Alias cá nhân khác tên nhóm; avatar nhóm Inspect quyền→upload→mutation; không đổi avatar người khác hoặc tự rename cloud. Modal pending không đóng mất state. | Quyền/ACK profile/group thật chưa thử. |
| MAP-20 links | Title/URL thật trong overview/archive; thumbnail qua worker/cache40px, fallback, không I/O UI; inline/context copy cùng guard bảo vệ. | Native layout/image tải thật chưa thử. Sau audit bổ sung, date filter có preset 1 tuần/1 tháng/3 tháng và calendar E2; Photos đã nhóm theo ngày. |
| MAP-21 My Documents | Nhận diện chính xác own_cloud_id; drawer riêng, viewer-key auth/encrypted usage/quota thật, thiếu quota không giả0. Review cleanup→archive selection→confirm delete. | Cloud auth/usage/cleanup trên account thật chưa thử. |
| MAP-22 group/community | Metadata numeric type1/2 có authority; unknown trung tính, explicit unknown xóa cached kind; không đoán qua tên/member count; role quyền kiểm tra độc lập. | Native/live entitlement cộng đồng chưa thử. |
| MAP-23 call history | Không purge/filter call; parse recommended.calling/receivecall duration; local call scoped owner/runtime/direction và ID; local deletion không gọi server, call không forward. | Call native/WS history, unread/dedup thật chưa chứng nhận; source không tính call logs vào unread observe. |
| MAP-24 sidebar time | Formatter chung phút/giờ/ngày, đúng7ngày rồi date cùng/khác năm, local timezone; repaint định kỳ 30s khi account mở khóa để nhãn đổi dù UI idle. MAP-26/29 cũng chuẩn hóa giây/mili-giây trước lọc/chọn latest. Regression ranh giới/time và mixed units. | PC rounding/native DPI chưa đo lại. |
| MAP-25 file icons | Resolver chung PDF/Word/PowerPoint/Excel/CAD/other; vector màu bubble/drawer/filter, filename→metadata MIME/name→URL fallback, category regressions. | CAD là extension do người dùng yêu cầu; native DPI/theme chưa thử. |
| MAP-26 archive filters | Sender ID picker có avatar/search/tên trùng; local date bounds/type/query intersect; independent SQLite keyset pages200, owner/runtime/thread/generation, load older khi trang không match; timestamp seconds/milliseconds normalized at storage migration/persistence and UI sort; count ghi local loaded. | Phạm vi là lịch sử LOCAL đã lưu, không chứng nhận toàn bộ server archive. PC preset/calendar đã đối chiếu E2 và triển khai; server archive vẫn không được chứng nhận đầy đủ. |
| MAP-27 archive actions | Message+attachment identity, parent dedup; copy ảnh original qua worker, copy URL bằng clipboard text, reveal verified local path; serial downloads folder chooser nonoverwrite/progress/cancel/per-item; common two-step share/MyDocs; separate delete/recall snapshot confirmation và per-item errors. | Live destructive KHÔNG thử. Media expired/renew/native clipboard/download cần nghiệm thu thiết bị. |
| MAP-28 bubble menu | Ordered icon actions/submenu, capability/state/private/expiry guards, details re-resolve current identity; recall cập nhật tombstone. Timeline multi-select dùng common preview, scope account/runtime/thread/group, reset, modal gate; bỏ BatchForward cũ. | Ordinary link re-send giữ card là fork deviation (re-notes47), opaque content không tự suy contract. Native menu/edge/scroll/keyboard chưa chạy. |
| MAP-29 sidebar menu | Target thread ID ổn định khác open thread; nhãn/pin/unread/mute/hidden/delete giữ scope; report form và invite picker→Inspect→final confirm backend thật. Không gửi khi chỉ mở menu. | Conditions/menu mẫu chưa thấy trên PC và mutations thật chưa nghiệm thu. |

Nguồn E2 và deviations: `../re-notes/45-map19-21-29.md`, `../re-notes/45-phase4-group-kind.md`, `../re-notes/46-outgoing-composer-metadata.md`, `../re-notes/47-map26-map27-archive.md`. Không commit bundle PC độc quyền hoặc dữ liệu account.

## Finding đã đóng qua audit độc lập và phản biện

- MAP15 chọn host ở builder né resolver, is_connect quá rộng, taxonomy envelope/nested, enqueue identity, queue-inclusive deadline, canceled BodyAck không ghi timing, fixture tự gắn timing thay production path.
- Pins trộn local/server count và reminder pagination; group kind stale/heuristic; cloud thiếu quota giả0/credential scope; call direction/local IDs/server action; details snapshot lộ sau relock/recall.
- Composer metadata rơi ở file/voice/schedule/sticker và async voice finish; Ctrl+Shift+X không tắt metadata thật; picker Cancel mất draft. Mở picker hiện giữ draft cả khi hủy/queue fail; không giả có acceptance để xóa draft.
- Archive timeline-window thay pagination, attachment identity/parent scope, copy thumbnail thay original, batch download collision/error, confirmation bị giấu trong drawer, stale result/account/thread.
- Timeline legacy multi-forward bấm recipient gửi ngay; selected IDs còn sau đổi thread/account; independent timeline/narrow drawer Areas xuyên modal. Tất cả đã sửa và thêm regression.
- Link renderer không dùng thumbnail, inline copy bỏ guard; genuine link forward bị disable. Đã thêm async cache render/guard và ordinary-card forward bảo toàn caption/href/title/desc/thumb/src, direct/group encrypted request fixture.

A/B kết luận source không còn finding actionable xác nhận sau các sửa trên; đây không phải bảo đảm không có bug và không thay native/live acceptance.

## Kiểm chứng cuối

- Vòng full test đầu: 943 passed, 1 failed, 10 ignored ở lib. Failure là shared test-ring interference: stress fixture ghi301 mẫu vào ring256, làm mất Attempt của body-stall đang chạy đồng thời; BodyAck đã có nên production Attempt Success đã chạy.
- Advisor/auditor B xác nhận nguyên nhân; sửa bằng mutex CHỈ TRONG TESTS dùng chung cho stress/ring-inspection tests, giữ mọi assertion và actual Session path. Focused timing suite:5 passed,0 failed. Full suite đang chạy lại sau khi mở rộng isolation cho hai outgoing timing tests.
- audit-prepush vòng source cuối trước sửa test-only: hard gates passed (cargo check và Clippy). Một heuristic UI-stub hit đã đọc: đó là guard chặn metadata chưa hỗ trợ và giữ draft; nhánh else gọi picker thật, không phải nút giả. Không release tag nên tag gate skipped.
- Full suite cuối sau sửa test-only: lib 944 passed/0 failed/10 ignored; tổng tất cả test targets 0 failed (28+7+1+9+15+1+3+19+3+1+1 qua các suite). cargo fmt --check exit 0.
- audit-prepush vòng cuối: hard gates passed (cargo check, Clippy -D warnings, version/tag gates). Kết quả ghi từ exit code thực tế của snapshot này.

## Trạng thái nghiệm thu tổng thể

Các MAP đã triển khai trong hai phase được audit A/B đối chiếu; riêng checklist MAP-14 vẫn còn các gap source được liệt kê ở trên và trong coverage matrix. Toàn bộ checklist nghiệm thu KHÔNG được đánh dấu hoàn tất: native/live, archive server completeness, UI parity phần chưa quan sát và các fork deviation vẫn ghi rõ phía trên. Build/test binary native đang hoãn theo lệnh người dùng “chưa build”; không tự gửi/xóa nội dung account thật. Phase5/audit toàn diện Phase1–5 không được gộp nhầm thành audit này.
