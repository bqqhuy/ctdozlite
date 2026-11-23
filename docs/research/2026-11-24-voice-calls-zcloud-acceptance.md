# Nghiệm thu voice message, cuộc gọi và zCloud

Ngày 24/11/2026. Đây là checklist cho bản triển khai sau báo cáo nghiên cứu.
Mọi checkbox dưới đây để mở; phiên nghiên cứu chỉ test client Zalo gốc.

Liên quan: [phân tích](2026-11-24-voice-calls-zcloud-analysis.md),
[gói triển khai](2026-11-24-antigravity-voice-calls-zcloud-tasks.md),
[bằng chứng tham chiếu](../../re-notes/74-voice-call-zcloud-ui-and-contracts.md).

## 1. Cách ghi bằng chứng

Với mỗi ca: ghi commit + dirty diff/build hash; Windows/ARM64/DPI; thiết bị
input/output/camera và peer PC/mobile version; role caller/callee; thời điểm;
bước thực hiện; expected/actual; metadata log đã làm sạch; kết quả và giới hạn.
Không lưu session, token, signed URLs, giọng nói, transcript hoặc hình camera
của người thật vào Git. Ảnh bố cục dùng fixture giả cho việc chia sẻ.

| Lớp | Chứng minh được | Không thay thế |
|---|---|---|
| Source audit | Đường chạy, ownership, evidence của contract | Request thành công hoặc UI thật |
| Unit/model | Transition, bounds, parser và local behavior cụ thể | Server contract hoặc thiết bị |
| Fixture/loopback | Request qua helper thật khớp fixture có nguồn | Interoperability với Zalo hiện tại |
| Native local | CPAL/MFT/device/GUI chạy trên Windows ARM64 | Peer chính thức nhận/phát được |
| Native interoperability | Kịch bản fork↔PC/mobile đã thực hiện | Các tình huống khác chưa chạy |

UI-LIVE trên Zalo PC trong re-notes/74 chỉ xác nhận reference UX. Không tick các
mục fork bên dưới từ cuộc gọi reference đó. Không copy số test của phiên khác.

## 2. Voice message

- [ ] V01 Recorder start/cancel/stop không chặn UI; level/duration thật, mic được
  nhả khi hủy, logout, explicit Exit và lỗi callback.
- [ ] V02 Capture các sample format/channel/rate cần hỗ trợ; 44.1/48kHz resample
  giữ pitch/thời lượng, không clip/phát nhanh/chậm. Min/max/empty/overflow có lỗi
  rõ; giới hạn local không được mô tả là giới hạn server khi chưa có bằng chứng.
- [ ] V03 Preview nghe thử, pause/resume/replay; Ghi lại/Hủy không gửi mạng;
  draft text/reply không mất, chat/account binding giữ nguyên.
- [ ] V04 Native request fixture xác minh service/params/cipher/multipart/codec,
  không gửi WAV như generic file rồi đổi enum để giả voice.
- [ ] V05 Gửi voice 5–10 giây từ fork tới đúng tài khoản thử: PC/mobile hiện
  bubble thoại; nghe trọn, duration khớp, không là attachment thông thường.
- [ ] V06 DM và group được nghiệm thu riêng. Không gửi nhóm chưa được chỉ định.
- [ ] V07 Bubble hiện trước I/O nặng; pending/progress/error đúng cùng client ID;
  echo-before-ACK, reload, duplicate echo không tạo row thứ hai.
- [ ] V08 Timeout trước submit vs sau submit: không tự gửi lại mutation chưa rõ
  outcome; retry có căn cứ, giữ identity; file tạm không bị xóa trước retry.
- [ ] V09 Incoming/history voice play/pause/replay inline, đổi voice/chat/account,
  codec lỗi, URL hết hạn, device lỗi, cache budget và cleanup.
- [ ] V10 AI collapsed→pending→text/error, transcript inline co giãn, collapse;
  quota/permission/timeout/early-push/late-push/duplicate có hành vi xác định.
- [ ] V11 Recall/delete/TTL/account switch khi đang tải/phát/AI: dừng đúng audio,
  event muộn không đưa text hoặc audio trở lại; protected/E2EE guard giữ nguyên.
- [ ] V12 UI 380/800/1280px và DPI 100/125/150/200%, light/dark, keyboard/focus,
  text Việt dài, spinner/error không đè nút Gửi/Hủy hoặc làm nở sidebar.

## 3. Cuộc gọi thoại 1–1

- [ ] C01 Header đúng voice/video; self-thread không gọi; recipient xác định từ
  exact account/UID, không dựa display name trùng.
- [ ] C02 Preflight mic/speaker selectors, meter/test speaker thật; missing device
  và privacy denial có hướng dẫn; Cancel không allocation, Start chỉ tạo một call.
- [ ] C03 Outgoing fork→PC/mobile: allocating/ringing/connecting/connected thể hiện
  đúng; timer đàm thoại từ connected. Caller nghe và callee nghe là hai kết quả
  riêng, không suy từ UDP sent_ok.
- [ ] C04 Incoming PC/mobile→fork: ring UI/notification, accept/reject, double-click,
  peer cancel khi đang accept; không leak ring/mic hoặc call mới bị event cũ đóng.
- [ ] C05 Busy/no answer/declined/allocation rejected phân biệt, không hiện sent/
  connected giả; retry/callback tạo intent mới đúng recipient.
- [ ] C06 Mute/unmute thật ở remote, pending/failed apply; chọn mic/loa có ACK
  phản ánh active stream, unplug/replug/default change không treo hoặc backlog.
- [ ] C07 Headset và loa ngoài; nói luân phiên rồi double-talk; không echo/hú,
  không mất hẳn tiếng người nói vì ducking. Ghi quan sát và metric trước/sau.
