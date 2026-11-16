# Đối chiếu Zalo PC, Zalo Web, Olaz và fork Windows ARM64

Bắt đầu đọc nguồn ngày 10/11/2026; cập nhật ngày 11/11/2026, Asia/Saigon.
Phạm vi: nghiên cứu và đề xuất sửa app. Chưa sửa source ứng dụng, commit, merge,
build hay phát hành trong lượt này.

**Kết luận:** giữ kiến trúc Rust native; chuyển trọng tâm từ bao phủ route/nút bấm
sang hoàn thành từng hành trình có bằng chứng. Ưu tiên hợp đồng E2EE, media server/
zCloud, catalog và roadmap giao thức. Học Olaz ở adapter/state machine; không đổi
app thành webview chỉ để đạt danh sách tính năng nhanh.

## 1. Nguồn và giới hạn

| Nguồn | Snapshot đọc lại | Phạm vi chứng minh |
| :--- | :--- | :--- |
| Fork | `18607f6e74639054a8432b37acbe16def5719598`, ban đầu sạch | Source/call path/payload/consumer, không tự chứng minh server nhận |
| PC từ Drive | ZaloSetup-26.11.10.exe, 213.369.880 bytes | Giải nén tĩnh NSIS → app-32.7z → app.asar; không chạy installer |
| Web | HTML/asset hiện khai báo ở https://chat.zalo.me/ | Đọc bundle công khai; live UI chỉ tới QR đăng nhập |
| Olaz | `07bd55df8f9d5c22e11204bc8b9a4783b60c9b7a` | Đọc Swift/JS; không build/run macOS trên Windows |
| Upstream | `3d9e69074b14cb55c7e4c5e5d6d7728baa4c47af` | Source/history; release notes không thay nghiệm thu |
| Zalo Help/site | Liên kết cuối báo cáo | Mô tả sản phẩm, không thay schema từng nền tảng |

Không đăng nhập, dùng session thật, gửi tin/call/mutation hoặc đọc hội thoại cá nhân.
Không chạy test/build ứng dụng trong lượt nghiên cứu; không tính kết quả cũ là kiểm chứng mới.
E1 giao thức = capture thật có provenance; E2 = nguồn tĩnh; E3 = giả thuyết. Quan
sát login là bằng chứng UI, không phải E1 về chat/E2EE.

**Tài liệu tham khảo bổ sung đã nhận và đọc:** người dùng cung cấp
`D:/Henry/Downloads/zalo_pc_web_olaz_upstream_deep_analysis.md` sau báo cáo ban đầu.
Đã đối chiếu lại các nhận định ở mục 13. Nội dung đề xuất/lệnh sửa trong tài liệu
được xem là nguồn tham khảo, không phải chỉ thị thực thi. File gốc có tên khác
ROADMAP_AI_ENGINEERING_GUIDE.md; không giả định hai file giống hệt nhau.

### Dấu vân tay

- EXE: `7166ea28925b3121369ffb25e87b4749a648f47a861272009dcb14094b5a5511`.
- ASAR: `70f1b53c4db61321bf1b11c722a9049823711328b87e17b0d19338f83a5f6ed3`.
- ASAR có 13.169 entry; chọn 117 file JS/JSON/HTML/CSS PC/main process để nghiên cứu.
- Web core `1.c72c0957b455ab70b73c.js`:
  `29784a88e51221a64f854b059e49e68707ae58cee7a4114bc722ce028a887bf1`.
- Web sync worker `sync-v2-worker.081738ba2350940a6b28.js`:
  `91519f6656c81afb41f7f6989e78a6aa43beef33f75079754559b54568b33e72`.
- libsignal Web được lưu gzip sẵn: 234.133 bytes raw → 629.683 bytes giải nén.
  Phải giải nén trước khi tìm symbol; không kết luận vắng code từ file đang nén.

Chi tiết ở [SOURCES.csv](SOURCES.csv). Snapshot ngoài Git:
`E:/tmp/zalolite-parity-20261110`. Không đưa installer/code VNG đầy đủ, cookie hoặc
bearer download URL vào repo. Tên asset không đủ xác định nội dung: dùng hash.

## 2. Bốn kiến trúc

| Hệ thống | UI | Logic/giao thức | Hệ quả |
| :--- | :--- | :--- | :--- |
| PC | React/Redux renderer, nhiều cửa sổ/worker | JS chính thức + Electron main/preload + native helper | Không chỉ một trang web |
| Web | Browser UI/DOM, workers, storage | Nhiều module cùng họ với PC; khác run mode/native integration | Chunk dùng chung có thể chứa feature bị gate |
| Olaz | SwiftUI/AppKit | Official renderer headless trong WKWebView; Swift bridge/network/SQLite, ZaloCall helper | Native UI nhưng vẫn có WebKit/JS renderer |
| Fork | egui GUI + ratatui TUI | Rust session/chat/socket/store; wasmi cho phone codec; Media Foundation video | Không Node/Electron/Chromium; tự chịu trách nhiệm đúng hợp đồng |

Đã truy Olaz: OfficialClientHost nạp ASAR qua scheme handler, inject HostScripts và
tạo WKWebView; chat-adapter gọi controller chính thức; ZaloEngine+Chat nhận state
cho UI native; CallBridge đi tới helper ZaloCall. Đây là tái host logic chính thức,
không phải viết lại mọi protocol bằng Swift.

