# Phản biện độc lập B — tái kiểm `f2b5e97`

**Snapshot:** `f2b5e9736c26dd81f4ea9342eaabf7594dac124c`, `main` sạch khi bắt đầu. **So sánh:** `e75e372..HEAD`, hai commit sửa `d3e0630`, `4496952` và tài liệu `f2b5e97`. **Ngày:** 17/11/2026. Chỉ đọc production source; chỉ thêm báo cáo này. Không chạy Cargo song song với root, không sửa code/index/refs, không dùng tài khoản thật.

Đã đọc diff và source hiện tại tại từng boundary B01–B06; đọc `CONSENSUS_AUDIT_PHASE1_4.md` như tuyên bố cần kiểm chứng. Bản “100% unanimous approval” không thay kết quả kiểm tra độc lập dưới đây.

## Kết luận

**CHANGES REQUIRED.** Bản vá xử lý đúng lỗi kích thước Ami, giữ permit trên đường tải ảnh chính và chuyển decode CPU ra blocking pool. Tuy nhiên cache chưa có phân biệt thumbnail/sprite, I/O ghi cache mới lại chặn async runtime, và sửa logout TUI tạo lỗi báo thành công dù xóa credential chưa xong/thất bại. Không có căn cứ gọi toàn bộ phạm vi đã hoàn tất.

## Trạng thái B01–B06 sau sửa

| ID cũ | Kết quả hiện tại | Bằng chứng và giới hạn |
|---|---|---|
| B01 — atlas quá rộng | Đã sửa root cause cho 15 asset và renderer ARM64 mặc định, ở mức source/test thiết kế | `bundled_stickers.rs:158-190` resize từng frame về160px, từ chối sheet>8192; tất cả asset hiện có tối đa3200px. Test all15 có assert8192. `gui.rs:2135` thêm guard. Không tuyên bố đã trực tiếp chạy upload GPU ARM64 của HEAD mới. |
| B02 — permit bị bỏ trước UI | Đã sửa đường production chính | `worker.rs:1139-1143` chuyển `Some(job)` vào event; `gui.rs:4328` drop sau cài texture. Xem phân tích `None` bên dưới: không có bypass reachable qua WorkerSender hiện tại. Preload cache riêng vẫn không giữ permit; không dùng điều đó để phủ nhận bản sửa đúng của đường chính. |
| B03 — cache bị bỏ qua | Một phần; còn lỗi chức năng rõ ở R-B1 | Có read/write cache mới, nhưng dùng chung namespace static/sprite và preloader cũ, không upgrade metadata. Private-media cache cũ chưa được migrate/đọc lại; không có budget/prune cho namespace mới. |
| B04 — decode CPU trên executor | Decode CPU đã sửa; phát sinh I/O đồng bộ R-B2 | Các GIF/PNG/network decode đã vào `spawn_blocking` (`worker.rs:769`,`:777`,`:795`,`:806`,`:821`,`:832`,`:848`,`:855`). `atomic_write` mới vẫn chạy ngay trong async function. Không có số đo heartbeat/latency mới của B. |
| B05 — logout TUI đồng bộ | Đã chuyển công việc filesystem sang thread, nhưng tạo regression R-B3 | Detached thread không có completion, lỗi chỉ vào logger và không join khi thoát; UI luôn công bố logout xong. |
| B06 — nút Gửi bị clip khi sync | Đã sửa trigger đã biết; root đối chiếu CI ARM64 HEAD thành công | `workspace.rs:87-89` dùng status34px khi viewport<300logicalpx thay92px. Root xác minh CI ARM64 run43942850530 tạiHEAD mới thành công, gồm native smoke/package. Các quote/picker/attachment combinations cần phân biệt với trigger480x360@1.5 ban đầu; không suy pass mọi tổ hợp. |

## R-B1 — P2 — Cache sprite và thumbnail dùng chung đường nhưng preloader không khôi phục metadata

**Anchors:** `src/gui/worker.rs:652-674`,`:784-797`,`:828-835`; `src/gui.rs:4301-4305`; `src/gui/messages.rs:49-51`,`:105-107`; `src/gui/stickers.rs:1579-1581`.