- [ ] C08 Nhận trên điện thoại khi fork đang reo: fork dừng ring/UI/local media;
  cuộc gọi remote tiếp tục. Test âm tính: local accepted hoặc outgoing connected
  không bị duplicate answer làm đóng. Liên kết C-T002/C-T090 của spec 007.
- [ ] C09 End/cancel/reject cũ, terminal-before-invite, duplicate, reconnect queue,
  logout/same-account relogin, hai call ID gần nhau có fixture và native ca chính.
- [ ] C10 Remote hangup, local hangup, close viewport, main hide-to-tray, minimize,
  explicit tray Exit: cleanup đúng, không thread/socket/camera/mic leak.
- [ ] C11 Call card type/direction/reason/duration đúng; Gọi lại hoạt động đầu-cuối,
  local record không giả server message.

## 4. Video 1–1 và khả năng nâng cao

- [ ] C12 Camera capture→H.264 encode→decode local trên ARM64; ghi native MFT test
  thực chạy, không tính ignored test là pass.
- [ ] C13 Outgoing/incoming fork↔PC/mobile thấy hình thay đổi hai chiều; local
  preview không được dùng làm bằng chứng remote nhận được video.
- [ ] C14 Remote portrait/landscape giữ tỷ lệ, resize/high DPI không giãn/cắt ngoài
  viewport; local PIP/generation đúng, không frame call trước.
- [ ] C15 Local camera off vẫn nhận video remote; remote off/on placeholder và
  keyframe recovery đúng, không giữ ảnh cuối làm tưởng camera còn bật.
- [ ] C16 Camera menu preview và selected state thật; hot-switch/failure/call end
  trong lúc chọn không restart call khác hoặc giữ camera nền sau hangup.
- [ ] C17 Đo first-frame, freezes, CPU/memory, packet loss/jitter, A/V sync;
  30 giây và 3 phút là test budget đề xuất. Tiêu chí định lượng cuối phải ghi
  trước run theo thiết bị, không dựng số đạt sau khi thấy kết quả.
- [ ] C18 Network interruption và relay path: packet ở hai socket, RTCP/PLI,
  dropped fragment, queue bound/decoder recovery; không retry signaling mutation
  tự động. Không đổi wire framing sang RFC thuần khi khác evidence Zalo.
- [ ] C19 Audio→video cùng call ID: handshake/capability/decline, request pending,
  không bật cam trước policy/consent; chưa đủ evidence thì giữ chưa hỗ trợ.
- [ ] C20 Group theo ADR-0003: allocation/join/member/leave, self accepted elsewhere
  khác other-member answered, mix audio, limit hai remote video, no DM fan-out.
- [ ] C21 Screen share: chọn nguồn, capture/send/remote render, stop/remote stop,
  account/call cleanup. Builder signal test không đủ đóng mục này.

## 5. zCloud

- [ ] Z01 Credential AUTH GET và cloud enk đúng reference; fixture của executor
  xác nhận credential placement, decrypt/envelope/expiry/refresh.
- [ ] Z02 Không secret-bearing GuiEvent/UI cache/log; late key sau logout/relogin
  không apply. Metadata event bind owner/runtime/asset/request.
- [ ] Z03 Verify/download noise ID đúng metadata, không suy file_id; error/missing/
  malformed/partial data phân biệt khỏi not-in-cloud/expired.
- [ ] Z04 Một asset thật có quyền: click từ bubble/drawer → status → download →
  integrity/atomic finish → preview/open. Không phải chỉ toast lấy key thành công.
- [ ] Z05 Hết quyền, signed URL hết hạn, rate limit, network error và server expiry
  có lỗi tương ứng; URL refresh bounded, không gửi credential tới host ngoài scope.
- [ ] Z06 Download cancel/account switch/Exit: không file hoàn chỉnh giả, không
  temp file/job treo, event cũ không cập nhật asset account mới.
- [ ] Z07 My Documents quota, Server Media Library và zCloud hiển thị đúng nguồn;
  unknown quota không 0, thiếu expiry không “Không thời hạn”. Không tự mua plan,
  reset storage, upload hoặc chuyển dữ liệu để làm test đọc thành công.
- [ ] Z08 UI người dùng không nhập file_id hoặc nhìn viewer key; badge cloud,
  progress, cancel và actionable error nằm đúng tệp, narrow/high DPI không vỡ.

## 6. Kiểm chứng tích hợp và điều kiện kết thúc

- [ ] Compiler all-targets, fmt, relevant fixture/integration, full test phù hợp,
  Clippy -D warnings, build ARM64 và diff check có command/exit code mới.
- [ ] Không tác động các sửa sync/config đang dirty; review diff theo baseline.
- [ ] Hai lượt audit sạch liên tiếp cùng diff; finding mới làm reset streak.
- [ ] `specs/007`, `009`, `011` cập nhật evidence theo requirement/task ID cũ;
  đánh dấu riêng mã/fixture/native. Không đóng native bằng ảnh renderer giả.
- [ ] Report cuối ghi phần chưa đạt và ca chưa chạy, build dùng để nghiệm thu,
  rollback theo thay đổi thực; không rollback bằng copy DB/account khác.

## 7. Bản ghi nghiệm thu ngắn

```text
ID: Vxx / Cxx / Zxx
Build SHA/hash + dirty delta:
Windows/ARM64/DPI và devices:
Peer client/version + role (không public UID):
Thời gian và duration:
Bước tái hiện:
Expected:
Actual:
Evidence: fixture nguồn / sanitized metric / ảnh dữ liệu giả:
Result: PASS / FAIL / CHƯA CHẠY / BLOCKED:
Limitation và follow-up task:
```
