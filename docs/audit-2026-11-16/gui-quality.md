# Audit độc lập GUI, hiệu năng và code thừa

Ngày: 2026-11-16. Snapshot: `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`, `main`.
Phương pháp: đọc tĩnh source và đối chiếu AGENTS.md, chuẩn IPC/concurrency/testing,
GUI handover, ZALO_REFERENCE_ANALYSIS. Không chạy ứng dụng, không live Zalo,
không thay đổi source. Các đường lỗi dưới đây là chứng minh theo luồng source,
không phải nghiệm thu hành vi thiết bị. Kết quả kiểm tra Cargo do agent chính tổng hợp.

## GUI-01 — P1: Bản ghi âm và micro vượt ranh giới tài khoản

- Nguồn: `src/gui.rs:1020`–1022; `src/gui.rs:1847`–1894;
  `src/gui.rs:2921`–3053; `src/gui/workspace.rs:6992`–7047.
- `voice_recorder`, `voice_preview`, `recorded_voice_samples` là state toàn app,
  không mang owner/thread. `LoggedOut` và `ProfileSwitched` gọi clear/reset nhiều
  state nhưng không cancel/reset ba trường này. Tìm kiếm mọi chỗ tham chiếu đã
  loại trừ khả năng helper khác dọn gián tiếp.
- Kịch bản: tài khoản A ghi âm hoặc đã dừng thu nhưng chưa gửi; chuyển sang B,
  mở hội thoại và gửi. `send_voice_recording` lấy buffer cũ, sử dụng to_id/group
  hiện tại và worker hiện tại. Micro đang thu cũng tiếp tục tồn tại qua logout.
  Việc đổi hội thoại trong cùng tài khoản cũng đổi nơi gửi mà không có ràng buộc
  với nơi bắt đầu ghi âm.
- Tác động: nội dung âm thanh của A có thể bị gửi bởi B; micro không dừng ở
  ranh giới logout. Đường chuyển tài khoản có thực: account_switcher được gọi
  tại workspace.rs:253, không phải thành phần chết.
- Đề xuất: owner + thread capture context cho voice draft; cancel recorder và
  preview, xóa buffer tại account/session boundary; kiểm tra lại context khi gửi.
  Kiểm tra khi khóa ứng dụng cũng cần chính sách rõ ràng.
- Regression cần có: tạo state buffer giả, xử lý account switch/logout, xác nhận
  state đã rỗng và không phát SendVoiceMessage với buffer cũ. Kiểm tra native
  riêng cho việc dừng thiết bị micro. Không cần suy đoán hợp đồng Zalo.

## GUI-02 — P2: Nút reaction nói bỏ nhưng vẫn gửi thêm

- Nguồn: `src/gui/workspace.rs:3050`–3076; `src/chat.rs:4263`–4273;
  `src/gui/worker.rs:2918`–2958; `src/gui.rs:3799`–3837.
- Khi `real.reactions` đã chứa r, UI clear vector và hiện “Đã bỏ cảm xúc”.
  Nhưng WorkerCmd::React luôn nhận `r.to_string()` không rỗng. Trong mã request,
  chỉ chuỗi rỗng hoặc `/-clear` mới tạo rType=-1. ACK lại áp dụng reaction r.
- Kịch bản: thả một reaction, sau ACK bấm lại cùng reaction. Lệnh phát ra là add,
  trái với hành vi remove mà UI công bố. Nếu request lỗi, vector optimistic đã
  persist cũng không được rollback; error chỉ là status.
- Phản chứng: nút riêng “Bỏ cảm xúc của tôi” trong reaction modal có gửi chuỗi
  rỗng đúng. Đây là bug ở đường BubbleAction, không phải toàn bộ API clear.
- Đề xuất: quyết định add/clear dựa own-reaction state, gửi đúng action; chỉ
  cập nhật canonical summary khi xác nhận hoặc có rollback có correlation.
  Kiểm tra command dispatch không cần live API. Semantics enum có dẫn bundle
  E2 trong chat.rs; không coi phân tích tĩnh là E1.

## GUI-03 — P2: Luồng thu âm xử lý nặng trên UI và copy toàn buffer mỗi frame

- Nguồn: `src/gui/workspace.rs:3754`, 3787, 6992–7040;
  `src/voice.rs:91`–205, 233–281.
- Mỗi frame khi audio draft tồn tại đều clone toàn `Vec<f32>` chỉ để chuyển
  cho hàm render, dù người dùng không bấm preview. Khi 48 kHz mono, 10 phút
  tương đương 115,2 MB/buffer; lượng copy mỗi giây = số frame repaint × dung
  lượng buffer. Đây là tính toán dung lượng, chưa phải benchmark FPS.
- Micro setup, thiết bị playback, finish (clone), resampling tuyến tính và
  encoding WAV chạy trực tiếp trong UI callback. Recorder không giới hạn độ
  dài; samples.push giữ tăng bộ nhớ. Vi phạm INV-3 về audio setup/blocking work.
- Đề xuất: render chỉ nhận duration/waveform và action enum; snapshot buffer
  đúng lúc preview/gửi, giữ Arc immutable hoặc chuyển ownership; device setup,
  resample/WAV qua worker/spawn_blocking; giới hạn thời lượng/dung lượng với
  thông báo rõ. Đo allocation/frame và độ trễ input theo thời lượng audio.

## GUI-04 — P2: Drawer công bố E2EE vượt bằng chứng và trạng thái thực

- Nguồn: `src/gui/workspace.rs:6633`–6675, đặc biệt 6669–6672;
  `src/gui/worker.rs:1357`; AGENTS.md và E2EE trial gate.