Bản mới ghi nguyên sprite strip vào `media-cache/stk_<cid>_<sid>.png`, rồi dùng đúng tên này cho cache ưu tiên trước network. Nhưng search/suggestion preloader vẫn decode file thành `StickerImageReady { key, image }`, không có frame/delay. Sau restart, state `sticker_sprites` rỗng; event này cài texture chứa toàn dải vào `sticker_textures`. Widget thấy texture đã tồn tại nên không gửi FetchStickerImage. Fallback renderer vẽ toàn strip như một ảnh vuông; picker cũng không có UV metadata để tách frame. Đây là lỗi mới có trigger bình thường: tải sprite -> restart -> search/suggestion trả cùng sticker -> preload cache -> sticker dẹt/tĩnh. Không cần lỗi server, dữ liệu độc hại hoặc tài khoản khác.

Chiều ngược lại cũng chưa sửa: PNG thumbnail vuông cũ được `fetch_and_decode_sticker_sprite` đọc trước, giải thành1frame và return ở`:797`, nên không có request nâng lên sprite. Cùng namespace không có version/type/metadata để phân biệt hai loại. Cache private-media đã tồn tại từ phiên bản trước vẫn không được đọc trên đường mới. Ghi mới cũng bỏ qua `PrivateMediaCache::prune`/DISK_BUDGET nên chưa có giới hạn tổng dung lượng namespace này.

**Khắc phục:** cache có version/loại asset hoặc manifest metadata; preloader phải tái tạo SpriteReady cùng frames/delays (và resource permit) hoặc chỉ dùng static preview rồi chủ động nâng cấp. Không dùng điều kiện `texture exists` như đồng nghĩa animation đã sẵn sàng. Áp budget/prune ở cùng đường ghi. Kiểm warm/cold restart, static legacy upgrade, offline, eviction, late static completion và tổng dung lượng.

**Mức kiểm chứng:** đọc source/diff liên kết đầy đủ; B chưa chạy GUI/account/network hoặc harness mới cho private worker function. Vì vậy không gọi đây là native screenshot đã tái hiện.

## R-B2 — P2 — Ghi cache mới vẫn fsync trực tiếp trên runtime hai luồng

**Anchors:** `src/gui/worker.rs:809`,`:823`,`:834`,`:849`; `src/storage.rs:288-292`,`:317`; runtime `src/gui/worker.rs:1082-1084` (hai worker threads).

Decode đã offload đúng, nhưng sau khi await decode, `fetch_and_decode_sticker_sprite` gọi thẳng `storage::atomic_write`. Hàm này tạo file, `write_all`, `sync_all` và `MoveFileExW(...WRITE_THROUGH)`. Đây vẫn là I/O blocking trên executor, có thể xảy ra đồng thời khi nhiều sticker tải xong. Trên đĩa chậm/bận, command/socket/cancellation phải chờ hai executor threads; bản vá chưa đáp ứng chuẩn không blocking trên async runtime. Không có bằng chứng đo được45giây timeout, CPU spike hay mất mạng thực tế trong lần kiểm này.

**Khắc phục:** chuyển write/prune vào blocking closure hoặc store/cache actor; giữ permit/budget/cancellation xuyên suốt, cho phép hiển thị dù cache write lỗi nhưng phải xử lý lỗi có chủ đích. Kiểm delayed/faulty storage và latency socket/command, thay vì chỉ assert decode có spawn_blocking.

## R-B3 — P2 — Logout TUI báo xong trước khi credential được xóa, bỏ lỗi và không đợi thread khi thoát

**Anchors:** `src/tui.rs:1884-1905`; `src/profiles.rs:141-160`,`:167-173`; `src/tui.rs:895-915`,`:2674-2678` (shutdown không quản lý cleanup thread).

Detached `std::thread::spawn` bắt đầu xóa phiên, nhưng UI lập tức gọi Session::logout, xóa danh sách và hiện “Đã đăng xuất…qđể thoát”. Nếu `forget_session` thất bại lúc remove credential/đọc-ghi registry, thread chỉ log lỗi, UI không nhận kết quả và credential còn có thể được khôi phục lần mở sau. Nếu người dùng thoát ngay, không có handle/join/critical tracking bảo đảm thread hoàn thành trước process exit. Không cần suy đoán server revocation để chứng minh vấn đề: bất nhất giữa lời báo của UI và trạng thái credential cục bộ là trực tiếp từ code.

Đây là regression của việc chuyển sync sang background: bản trước trả lỗi và không báo thành công khi cleanup thất bại. Sửa responsiveness không nên đánh đổi kết quả logout bền vững. Chưa thực thi fault injection bằng credential thật; tuyệt đối không dùng dữ liệu người dùng để tái hiện.