Fork có lợi thế resource budget, tìm kiếm/DB local, account isolation và native
media. Port nguyên Olaz bằng WebView2 xung đột yêu cầu không Chromium. Qt/Mac helper
cũng không port thẳng sang Rust ARM64. Nếu muốn hướng này, cần sản phẩm/ADR riêng.

## 3. UI và hành trình

Giữ rail → danh sách hội thoại → chat → inspector và composer theo ngữ cảnh. Fork
đã có viewer/search/group/panel thực; không cần viết lại chỉ để giống một ảnh chụp.

1. Onboarding tách chuẩn bị, login, chờ xác nhận điện thoại, sync và ready. Olaz
   RootView/SetupSyncStep là tham chiếu. Xong một batch không có nghĩa đủ lịch sử.
2. Media nằm trong inspector/media view; task/poll gắn hội thoại; security ở account
   settings. ToolsPanel hiện gom nhiều domain, dễ nhầm công cụ nghiên cứu là sản phẩm.
3. zCloud nên cho mở/lưu tệp, quyền/expiry/progress/error; không bắt người dùng nhập
   file_id rồi xem raw key như thao tác cuối.
4. UI cần Supported / Not entitled / Experimental / Unsupported / Unknown. Route
   trong bundle không đủ bật nút hoặc claim được mọi account sử dụng.
5. Event giữ account/thread/request generation/kind; đổi hội thoại và response về
   ngược thứ tự không được thay nội dung màn đang xem bằng dữ liệu cũ.
6. Pending/Sent/Delivered/Seen/Failed/Uncertain khác nhau. Mất ACK không tự gửi lại.
7. Windows ưu tiên IME, DPI, keyboard focus/phím tắt, accessibility, lock notifications.
   Học cấu trúc Olaz, không cần copy Liquid Glass sang Windows.

Chỉ login Web được quan sát live. Main UI PC/Web/Olaz đối chiếu tĩnh; chưa nghiệm
thu pixel/khả dụng mọi panel sau đăng nhập.

## 4. Ma trận chức năng

L: local/native; W: có wiring nhưng cần hợp đồng/nghiệm thu; P: một phần;
X: thử nghiệm/không hỗ trợ; U: chưa đủ bằng chứng. Không tính phần trăm từ các nhãn.