- Phần “Mã hóa đầu cuối” luôn hiện “Tin nhắn được bảo vệ theo giao thức mã hóa
  Zalo”, hoặc “Cuộc trò chuyện được bảo vệ toàn diện” chỉ dựa trusted_peers.
  Không kiểm tra runtime/build trial, phiên đang mã hóa, loại nhóm, hay phạm
  vi media. Người dùng mặc định cũng nhìn thấy assurance này.
- Tác động: UI có thể khiến người dùng hiểu sai mức bảo vệ E2EE thực tế.
  Xác minh safety number không chứng minh mọi tin/media của hội thoại được mã hóa.
- Đề xuất: state hiển thị từ capability + policy + active crypto session; ghi rõ
  thử nghiệm/chưa xác minh/chưa bật và phạm vi. Unknown — AMBIGUOUS về parity
  server; audit này chỉ khẳng định assurance đang vô điều kiện trong source.

## GUI-05 — P3: I/O đồng bộ ở nút mở diagnostics

- Nguồn: `src/gui/tools_panel.rs:1224`–1227; `src/platform.rs:49`–59.
- Callback trực tiếp create_dir_all rồi open_folder (is_dir + ShellExecuteW),
  bỏ cả lỗi tạo thư mục và kết quả mở. Có thể chặn frame khi storage/shell chậm,
  lỗi không có terminal event hay thông báo.
- Đề xuất: dùng worker hoặc cùng mẫu thread + GuiEvent như phần mở thư mục
  download trong modals.rs:1517–1535. Không tuyên bố đã tái hiện treo native.

## Tối ưu và ứng viên dọn code

- `workspace.rs:2601`–2621 quét toàn bộ message vector và cấp phát visible_indices
  rồi copy tail mỗi repaint mặc dù render_limit thường chỉ 150. Có thể duyệt
  ngược lấy đủ render_limit rồi đảo, hoặc cache chỉ số theo revision. Chi phí
  hiện tại O(N) mỗi frame, tối ưu trường hợp thông thường O(K) với K là phần
  cần hiển thị; khi filter có thể vẫn phải O(N). Cần benchmark N lớn và giữ
  đúng call-log/star/jump/album behavior. Đây là đề xuất, không phải lỗi đo được.
- Xóa candidate nhỏ: method wrapper `ZaloGuiApp::sender_avatar_for` tại
  workspace.rs:5052; không có call site trong src. Free function cùng tên tại
  5039 được gọi thật tại 2720/2723 và phải giữ. Xác minh mọi cfg/test trước xóa.
- Có thể bỏ các suppression dead_code đã thừa ở gui.rs:642, 870, 1044 vì các
  field có real callers; không xóa các field đó. Không suy từ warning suppression
  ra kết luận code rác.
- VoiceRecorder F32/I16/U16 có callback gần trùng nhau; chỉ gom nhỏ nếu benchmark
  và test giữ mono conversion, waveform cadence, lỗi device. Ưu tiên đúng đắn
  và ownership hơn refactor thẩm mỹ.

## Những nghi vấn đã loại bỏ

- “Tải thêm tin cũ” không phải nút giả: dispatch LoadGroupHistory tại
  workspace.rs:2580 trước khi tăng local render limit.
- Mở download folder đã nằm trong thread ở modals.rs:1517; không báo nhầm là
  UI I/O. Clipboard PNG helper cũng phải lần theo caller, không kết luận từ
  riêng hàm có std::fs.
- `server_pins` được reset tại reset_sync_status(workspace.rs:44), nên không
  báo leak account từ việc clear_presence_state không nhắc đến field.
- AccountSwitcher và video_state có real callers; không đề xuất xóa.

## Phản biện chéo đã thực hiện

- Chấp nhận lỗi TUI delete không tombstone: DeleteDone tui.rs:1478–1485 chỉ bỏ
  vector rồi upsert; store.rs:1594–1648 không xóa; new hydrate tui.rs:206–208.
- Chấp nhận lỗi TUI send bỏ ACK (.map(|_|())) và SendFailed chỉ status: persisted
  optimistic row không được chuyển trạng thái thất bại theo message identity.
- Chấp nhận socket unbounded selected-branch send: run_once được await trực tiếp
  tại socket.rs:654; cancellation/liveness chỉ được poll ở select vòng kế tiếp,
  không bảo vệ ws.send đang Pending tại 857/896/974/1017.
- Chấp nhận crypto bundle signing/DH binding flaw P2 trong phạm vi experimental:
  verify_signed_prekey dùng signingIdentityKey tự cung cấp, còn start_session
  trust/derive identity_key khác; không thấy proof liên kết. Không coi đây là
  chứng minh khai thác Zalo thật.
- Chấp nhận TUI hidden-chat display gate gap theo ADR-0034:13–24; đây là gate
  riêng tư trong ứng dụng, không phải mã hóa bảo vệ dữ liệu trên filesystem.
- Chấp nhận migration13 P2: store.rs:228–247 nuốt lỗi ALTER/user_version và
  không transaction, có thể báo mở store thành công với schema không đúng.
- Chấp nhận queued TUI refresh/poll sau logout: tui.rs:988–995 chỉ clear rồi
  logout; handler 1393/1406 không owner/login guard, có thể hiện lại dữ liệu.
- Chấp nhận host fallback runtime chat.rs:3296 là vi phạm endpoints invariant P3.
- MediaNeighbor không bỏ payload: gui.rs:2285–2310 kiểm tra owner/anchor và mở
  message trả về. TODO album counter modals.rs:2836 chỉ là counter chưa có,
  không phải nút navigation giả; loại khỏi danh sách lỗi chức năng.

Các kết luận cần được agent chính đưa vào ma trận đồng thuận cuối; không có
thay đổi code nào được thực hiện trong audit này.