**Khắc phục:** typed runtime-stamped completion, state logout pending và bounded shutdown/join. Có thể khóa/clear nội dung ngay để bảo vệ màn hình, nhưng không công bố đã xóa credential khi chưa thành công; cung cấp retry/error rõ. Kiểm lỗi xóa/registry, thao tácquitngay, delayedcleanup, login/switch sau đó và owner cũ/mới.

## Phản biện `permit: None`

`worker.rs:3447-3456` tạo SpriteReady không permit trong nhánh command trực tiếp. Tuy nhiên `WorkerSender::send` intercepts FetchStickerImage tại`:529` và chuyển sang image channel; command sender không public ra ngoài mailbox. Đường image channel tại`:1139` giữ Some(job). Không tìm thấy caller production gửi FetchStickerImage trực tiếp vào raw command sender. Vì thế nhánh None là code dư/rủi ro thiết kế, **không phải bypass reachable đã được chứng minh**. Nên bỏ nhánh dư hoặc làm type không cho None; không tăng severity chỉ vì Option hiện diện.

Preloader `StickerImageReady` vẫn có thể phát tới24ảnh cache trong batch không resource permit; sprite file mới lớn hơn thumbnail cũ. Đây là giới hạn cần sửa khi thiết kế cache R-B1, không được tuyên bố mọi completion đều bounded chỉ từ Some(job) của đường chính.

## Cross-review với reviewer A

B xác minh độc lập và đồng ý:

- **A1 recursion:** depth helper từ chối nestedPreKey trước bước recursion tiếp; root cause source đã sửa. Test mới gọi trực tiếp internal helper với depth giả, chưa thay integration test đi qua public deserialize và receive boundary. Không tạo hoặc chạy payload gây crash.
- **A2/M-05:** ảnh mới vào accounttemp, replace/cancel draft hiện tại được unlink; stale ClipboardImageRead khác owner/thread và worker inactive vẫn có đường bỏ payload mà không cleanup file; không startupprune production, voice/sentfile switch chưa đủ. Không coi test tự tạo file dưới accounttemp là test của Windows clipboard producer.
- **A3/M-01:** đã có completion typed và lỗi DB, nhưng reset vẫn chỉ xóa flag trong khi `requires_e2ee` còn OR ratchet session; promised ordinary-send recovery chưa đúng với peer có ratchet. Handler bỏ owner ở`gui.rs:4331`; cần kiểm runtime/eventchannel reachability trước khi khẳng định cross-account exploit.
- **M-06:** history_sync và QR save đồng bộ khác chưa đổi; riêng PersistSession offload không đóng toàn bộ nhóm.

Reviewer A đã độc lập xác nhận R-B1 (full-strip cache bị preload như static), R-B2 (fsync trên async), R-B3 (logout detached báo thành công dù cleanup lỗi) và đồng ý không tính OptionNone là bypass reachable. Reviewer B đã đọc báo cáo A tại `PRO_A_REVIEW_F2B5E97.md` và đồng ý ma trận A1 đóng root cause; A2/A3 một phần; M-06 còn mở. Cả hai thống nhất CHANGES REQUIRED cho snapshot này, không sao chép lời approve của tài liệu phase1–4.

## Giới hạn và bằng chứng kiểm tra mới

B chỉ chạy read-only Git/source/diff trong tái kiểm; không chạy lại probe cũ vì B01 source đã thay đổi, không tái sử dụng panic ởe75e372 làm kết quả HEAD mới. Root sở hữu các gate Cargo/default/features/clippy/smoke hiện tại; kết quả mới phải nằm trong VERIFICATION/consensus hiện hành. Không kết luận native ARM64, micro, WindowsHello, peakRSS/battery hoặc Zalo protocol parity từ unit tests. Timing80ms và interoperability vẫn cần nguồn chính thức/capture riêng.

## Phê duyệt bản báo cáo hợp nhất

Đã đọc toàn bộ `FINAL_CONSENSUS_AUDIT_2026-11-17.md` với snapshot cuối `f2b5e9736c26dd81f4ea9342eaabf7594dac124c`. B đồng ý ma trận12mục đạt cục bộ/9mục một phần, đóng A1/B01/B02/B04CPU/B06 đúng phạm vi, giữ N01–N05 ởP2 và giới hạn static/native/parity. Đồng ý ưu tiên và kết luận CHANGES REQUIRED; đây là chấp thuận tính chính xác của báo cáo, không approve sản phẩm. Đề nghị nhỏ không chặn: N02 ghi rõ cache disk budget/prune và dẫn các dòng preload652–674/cache834 để thuận sửa.