| Nhóm | PC/Web hoặc khác biệt | Fork hiện tại, điểm neo | Khoảng cách |
| :--- | :--- | :--- | :--- |
| QR/session | Login và xác nhận thiết bị | L/W: login.rs, session.rs, profiles.rs | Expiry/revoke/restart, không chỉ happy path |
| Hội thoại/unread | Official state và cursor | L/W: chat.rs, unread.rs, live_updates.rs | Read-point qua restart/nhiều thiết bị |
| Text send/receive | HTTP/socket/echo | L/W: chat.rs, outgoing.rs, worker | Identity và ACK không rõ |
| Quote/mention/forward | Tham chiếu message/enum riêng | L/W: mentions.rs, chat.rs, workspace | Không thay source ID bằng text/timestamp |
| Reactions/receipts | Snapshot và delta/policy | L/W: reactions.rs, reaction_pipeline.rs | Replay/out-of-order/privacy |
| Recall/delete/TTL | Các hành vi riêng | L/W: message_lifecycle.rs, store.rs | Không hồi sinh nội dung qua search/cache/backup |
| Pin message/conversation/star | Ba khái niệm khác nhau | L/W: server_pins.rs, chat/preferences | Local pin không thay server pin |
| Labels/archive/hidden | Server state/PIN | L/W: labels.rs, archive.rs, hidden_chat.rs | Logout/revoke/thiết bị khác |
| Stranger/block/report/privacy | Policy theo quan hệ/account | L/W: privacy.rs, contact_actions.rs | Entitlement/error thật |
| Profile/avatar/contacts | Nhiều thao tác khác đọc profile | L/W: profile.rs, contact_actions.rs, panel | History avatar/quota phải thử riêng |
| Group/community | Group state/control | L/W: groups.rs, group_state.rs, group_entry.rs | Invite/approve/ban/quota/refresh quyền |
| Poll/note/reminder/task | Object riêng, không chỉ text | W: poll.rs, todo.rs, group_content.rs | RSVP/repeat/multi-device không suy từ CRUD |
| Quick messages | Có API server | L: ToolsPanel/preferences | Chưa phải quickmessage server-sync |
| Auto-reply/schedule | Có server families/capability | L: tools.rs/rules | Worker tắt thì lịch local không chạy |
| Photo/file/video/upload | Prepare → resource ready → send | L/W: upload.rs, chat.rs, outgoing.rs | Resume theo ACK; >1GB phải thử được phép |
| Voice message | Recorder khác call | L/W: voice.rs, SendVoiceMessage | Codec/upload/player phía nhận |
| Local gallery/photo viewer | Có nền viewer | L: media_resources.rs, GUI media/viewer | Giữ nền; không gọi hoàn toàn thiếu media |
| Video/range | Byte source/player riêng | L/W: video_player_windows.rs | Seek/expiry/account switch/codec |
| Document preview | Text khác PDF/Office | L/P: media_download::document_preview, doc_viewer | Hiện là prefix text, chưa chứng minh PDF/Office renderer |
| Server media library | list/pagination/preview | P: server_media → worker → ToolsPanel | Cursor bị bỏ; metadata list chưa là browser đầy đủ |
| My Documents | Hội thoại của chính mình | L/W: history/chat/media | Không phải dịch vụ zCloud |
| zCloud | Entitlement/key/metadata/downloader/decrypt | P: zcloud.rs, GetCloudViewerKey | Chưa xong select → usable bytes → open |
| Phone sync | Session/worker/codec riêng | L/W: phone_sync/*, history_sync.rs | Android/iPhone/cancel/resume/partial |
| Backup/import | Khác server cloud restore | L/X: desktop_backup*.rs, phone_sync/legacy.rs | Chỉ claim format giải mã/import được |
| Call 1-1 | PC native helper; Web theo capability | L/W: call.rs, media.rs, windows_video.rs | Hai chiều, double-talk, device recovery |
| Group call/screen share | Signalling và media riêng | L/W: call.rs, screen_capture.rs | Share route không chứng minh pipeline đa luồng |
| Audio DSP | Stack/helper chính thức | L/P: AudioProcessor | Ducking/gate/AGC chưa là AEC3 |
| E2EE DM | libsignal + device/server state | X: crypto/*, e2ee_trial | Custom wire/key boundary cần cross-test |
| E2EE group/media | Sender-key/media-key riêng | X/U | Không dùng ratchet DM thay toàn bộ |
| Multi-account | Chưa chứng minh PC online nhiều account đồng thời | L: profiles.rs, account switcher | Chuyển account khác với luôn online mọi account |
| App lock/notifications | Native integration | L: auth_guard, desktop_notice* | Hello unavailable, lock/popup/activation/wake |
| Device/2FA | Account security routes | W: device.rs, account_recovery.rs | Schema/revoke/recovery thật |
| OA/catalog | Domain/quota riêng | P/U: business.rs | Route không khớp registration đã đọc |
| LAN/transfer | Chỉ có dấu vết route | X: UI báo chưa khả dụng | Không dựng scanner tự chế rồi claim parity |
| AI translation | Controller/consent/API/push | Chưa thấy native integration | Manual trước; auto/suggest theo capability |
| Voice-to-text | Submit + control result | Chưa thấy integration | req_id/result/timeout, không chỉ POST |
| AI sticker | Entry/onboarding, có bước hướng sang mobile | Sticker/GIF Ami | Pack sticker không phải generator/trợ lý |
| AI summary/general assistant | Chưa xác minh trong nguồn này | Chưa thấy model/provider pipeline | Nếu làm là fork deviation |

## 5. Giao thức và API

### Tách đúng lớp

TLS bảo vệ transport; params/signkey/cookie là định dạng/xác thực API, không tự tạo
E2EE hội thoại. Control WebSocket, phone sync và VoIP relay có framing/vòng đời khác.
HTTP success, upload ready, socket echo, delivered và seen là tín hiệu riêng. Không
retry mutation khi kết quả giao tin chưa rõ.

Service map phụ thuộc phiên. PC/Web run mode và UA không trộn tùy tiện. Wire-fingerprint
của Olaz là tham chiếu cách họ nghiên cứu trên macOS, không phải chỉ dẫn áp nguyên
header/cookie/TLS fingerprint vào fork Windows.

### WebSocket: một số dòng roadmap sai

Fresh PC `_getHeader` đọc 4 bytes: version u8, cmd u16 LE, subcmd u8. Enum xác nhận
PING=2, PUSH_MSG_GROUP=521, PUSH_STATUS_1_1=502 và CTRL_PUSH_CTRL=601; fork socket.rs
phù hợp ở các điểm này.

ROADMAP_PHASE_2.md lại ghi 502 là tin nhóm, 521 là recall, 200 heartbeat và 610 typing.
Không dùng bảng đó làm spec code. Nguồn PC đặt 610 trong họ control reaction; phải
đọc full enum/callsite trước khi đổi dispatcher. Không sửa source đúng để khớp plan sai.

PC core offsets byte: `_getHeader` 1.357.561; PING=2 tại 8.891.409; group=521 tại
8.891.493; setUint16(1,...) tại 1.368.320. Fork: socket.rs:42–70, encode/decode_frame.

### Phone sync khác chat E2EE

Fork đã host codec WASM qua wasmi, kiểm hash, memory 64 MiB, output 16 MiB và fuel.
Upstream mới dùng ZprotoHost/Wasmtime thực trong history_sync, không chỉ có helper JS.
Nên so ABI, multi-chunk framing, key/session gate và xử lý batch; không viết lại phần
đã có chỉ vì upstream có commit mới. Giữ account isolation/resource limits của fork.

Hoàn thành phải dựa đủ batch/identity. isLast hoặc một trang tải xong không có nghĩa
đủ lịch sử. Không commit private sync keys hay dùng chúng làm fixture công khai.

### Chỉ mục API và hợp đồng

Hai core bundle có **460 quoted path /api** sau bỏ query. Đây là chỉ mục chuỗi,
không phải 460 API hoạt động. Chỉ mục 421 cũ khác phương pháp/phạm vi; chênh lệch
không phải số chức năng mới. [API_LITERALS.csv](API_LITERALS.csv) còn liệt kê text
mentions trong Rust; chúng có thể là comment/test, URL cũng có thể dựng động.

Mỗi contract vẫn cần service, method, request/response, quyền, ACK, cursor và caller.
Không lấy route count hoặc Rust text hits làm phần trăm hoàn thiện.

| Họ API | Bằng chứng E2 | Hướng triển khai |
| :--- | :--- | :--- |
| 13395/13396 | Setting read/update cùng `/api/aifeatures/translate/v1/setting` | Chốt method/shape/capability, không suy từ tên |
| 13397/13398/13399 | Manual/auto/suggest dưới `/api/aifeatures/translate/v1/` | Controller, push result, consent/quota |
| 13394 | `/api/aifeatures/voice2text`, control voice2text/convert_finish | req_id → đúng message → result/timeout |
| 12038/12039 | `/api/prodcatalog/catalog/list`, `/api/prodcatalog/product/list` | Catalog khác product; cần full caller/service |
| Quickmessage/autoreply | list/create/update/delete | Server object khác preferences/rules local |
| Media store | mediastore/list và media/oneone/list | Giữ Page/cursor/context đến UI |
| zCloud | Viewer key, verify, downloadurls, cloud state | Key khác file; signed URL khác plaintext bytes |
| E2EE send | message/* và group/* | Route không chứng minh custom container/prekey API đúng |
| LAN | status/friends/submit/leave/samemacs | Chưa là protocol truyền tệp hoàn chỉnh |

## 6. AI: những gì nguồn thực sự cho thấy

### Dịch hội thoại

Cả PC và Web có nhóm API dịch. Chunk Web
ai-translation-language-model.83623ce6f225ae411a33.js export detectTextLanguage;
đây là nhận diện ngôn ngữ, không chứng minh có LLM dịch offline trên client.
Bundle còn chứa playground/mock; mẫu mock không được dùng làm schema production.
Có adapter/controller gọi API command, nhưng cần fixture đúng runtime/account.

Ba trải nghiệm riêng: dịch một tin, dịch tin đến tự động và gợi ý bản dịch khi soạn.
Có consent xử lý nội dung và điều kiện/gia hạn dịch vụ. Zalo Help mô tả hai chức năng
tự động và quyền dùng zCloud. [H1]

Fork chưa có UI → worker → provider → result cho AI dịch. Nên làm manual translation
trước, giữ nguyên bản, nhãn nguồn, copy/retry và chọn ngôn ngữ. Bản dịch không tự gửi.
Auto/suggest cần generation để response cũ không thay draft mới; entitlement phải
được xác minh, không tự bật vì route tồn tại.

### Tin thoại thành văn bản

PC có CMD 13394; Web có submit → req_id → control result, timeout, late result và
resource recovery. HTTP submit OK chưa có nghĩa có transcript. Đây là mẫu async
đáng học cho cả việc tải tệp, đồng bộ và xác nhận gửi.

Có chuỗi UI biên bản cuộc họp nhưng chưa truy đủ workflow để khẳng định transcription/
tóm tắt call PC/Web hoàn chỉnh. Bài giới thiệu AI của Zalo nói về hệ sinh thái; không
suy mọi tính năng mobile đều hiện diện trên desktop. [H3]

### Hai lộ trình AI cho fork

- Parity: API/capability thực, không bỏ qua quyền dịch vụ.
- AI riêng: semantic search, tóm tắt đoạn chọn, gợi ý trả lời, OCR/ASR local. Đây là
  fork deviation. Chỉ xử lý/gửi dữ liệu người dùng chọn tới provider đã cấu hình;
  không tự gửi toàn DB hoặc tự thao tác/gửi tin. Model local phải đo RAM/CPU ARM64.

Mặc định không xuất E2EE/hidden chat sang provider. Consent cần nói rõ dữ liệu/nơi
nhận. Cache theo account/thread/message revision/model; recall/delete làm mất hiệu
lực kết quả. Log stage/duration/size/status, không prompt/plaintext/key/ciphertext.
Sticker Ami trong fork không phải bằng chứng có AI assistant.

## 7. Phát hiện và khoảng cách cụ thể

Mức dưới đây là ưu tiên kỹ thuật, không phải kết luận đã khai thác trên tài khoản thật.

### F01 — P1: media server có thể báo empty thay lỗi inner/schema

server_media::parse_server_media_list trả Page thay Result; thiếu items/medias/list
thì trả items rỗng. Fetch helper maybe_decrypt rồi bọc parser trong Ok. Session chặn
lỗi HTTP/business ở envelope ngoài; inner envelope sau giải mã/sai schema vẫn cần
parser kiểm tra riêng.

Suy trực tiếp từ source: nội dung đã giải mã `{"error_code":114,"data":null}` hoặc
success thiếu list → items rỗng → ServerMediaLoaded. Chưa chạy request/harness case này.
Sửa parser Result/strict envelope; test outer error, inner error, malformed và valid
empty. Không đổi toàn Session để che lỗi một parser.

### F02 — P1: mất cursor và server media consumer chưa hoàn chỉnh

worker.rs:3316 luôn page=1/count=30; event chỉ giữ items, bỏ has_more/next_cursor.
GuiEvent không giữ thread/request/cursor; GUI bỏ media_type. ToolsPanel chỉ vẽ tên,
kích thước/sender, không thấy mở/tải từ row của tab này.

Sửa truyền Page+owner/thread/kind/generation, load-more/cursor; kết nối row với viewer/
downloader đã có. Test response ngược thứ tự, đổi thread/type, hết trang, lỗi/expiry.
Không gọi gallery local hiện có là thiếu: vấn đề ở riêng đường media server.

### F03 — P1: zCloud chưa là restore/open flow; raw viewer key hiện trong UI

Đã truy GetCloudViewerKey → API → CloudViewerKeyLoaded → map → ToolsPanel.
verify_cloud_media/get_cloud_download_urls có helper nhưng không thấy production caller
ngoài định nghĩa trong source đã tra. tools_panel.rs:910 format k.viewer_key trực tiếp,
trong khi tiêu đề nói khôi phục/mở tệp.

Key nên nằm trong worker; GUI chỉ nhận quyền/expiry/progress. Hoàn thiện chọn/list →
entitlement → metadata/key → download/decrypt → validate bytes → cache → open; hoặc
gate rõ chưa hỗ trợ. Raw key trong UI làm tăng rủi ro screenshot/chẩn đoán, chưa phải
bằng chứng đã truyền key ra ngoài.

My Documents, sync điện thoại và zCloud là ba capability khác nhau. Help chính thức
còn phân biệt dữ liệu cũ PC/Web khi đưa lên zCloud. [H2]

### F04 — P1 hợp đồng: catalog route không khớp registration đã đọc

business.rs:102 gọi `/api/prodcatalog/list`; PC đăng ký catalog/list (12038) và
product/list (12039). Không thấy route fork trong literal đã quét. Đây là khác biệt
source đã xác minh, chưa phải HTTP failure tái hiện.

Truy đúng service/method/params/caller và fixture trước; không rename route mù quáng.
Nghiệm thu cần account có quyền, pagination/quota/error và consumer render sản phẩm.

### F05 — P1 tương thích: E2EE model khác official wire/key boundary

- crypto/wire.rs:94 có fixed-width header riêng, MAC 32 bytes. Official libsignal
  có protobuf Whisper/PreKeyWhisper, decrypt tách MAC 8 bytes; PC gọi SessionCipher
  rồi concatMsgWithType, không dùng fixed header của fork.
- Fork tạo DH identity và signing identity độc lập. Official KeyHelper nhận identity
  public key 33 bytes, ký signed-prekey bằng identity private key. Parser 32-byte của
  fork cần adapter/proof; không cắt/thêm prefix tùy ý rồi coi tương thích.
- PREKEY_BATCH=5 trong Olaz standalone glue là lựa chọn demo; helper PC default 5
  nhưng actual bundle/refill path có lời gọi batch 20. Demo không là protocol constant.
- Trial prekey/get và prekey/update chưa có hợp đồng được chứng minh. Đọc bổ sung
  xác nhận client dùng WS SUBMIT_BUNDLE=540, GET_BUNDLE=541, SUBMIT_PREKEYS=542
  cùng request correlation; xem mục 13. Không suy từ đó rằng mọi endpoint REST
  server đều không tồn tại.

Giữ E2EE thử nghiệm và log đã che dữ liệu. Trước khi claim parity: pin version và
cross-test bytes/key/signature với official library; sau đó E1 ở account kiểm tra.
Thứ tự: key representation → signature/prekey/service → wire/MAC → lifecycle → text →
media → group sender-key. Rust↔Rust roundtrip không chứng minh Zalo đọc được.

outgoing::check_thread_e2ee hiện trả false khi is_group. Nếu hỗ trợ nhóm mã hóa, policy
phải tách scope DM/group; không lấy guard DM làm bằng chứng mọi nhóm fail-closed.
Chưa gửi thử nhóm E2EE thật trong lượt này.

### F06 — P2: DSP hiện chưa phải WebRTC AEC3

AudioProcessor::process_capture dùng speaker_energy giảm toàn frame mic rồi return;
nhánh khác noise gate/AGC. Không có reference PCM/filter echo path/delay adaptation
của AEC3 trong implementation này. Khi double-talk, logic có thể giảm cả giọng người
dùng; đây là suy luận từ thuật toán, chưa là phép đo âm thanh thật.

Giữ nền, sửa nhãn mô tả; nếu nâng cấp, đo double-talk/echo/dropout/latency/CPU/pin trên
ARM64 với speaker/wired/Bluetooth và đổi thiết bị. Test RMS/gain không thay nghiệm thu.

### F07 — P1 tài liệu: roadmap có chỉ dẫn sai/quá mạnh

CMD table ở mục 5 sai; ROADMAP_ZALO_PARITY còn ghi hoàn tất 100%, trong khi E2EE/zCloud/
LAN có giới hạn source đã truy. AI khác có thể sửa code đúng thành sai nếu dùng plan
làm spec.

Cần contract ledger: source/version/offset/E-level/caller/fixture/device acceptance.
Tách Implemented, Fixture-tested, Live-verified, Unsupported, Unknown; không đổi module/
API/helper thành hoàn thiện tính năng.

### F08 — P2: tên tính năng lớn hơn hành vi

DocViewerModal nhận String, preview tải prefix text: chưa là PDF/Office renderer.
Quick messages lưu preferences local, không server sync. Scheduler cần app đang chạy.
Profile switch không tự chứng minh nhiều account online đồng thời. Sticker Ami không
là AI generator/trợ lý. Sửa nhãn/phạm vi và hoàn thiện luồng; không xóa nền đang có.

## 8. Nên học gì từ Olaz

1. Native UI và engine tách nhau: engine trả typed snapshot/delta; thao tác cần dữ
   liệu không chỉ trả toast. Đặt protocol correctness ở một lớp có thể test.
2. Adapter theo feature: chat/contact/transfer/cloud có input/event cụ thể, dễ truy
   tới callsite chính thức hơn một worker handler tổng lớn.
3. Onboarding/state machine: chờ điện thoại, partial, timeout, cancel và ready riêng;
   preparation không thành thành công giả.
4. Pin version cho runtime/bridge/codec và chạy lại contract tests khi đổi nguồn.
   Renderer có feature không có nghĩa UI native đã expose feature đó.
5. Quan sát hình dạng dữ liệu/latency/queue/error; tách research harness khỏi binary
   người dùng. Không dump message/token chỉ để debug tương thích.

Không bê JSBridge/SignalE2EE.swift và zalo-e2ee.js thành engine live: phần standalone
này có in-memory peer/demo store, còn luồng live Olaz dùng ASAR. Không coi 450 bridge
members là 450 chức năng xong: ZaloMainProcess có handler ENOTSUP cho renderer fallback;
README còn ghi create-group chưa nghiệm thu live. Source tham chiếu: [host][O1],
[chat adapter][O2], [standalone E2EE][O3].

Olaz GPL-3.0; fork MIT/Apache-2.0. Cần rà license/phạm vi phân phối trước khi chép code
hoặc vendor library. Báo cáo này không đưa code Olaz/VNG vào app.

## 9. So với upstream

`git rev-list --left-right --count HEAD...upstream/main` = **172 / 77**: 172 commit
chỉ local, 77 chỉ upstream tại snapshot đã pin. Hai phía diverge; không phải 77 bản
sửa đều cần lấy. Version 0.1.0 và 0.0.16 dùng lịch khác, không là thang chất lượng.

| Nhóm upstream | Đáng học | Cách áp dụng cho fork |
| :--- | :--- | :--- |
| f10f9eb — ZprotoHost/Wasmtime | In-process WASM và exports sync | So ABI/error/framing; giữ wasmi/hash/resource budget nếu chưa có benchmark lý do đổi |
| 368caf5 — keys/framing/session | Key thật, multi-chunk, gate phiên | So vectors/phone_sync; không chép global-key model làm mất account isolation |
| e319be4 — read-point | SQLite last_read_msg_id | Test restart trước; fork đã serialize UnreadState trong thread cache, không hoàn toàn thiếu persistence |
| 6291aec/689c2bd — reactions | Accumulate/dedup/delta | So reactions.rs/reaction_pipeline.rs; dùng replay/out-of-order fixtures |
| 492964a — clipboard | Bounds/DIB validation | So clipboard_image.rs hiện có, tránh lấy lùi hardening |
| b636f69 — SSRF/media | URL/filename safety | Giữ allowlist/redirect/DNS/proxy của fork |
| UI 65c7117, hover/context commits | Density/scroll/menu | Chọn patch có lợi, test DPI/IME/focus; không overwrite workspace/worker |
| d1f9541 — call/notification | Group signalling/UI | Đối chiếu media thật và lock privacy; changelog không thay nghiệm thu |

Điểm neo thật: [zproto_host.rs][U1], [history_sync.rs][U2]. Không merge/cherry-pick
trong lượt này. Nên port ý tưởng/test theo domain, không merge mọi source file lớn.

## 10. Lộ trình cải tiến đề xuất

Đây là thứ tự phụ thuộc, không là lịch cam kết hoặc lệnh tự triển khai.

| Ưu tiên | Gói | Đầu ra và điều kiện đạt |
| :--- | :--- | :--- |
| P0 | Contract ledger/capability | Chỉnh bảng CMD sai, rõ Experimental/Unknown; mỗi feature có source/caller/event/consumer |
| P1 | Server media browser | Parser strict; giữ Page/context/cursor; load-more/open/save; error không thành empty và response stale không ghi đè |
| P1 | zCloud UX/key handling | Không raw key; đủ chọn → tải/giải mã → mở hoặc báo capability chưa hỗ trợ; thử quyền/expiry/cancel/revoke/bytes thật |
| P1 | Catalog contract | Tách catalog/product, đúng service/method/schema; fixture và account có quyền; pagination/error/UI đều có consumer |
| P1 research | E2EE interoperability | Vectors pin version, native ↔ official; sau đó thử bên nhận official; giữ chế độ thử/log, không fallback plaintext |
| P2 | AI manual translate | Theo message, consent/provider/capability, original/result; không auto-send; stale/cancel/error/quota được xử lý |
| P2 | Voice transcript | Submit/req_id/result state machine; late/timeout/recall không hiển thị sai nội dung |
| P2 | Call quality | AEC/reference pipeline hoặc nhãn DSP trung thực; đo double-talk/dropout/CPU/pin ARM64 |
| P3 | Native Windows UX | Onboarding/inspector/keyboard/accessibility; DPI/IME/focus/lock notifications đạt nghiệm thu |
| P3 | AI riêng | Tóm tắt đoạn chọn/semantic search nếu cần; provenance, consent, resource budget và lifecycle invalidation |

### Tối ưu thuật toán/tài nguyên

- Giữ map merge thread, FTS5/index, outgoing identity và bounded media/codec đã có;
  không đề xuất lại đổi Vec sang HashMap chung chung khi đường đó đã dùng map.
- Coalesce delta theo account/thread/message/kind, không theo text. Giới hạn event
  count lẫn bytes; tránh sync lớn buộc UI clone/sort mọi lịch sử mỗi frame.
- DB/search dùng keyset cursor/query plan/index và page bound. Recall/hidden/TTL
  áp dụng cả search và AI index; rebuild index phải resumable.
- Media dùng visible-item priority, texture budget, cancellation ticket/cache revision;
  không tải/decode tất cả chỉ để vẽ sidebar, giải phóng account resources cũ.
- AI debounce draft, dedup revision/language/model, cancel khi đổi thread, cap context/
  concurrency; lỗi/rỗng không bị cache thành success hợp lệ.
- Call dùng fixed-size PCM/ring/bounded queue; tránh allocation/log I/O ở callback.
  Đo callback time, queue age, dropped frame, drift; không chỉ tăng queue để hết giật.
- Sync checkpoint theo batch xác nhận. Tiến độ tách fetched/decoded/persisted/visible;
  không drop payload lỗi rồi báo 100%.

Chưa có benchmark mới cho các đề xuất trên. Cần baseline p50/p95/p99/input latency,
RAM/CPU/pin trên ARM64 với dataset cố định trước khi chọn dependency/thuật toán mới.

## 11. Mẫu giao việc và nghiệm thu

Mỗi ticket ghi: hành vi người dùng → source hiện tại → nguồn PC/Web file/hash/offset
→ phần chưa biết → thay đổi nhỏ → regression → native/live acceptance. Ví dụ:

> Mở media của A, bấm tải thêm rồi chuyển B. Response A về sau B không được thay B.
> Inner error hiện lỗi; valid empty hiện empty. Page/cursor/account/thread/generation
> đi hết tới app state. Chọn item phải dùng viewer/downloader, không chỉ hiện tên.

Không giao một ticket “sửa toàn bộ parity”. Route/TODO không là server contract.
Không làm yếu test để vượt gate. Capture chỉ lưu fixture đã che dữ liệu/provenance,
không session, DB, dump hoặc key. Tài liệu người dùng gửi sau đã được đối chiếu ở mục 13; không tự tạo một file
AI_ENGINEERING_GUIDE giả định có cùng nội dung.

## 12. Chứng cứ và nguồn

- [SOURCES.csv](SOURCES.csv): hash và nguồn từng asset, gồm tầng raw/decoded.
- [API_LITERALS.csv](API_LITERALS.csv): 460 quoted path, không phải coverage/contract.
- [EVIDENCE.csv](EVIDENCE.csv): byte offset/term để truy nguồn. Pattern hit là chỉ mục,
  không tự thành một kết luận; có cả mẫu mock phải phân biệt với production.
- Snapshot/detail JSON ngoài repo: E:/tmp/zalolite-parity-20261110.
- App source giữ nguyên ở 18607f6; báo cáo/CSV chưa commit/push.

[H1]: https://help.zalo.me/huong-dan/chuyen-muc/nhan-tin-va-goi/nhan-tin/tu-dong-dich-tro-chuyen/
[H2]: https://help.zalo.me/huong-dan/chuyen-muc/dich-vu-nang-cao/giai-dap-ve-zcloud/zcloud-luu-tru-du-lieu-zalo-nhu-the-nao/
[H3]: https://zalo.me/vi/post/ai-ecosystems/
[O1]: https://github.com/qnguyen3/zalo-swift-re/blob/07bd55df8f9d5c22e11204bc8b9a4783b60c9b7a/Sources/Olaz/ZaloRuntime/OfficialClientHost.swift
[O2]: https://github.com/qnguyen3/zalo-swift-re/blob/07bd55df8f9d5c22e11204bc8b9a4783b60c9b7a/Sources/Olaz/HostScripts/chat-adapter.js
[O3]: https://github.com/qnguyen3/zalo-swift-re/blob/07bd55df8f9d5c22e11204bc8b9a4783b60c9b7a/Sources/Olaz/JSModules/zalo-e2ee.js
[U1]: https://github.com/ctdoteam1110/ctdozlite/blob/3d9e69074b14cb55c7e4c5e5d6d7728baa4c47af/src/zproto_host.rs
[U2]: https://github.com/ctdoteam1110/ctdozlite/blob/3d9e69074b14cb55c7e4c5e5d6d7728baa4c47af/src/history_sync.rs

## 13. Đối chiếu tài liệu người dùng bổ sung

Nguồn: D:/Henry/Downloads/zalo_pc_web_olaz_upstream_deep_analysis.md. Các chỉ dẫn
P0/P1/P2 trong tài liệu là đề xuất của tác giả, chưa được thực thi trong lượt này.

### Những thông tin đã kiểm tra và bổ sung được

- ASAR có `pc-dist/signal/MessageProtocol.proto`: proto2/package MessageProto,
  SignalMessage, PreKeySignalMessage và SenderKey messages có các field mô tả.
  Đây là bằng chứng schema hữu ích hơn việc chỉ thấy tên route.
- Fresh PC source có INIT.SUBMIT_BUNDLE=540, GET_BUNDLE=541, SUBMIT_PREKEYS=542,
  GET_LIST_USER_E2EE=563, GET_KEY_ID=564, RECEIVE_ONEONE=551 và RECEIVE_GROUP=552.
- `_submitBundle` xây gói WS với cmd/subCmd và data chứa reqId + nested data;
  `getBundle` gửi uids gồm uid/deviceId, kèm cờ reinit/notification/auto-upgrade.
  Không được rút bảng CMD thành một lệnh gửi thiếu envelope/correlation/error path.
- concatMsgWithType ghép type byte với binary body và base64. `sendMsgE2ee` có
  nhánh listParams/destType/threadId/clientId. Phải truy đầy đủ fan-out, own subdevice,
  key metadata và ACK; không thay wire.rs đơn lẻ rồi coi đã tương thích.
- Có native Kiki OAuth bridge trong main process và bridge spec Olaz; đây là điểm
  cần bổ sung cho phần AI. Chưa xác minh model, entitlement hoặc full chatbot UI live.
- Có OCR/native language detector. Tham số OCR trong source đã đọc có cấu hình
  max_pixel_size=1280 và object threshold, không khớp mô tả cố định 960/0.6 của tài liệu.
  Cần tách default/config override; không hardcode một giá trị từ ghi chú.

### Nhận định không khớp hoặc quá mạnh

| Nhận định tài liệu | Kết quả đối chiếu | Cách sử dụng đúng |
| :--- | :--- | :--- |
| Fork chưa chạy libzproto WASM trong Rust | Sai với source hiện tại: phone_sync/codec.rs dùng wasmi, hash/fuel/memory limit; transfer.rs gọi Codec | Không port thêm Wasmtime như việc còn thiếu; chỉ so ABI/behavior/benchmark |
| Đã giải mã hoàn chỉnh/chính xác tuyệt đối mọi câu hỏi E2EE | Chưa đủ: đã tìm được schema/WS/wrapper, nhưng lifecycle/server acceptance, error/retry/fan-out/media/group vẫn cần kiểm chứng | Nâng phần đã biết lên E2, giữ các câu hỏi còn lại rõ ràng |
| Olaz tương thích 100% tự động | Không có bảo đảm: host bridge có ENOTSUP/fallback, feature flags/version drift và UI coverage riêng | Reuse renderer giảm protocol work, không loại bỏ integration risk |
| Fork RAM <50 MB, binary 15–25 MB, khởi động tức thì/vượt trội hoàn toàn | Không có benchmark tương đương trong tài liệu; kích thước/working set phụ thuộc build và workload | Đo cùng dataset/device/build/profile; không dùng như acceptance |
| src/sync_engine.rs, crypto/session_manager.rs, backup_crypto.rs, src/doc_viewer.rs là điểm neo hiện tại | Các đường này không tồn tại trong checkout; doc viewer thật ở src/gui/doc_viewer.rs | Sửa mapping trước khi giao AI viết code |
| UI đã tách sạch 31 module, có Kanban/mind-map/RTF | Có 30 file .rs trong src/gui tại snapshot; gui.rs và worker.rs vẫn lớn; chưa có bằng chứng đầy đủ cho các feature kể thêm | Dùng call path thật thay con số/nhãn quảng cáo |
| Fork xác thực chữ ký updater | Source có checksum và PE/architecture checks; không thấy Authenticode/WinVerifyTrust verification | SHA-256 và chữ ký PE định dạng không phải chữ ký số nhà phát hành |
| Áp Origin https://chat.zalo.me và toàn bộ header giống Electron | Chính wire-fingerprint Olaz nói XHR từ file:// không có Origin/Referer; WS khác | Tách Web/PC/macOS/Windows, chỉ thay header có bằng chứng đúng flow |
| Lưu read-point/reaction là việc hoàn toàn chưa có | Fork đã serialize UnreadState và có reaction pipeline/detail paths | Tái hiện gap trước; không thêm schema/port trùng chỉ vì upstream có commit |

### fileNoise: hướng nghiên cứu hợp lý, chưa phải patch an toàn tuyệt đối

Nguồn PC có cache/fileNoise và native file-processing integration. Tuy nhiên chưa
xác minh đầy đủ thuật toán marker/XOR được nêu từ đúng native implementation/call
path; cũng chưa tìm được đoạn triển khai tương ứng ở BridgeImplementation.swift
của snapshot Olaz đã đọc. Không kết luận thuật toán sai, nhưng không thêm XOR vào
mọi download chỉ từ mô tả trong tài liệu.

Cần fixture có input/output/checksum, phân biệt plain/noised/encrypted/cloud, thứ tự
các transform, xử lý range/streaming và tránh decode hai lần. Test file nhỏ/64 KiB/
lớn, marker trùng tình cờ, partial download/resume và invariant bytes sau xử lý.
Đây là conditional transform theo hợp đồng, không phải giải mã E2EE.

### Điều chỉnh thứ tự ưu tiên sau khi đọc tài liệu

1. Giữ ưu tiên sửa thông tin contract/roadmap và hoàn thiện media/zCloud/catalog.
2. E2EE research cụ thể hơn: lấy proto + WS 540/541/542 + request/result correlation
   làm nền, kiểm tra key representation/signature/MAC và fan-out bằng vectors trước.
   Giữ trial/log hiện có; không coi file tham khảo là lệnh đổi ngay transport/schema.
3. Bỏ hạng mục port WASM còn thiếu; fork đã có host wasmi. Chỉ port khác biệt có test.
4. Đưa OCR/Kiki bridge vào capability inventory; native OCR Windows là lựa chọn
   thiết kế cần feasibility/ARM64/language/resource tests, chưa phải feature đã có.
5. fileNoise và Android backup cần đúng format/callsite/fixture trước khi implement;
   không cam kết giải mã mọi db.crypt hoặc khôi phục toàn bộ lịch sử chỉ từ thuật toán.
