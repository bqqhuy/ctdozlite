# Kế hoạch hoàn thiện hệ thống và nghiệm thu parity Zalo PC/Web

Ngày: 08/11/2026 (Asia/Saigon). Trạng thái: đề xuất thực thi, chưa phải xác nhận hoàn thành.

**Cập nhật hợp nhất audit hệ thống + E2EE chuyên sâu + phản biện Claude:** [Hướng dẫn hành động cho AI](AUDIT_AI_REPAIR_GUIDE_2026-11-08.md) là điểm vào bàn giao thống nhất, có finding **A01–A41**, tối ưu **OPT-01–08**, task E2EE **E00–E12**, regression **ER01–ER22**, vị trí mã/trigger/cách sửa/phụ thuộc và tiêu chí nghiệm thu. Bản kế hoạch này giữ vai trò bản đồ roadmap; không cần AI ghép lại chat hoặc cộng các checklist trùng. Các ID công việc cũ được giữ, không tự đóng requirement.
Baseline kiểm tra: `af2dccf406c8339100fdab183f590faea1ab0711` trên `main`, cây làm việc sạch trước khi tạo tài liệu này ở lượt đầu. Lượt audit cập nhật bắt đầu với hai tài liệu chưa tracked: bản kế hoạch này và `V0_016_COMPLETION_GUIDE.md`; không sửa mã ứng dụng đã tracked.
Phạm vi lượt này: phân tích và lập kế hoạch; không sửa mã ứng dụng, không commit/push/tag/release.

## 1. Mục tiêu và cách xác định hoàn thành

Hoàn thành toàn bộ yêu cầu đã chốt trong roadmap, có luồng hoạt động đầu cuối, tương thích với hành vi Zalo PC/Web có bằng chứng; ổn định trên Windows 11 ARM64.

“100%” là tỷ lệ yêu cầu được nghiệm thu trên một ma trận phạm vi cố định, không phải tỷ lệ file đã tạo, test đã pass hay commit đã xuất bản. Không thể cam kết phần mềm tuyệt đối không bao giờ có lỗi hoặc tương đương mọi tính năng ngoài roadmap của Zalo.

Một yêu cầu chỉ đóng khi có đủ: nguồn yêu cầu; mã thực thi trong runtime; kiểm thử đúng hành vi; bằng chứng thiết bị nếu liên quan native/server; giới hạn đã công khai. Blocker vẫn là chưa hoàn thành. Không chuyển tính năng còn thiếu thành “polish” hoặc giảm phạm vi để đạt 100%.

Các trạng thái sử dụng: **chưa làm → một phần → đã có mã → qua kiểm thử → xác nhận trên thiết bị**. Với giao thức bổ sung **E1 live / E2 static / E3 suy luận**; E3 không đủ để triển khai server contract.

Nguồn phạm vi: `docs/ROADMAP_PHASE_2.md`, `docs/ROADMAP_ZALO_PARITY.md`, các release plan v0.013–v0.016, `docs/ZALO_FEATURE_API_GAPS.md`, và `specs/001`–`012`. Các mốc 0.015.3–0.015.5 trước đây mới là đề xuất chia đợt, không tự trở thành release đã làm hoặc được nghiệm thu.

## 2. Nguồn báo cáo và kết quả kiểm chứng

Đã đọc cả hai văn bản người dùng đính kèm và artifact Antigravity `AUDIT_REPORT_AND_ACTION_GUIDE_v016.md` tại thư mục brain `a6bb41f0-f779-4244-bb7a-9a598473617e`.

Kiểm tra baseline trong lượt lập kế hoạch:

- Lượt audit cập nhật đã chạy **`cargo test --locked`: 712 passed, 0 failed, 11 ignored**; gồm lib 649 pass/10 ignored và integration 63 pass/1 ignored. Host Rust 1.99.1 **x86_64-pc-windows-msvc**, chưa phải nghiệm thu ARM64.
- `cargo clippy --all-targets --locked -- -D warnings`: pass.
- Audit E2EE chuyên sâu cùng SHA đã rerun `cargo test --locked --lib crypto::`:31 pass/0 fail; đây là tập con gồm cả transport crypto, không cộng thêm vào712 hoặc gọi là31 interop tests. Probe local ngoài repository tái hiện counter sau gap sai, skipped-key JSON fail, RAM state đổi sau bad MAC và delayed old-chain failure; xem hướng dẫn §3.1.
- Full test build có LNK4199 cho các Media Foundation delay-load DLL không có import trong test binary. `cargo fmt --all -- --check` chưa chạy được vì thiếu rustfmt; cargo-audit/cargo-machete chưa cài. Không kết luận format/dependency audit đạt.
- Đây không phải bằng chứng video phát/seek thật, upload resume trên server, E2EE interoperable hay Windows Hello trên thiết bị.
- Tag `v0.015.2` hiện trỏ tới `660d88d1575b91f6345330fb43e76acd10df2d1b`, khác baseline `af2dccf`. Không thể dùng kiểm tra HEAD để chứng nhận binary của tag. Lượt này chưa kiểm tra lại trạng thái release/assets/CI trên GitHub.

### 2.1. Phân tích và phản biện Antigravity

| Nhận định | Kết quả kiểm tra và quyết định |
|---|---|
| Gửi 1-1 làm nhánh link preview không được chạy | Đúng về nhánh `Payload::Text` ở `src/outgoing.rs:112`. Không suy rộng thành mọi đường quote/forward/link UI đều hỏng nếu chưa tái hiện. Sửa routing và thêm regression cho 1-1/group. |
| Mỗi tin thường bị tăng 200–1000 ms | Có lời gọi E2EE thừa khi thiếu session; con số latency chưa có benchmark. Phải đo request count, p50/p95 trước/sau. |
| E2EE lỗi tự gửi plaintext | Đúng, lỗi P0. Mất ACK E2EE còn có nguy cơ gửi thêm bản plaintext/trùng tin. Không fallback khi E2EE required hoặc kết quả gửi chưa rõ. |
| Upload chưa wire vì UI dùng PickAndSendAttachment | Chưa chính xác. `outgoing.rs:229` gọi `chat::send_attachment_with_client_id`; `chat.rs:2182+` đã có load/save checkpoint và skip prefix. Phần còn thiếu là chứng minh server resume, nhận diện đúng file, pause/cancel thật và UI. |
| ResumeUpload đã có backend đầy đủ | Chưa đúng. `worker.rs:1599+` đọc state rồi chỉ gửi StatusUpdate; CancelUpload xóa row chưa chứng minh dừng upload đang chạy. Không được đóng task từ tên command. |
| Video controls chỉ đổi biến | Đúng. `duration_secs=0`, play/seek/volume chưa điều khiển player. `range_request` nằm ở `src/media_download.rs:503`, có test riêng nhưng chưa nối player. |
| Doc viewer chỉ thiếu nút mở | Thiếu nhiều hơn. Render call thấy trong `gui.rs:5915+` nằm dưới module test `#[cfg(test)]`; runtime còn thiếu trigger, worker đọc/tải và GuiEvent chứa nội dung, kiểm tra quyền/ngữ cảnh. |
| Avatar bấm không tạo giao diện | Account switcher chưa nối là đúng; nhưng menu profile có renderer thật: `modals.rs:4`, gọi từ `workspace.rs:3887`. Không xóa nhầm menu hiện hữu vì placeholder cạnh nó. |
| Chỉ cần nối switcher UI là đủ | Chưa đủ. `ProfileSwitched` bỏ uid và chỉ RefreshThreads; handler logout tài khoản cũ trước khi tải phiên mới. Phải xử lý rollback, identity UI và toàn bộ task/event/cache/account epoch. |
| Windows Hello có API nên hoàn thành | Chưa đủ. Có API thật, nhưng constructor join chặn UI, cấu hình thiếu, idle cần repaint timer, auth cần owner/generation và worker. `tokio::spawn` từ render là lệch kiến trúc; chưa chứng minh luôn panic vì main có Tokio runtime. |
| Máy không Hello sẽ kẹt vĩnh viễn | Có nguy cơ không mở khóa được trong luồng hiện tại; “vĩnh viễn” chưa được chứng minh. Không tự thêm PIN yếu hay dùng mật khẩu Zalo để xác thực local. Thiết kế opt-in, kiểm tra availability, recovery đăng xuất/đăng nhập lại an toàn. |
| Cache Galley giải quyết CPU | Là giả thuyết tối ưu, chưa có profile. egui có caching riêng; không thêm Arc Galley vào model tin nhắn bền vững ngay. Đo rồi mới quyết định cache có giới hạn và key invalidation đầy đủ. |
| Upload phải dùng memmap/chunk 4 MB | Source hiện đã seek/read với buffer giới hạn, không cần memmap để đạt bounded memory. Chunk đang 1 MiB; thay sang 4–8 MB chỉ sau khi xác minh contract và giới hạn server. |
| UI indicators và manual testing là phần thiếu cuối E2EE | Không đúng. API PreKey đang E3, envelope tự định nghĩa, signatures/fingerprint chưa có vector đối chiếu. Chưa thể chứng nhận mã hóa tương thích chỉ bằng loopback Rust. |
| Làm 6 bước action guide sẽ hoàn tất toàn bộ roadmap | Chưa đủ: còn streaming decoder, PDF, upload runtime, DoH correctness, E2EE contract, VoIP/realtime nợ cũ và nghiệm thu. Các hàm ví dụ như storage::list_profiles/compute_safety_number phải kiểm tra declaration thật trước khi dùng. |

### 2.2. Phát hiện bổ sung cần đưa vào phạm vi

**Audit cập nhật mở rộng phạm vi sửa:** A03 chữ ký64→32 byte; A06 skipped-key JSON không persist; A08 concurrent ratchet update; A10 realtime receive bỏ profile decrypt; A15/A16 liveness trước pong và channel backpressure; A18 FK khi promotion message ID; A19 login-attempt race; A21 lock/render/read ordering; A25 partial-file lease; A26 executable attachment; A31 RMS sample scale; A34 LAN chỉ discovery/scaffolding; A35 verified tag chưa bind commit. Xem báo cáo chi tiết để phân biệt source-confirmed, reproduced, latent và native acceptance gap; không coi tất cả là exploit đã tái hiện.

1. **DoH chưa được xác nhận đúng:** `type=A,AAAA` không phải truy vấn A và AAAA riêng; chưa xét DNS Status/type/CNAME đầy đủ; lấy TTL lớn nhất; mọi lỗi bị negative-cache 600 s; thiếu single-flight/cache cap. Phải giữ URL hostname/SNI/certificate validation và tôn trọng proxy, không bypass proxy ngầm.
2. **Nguồn bridge bị lẫn tầng:** re-note 37 gọi IPC là WebSocket bridge và liệt kê `zmessage-e2ee-*` không có nguồn bảng kèm theo. IPC channel không phải socket command; phải xác minh từng row từ snapshot upstream, không dùng bảng suy đoán để thiết kế wire protocol.
3. **Crypto có mã nhưng chưa interop:** `crypto/prekey.rs` tự ghi schema E3; `crypto/wire.rs` tự mô tả envelope riêng. Không gọi đây là Signal/Zalo production-ready. Kiểu chữ ký, KDF label, framing/protobuf/MAC và safety fingerprint cần reference vectors.
4. **Session durability:** `crypto/session.rs` ghi ratchet trước network send; nhận PreKey lấy one-time key trước khi xác thực thành công và chưa gắn đầy đủ ID khóa từ message. Cần transaction, bounded parsing, replay/dedup và durable outbox lưu ciphertext để retry đúng.
5. **Chuyển identity:** không được tự ghi đè TOFU cũ rồi chỉ cảnh báo sau. Thiết kế pending identity và quyết định trust rõ ràng; khóa mới không được làm mất phiên cũ khi xác thực thất bại.
6. **Nợ realtime/VoIP:** có reconnect/backoff và skip polling; vẫn cần test half-open trước pong đầu tiên, auth timeout, ping write timeout, catch-up/ACK. AudioProcessor hiện là energy suppression, không phải WebRTC AEC3; software H.264 fallback và screen-share 30 fps chưa được nghiệm thu.
7. **Release hygiene:** PRODUCT_VERSION hiện v0.016-arm, Cargo 0.0.16, nhưng tag v0.015.2 trỏ snapshot khác. Cần audit snapshot/tag/artifact riêng. Script audit-prepush hiện không tự chạy test suite và kiểm tra version mới chỉ in chuỗi; không coi script pass là toàn bộ release gates đã đạt.
8. **Spec cũ còn mở:** 33 task chưa tick tại 9/12 specs. Checkbox không chứng minh mã thiếu, nhưng cũng không được tuyên bố Giai đoạn 1 hoàn tất mà chưa reconcile bằng chứng.

## 3. Kế hoạch thực thi chung: sửa nền trước, nghiệm thu từng tính năng

Mọi ticket có requirement ID, file thật, dependency, nguồn parity, test và native scenario. Không tạo roadmap thay thế để che task cũ; giữ task ID hiện hữu.

### Đợt A — Baseline, sửa regression và ngăn mất dữ liệu (ưu tiên P0)

- [ ] **SYS-01** Lập requirement-to-evidence matrix cho tất cả WP13–WP16, nhóm A/B/C trong feature gaps và specs cũ. Mỗi hàng ghi runtime caller → command → kết quả → persistence → UI; tách tag cũ và HEAD.
- [ ] **SYS-02** Sửa outbound routing bằng trạng thái encryption có nguồn xác thực, scoped account/conversation. Plain conversation không gọi PreKey; E2EE required lỗi thì fail-closed; unknown không tự probe rồi gửi plaintext. Chặn mọi payload chưa hỗ trợ E2EE trong hội thoại required, kể cả quote/forward/attachment/automation/retry/TUI.
- [ ] **SYS-03** Khôi phục link preview 1-1/group, nhưng không tự fetch plaintext URL từ hội thoại E2EE qua dịch vụ bên ngoài. Mất ACK không làm gửi lại plaintext hoặc tạo tin trùng.
- [ ] **SYS-04** Sửa checkpoint/cancel lifecycle, ownership, lỗi SQLite; không nuốt persistence failures thành success. Rà rollback khi switch account thất bại.
- [ ] **SYS-05** Chuẩn hóa trạng thái release/changelog/version, loại fake completion, giữ worktrees ngoài index và thêm check gitlink trái phép trước release. Không force-move tag đã công bố trong quy trình mới.

**Nghiệm thu:** test routing plain/E2EE/unknown/group/link; inject lỗi key/MAC/network/ACK; không request PreKey cho plain text và không plaintext request khi E2EE required. App cũ/data cũ mở được, không làm mất lịch sử.

### Đợt B — Hoàn thiện v0.015.x đầu cuối

#### MEDIA-01 / WP15-1: Gallery

- [ ] Zoom/pan/rotate 90°, thống nhất zoom hiển thị theo ảnh thật, arrows/Esc/Ctrl+C/Ctrl+S, album position/count có nguồn DB và phân trang ổn định.
- [ ] Giữ quyền hidden/protected/recalled/expired khi save/copy/nav; khóa ownership theo account + message + generation cho mọi event.
- [ ] Chốt xung đột phạm vi: release plan cho modal hoặc popup, Phase 2 ghi cửa sổ độc lập. Để thỏa cả hai, triển khai cửa sổ viewer độc lập với lifecycle dọn dẹp rõ, có modal fallback khi native không khả dụng.
- [ ] Counter chính xác trên tập media xác định; thumbnail strip chỉ là bổ sung nếu được chốt, không phải yêu cầu chắc chắn từ mọi tài liệu.

**Nghiệm thu:** nhiều ảnh, nhiều cửa sổ, phóng/xoay/di chuyển, reload lịch sử và đổi tài khoản; keyboard hoạt động, không dùng counter giả, không vượt quyền hoặc giữ ảnh sau thu hồi.

#### MEDIA-02 / WP15-2: Player và progressive streaming

- [ ] Nối backend Media Foundation hiện hữu với command/event thật: open/buffering/playing/paused/seek/ended/error/position/duration/volume/mute/fullscreen. Không dùng biến boolean thay cho playback.
- [ ] Có adapter seekable byte stream hoặc pipeline native tương đương sử dụng range download đã validate; hủy request seek cũ, bounded cache và decode ngoài UI, đồng bộ audio/video.
- [ ] Kiểm tra 206/Content-Range, 200 khi server bỏ qua Range, 416, đổi ETag/If-Range, renew URL, redirect và quyền source; không trộn byte hai phiên bản tài nguyên.
- [ ] Đạt scope đã chọn “controls + native shell backend” trước, nhưng ghi rõ chưa đóng toàn bộ WP15-2 cho tới progressive seek hoạt động. Không gắn slider không điều khiển được player.

**Nghiệm thu:** video 100–200 MB không chờ tải toàn bộ; capture số byte/Range requests khi seek; áp dụng mục tiêu roadmap phát ~1 s và seek <1.5 s trên thiết bị/mạng/dataset được ghi rõ. Máy thiếu codec báo lỗi/có fallback thật, không pass giả.

#### MEDIA-03 / WP15-3: Document preview

- [ ] BubbleAction preview → WorkerCmd tải/đọc bounded file → GuiEvent chứa nội dung/metadata đúng owner/generation → App state → renderer runtime; bỏ render chỉ nằm trong test.
- [ ] UTF-8/text/code/JSON/Markdown/log: line numbers, syntax highlighting giới hạn, tìm kiếm/highlight/jump/next/previous; virtualization hoặc giới hạn kích thước để không treo UI.
- [ ] PDF native renderer phù hợp ARM64, dependency/license review trước khi chọn; render từng trang ngoài UI, cache texture có budget; tìm text khi tài liệu có text layer, xử lý password/corrupt/large PDF.
- [ ] Word/Excel: làm rõ hiện chỉ xuất hiện trong bối cảnh roadmap, chưa có WP chi tiết. Giữ mở ngoài đúng nhãn; nếu ma trận parity chốt preview Office thì bổ sung requirement/renderer/test riêng, không âm thầm tính đã hỗ trợ.

**Nghiệm thu:** bấm Xem trước mở được thật; tải/cancel/lỗi/đổi account/revoke; text lớn, search có jump; PDF nhiều trang không nạp toàn bộ bitmap; không mở file tùy ý ngoài cache do app quản lý.

#### TRANSFER-01 / WP15-4: Upload resume thật

- [ ] Dùng một transfer job duy nhất cho đường đính kèm hiện hữu; không tạo StartResumableUpload song song tách khỏi outgoing pipeline.
- [ ] Persist account/thread/client_id + source identity/checksum/file stamp + layout + server token/TTL nếu có bằng chứng + final response/fileId + trạng thái gửi tin. Checkpoint chỉ sau ACK hợp lệ.
- [ ] Restart/resume xác minh file không đổi kể cả cùng tên/cùng size; dựa vào contract server về retention/idempotency. Không chứng minh được resume server thì giữ blocker, không giả định clientId đủ.
- [ ] Pause/Resume/Cancel/progress có token dừng request thật, state machine và UI; Cancel không chỉ xóa DB. Phục hồi pending jobs khi startup/reconnect, tránh auto-retry vô hạn.
- [ ] Sau chunk cuối nhưng trước file_done/send ACK, giữ state để reconcile; chỉ cleanup khi message acceptance đã chắc chắn. Đọc file bằng bounded buffers hiện hữu; chunk size theo bằng chứng server, không đổi chỉ để khớp con số roadmap.

**Nghiệm thu:** 500 MB bị ngắt ở 50%, restart app, resume tiếp; đối chiếu byte thực tải lên. Thử file đổi cùng size, mất ACK cuối, token hết hạn, disk full, đổi tài khoản, cancel giữa request; không tin trùng/không file sai.

#### NET-01 / DoH và REF-01 / Olaz reference

- [ ] DoH truy vấn A/AAAA riêng, kiểm tra HTTP/DNS Status/RR type/CNAME; TTL theo record hợp lệ và remaining TTL, không kéo dài cache stale bằng clamp; NXDOMAIN và transient error phân biệt.
- [ ] Cache bounded, concurrent single-flight/cancellation cleanup, retry có tổng deadline; fallback chỉ host CDN hợp lệ, chặn kết quả private/loopback/link-local không được phép; tôn trọng cấu hình proxy.
- [ ] Giữ hostname/SNI và xác thực chứng chỉ chuẩn qua DNS override; không tắt TLS validation, không hardcode IP CDN; redirect revalidate mỗi hop. Kiểm tra download thường và Range dùng cùng chính sách.
- [ ] Pin commit/checksum/source path/version cho bridge-table Olaz; xác minh row/channel/direction và phân biệt IPC với WS/API. Chỉ dẫn mapping có nguồn, xóa claim E2EE IPC không có bằng chứng.

**Nghiệm thu:** fixture DNS lỗi/TTL/CNAME/AAAA/redirect/proxy, integration fallback trên mạng thử nghiệm; mọi row mapping truy được nguồn. Olaz/libsignal-protocol-javascript is GPL-3.0 and used **for reference/algorithm study only — no code copied**. ZaloLite E2EE implementations are written in Rust from the Standard Signal Protocol spec. Không vendor nguyên artifact GPL vào bản permissive khi chưa xem xét license.

### Đợt C — Chốt giao thức E2EE và hoàn thành v0.016

#### SEC-01 / WP16-1: PreKey contract và lifecycle

- [ ] Đọc bundle official đúng version và thu traffic trên tài khoản được phép: đăng ký thiết bị/khóa, fetch peer bundle, tin đầu, reply, offline, reset. Khử secrets và giữ hash/version nguồn.
- [ ] Xác minh endpoint/method/params/encoding/device addressing, chữ ký và khóa identity. Không coi Ed25519 identity sinh độc lập + X25519 là mặc định tương thích; xác định XEdDSA/biến thể bằng bằng chứng và vector.
- [ ] Chốt lifecycle: batch/quota thực tế, upload ACK, refill, unique key IDs, rotation và giữ signed prekey cũ cho message đang đến; message phải tham chiếu đúng ID, không lấy ngẫu nhiên prekey đầu pool.
- [ ] Không auto-upload key với contract E3. Đăng ký/refill thất bại không được nuốt rồi gửi ciphertext như đã sẵn sàng.

#### SEC-02 / WP16-2: Ratchet, wire format và durable delivery

- [ ] Thay wire envelope tự chế bằng format official đã xác minh; đối chiếu KDF labels, key prefix/length, signatures, serialization, MAC/associated data và cipher. Giải quyết roadmap ghi AES-GCM nhưng prototype CBC/HMAC bằng ADR có bằng chứng, không chọn cipher để khớp tài liệu sai.
- [ ] Serialize thao tác theo account/peer/device; transaction cập nhật ratchet + consumed prekey + inbox dedup hoặc outbox ciphertext tương ứng. Restart/mất ACK retry ciphertext đã lưu, không encrypt lại tạo trạng thái không khớp.
- [ ] Không commit state/trust hay tiêu thụ one-time key khi auth thất bại; reject non-contributory DH, malformed lengths, replay, counter overflow; cap tổng skipped keys và expiry, không chỉ cap từng bước.
- [ ] Identity change: giữ trusted identity cũ, pending key có provenance, chặn theo policy và cho verify; safety number đúng scheme/version/identifiers của Zalo, không chỉ test đối xứng 60 chữ số.
- [ ] Chạy encrypt/decrypt ngoài UI; audit DPAPI context, isolation, zeroize, log redaction và migration/backup. Review mật mã độc lập trước stable release.

#### SEC-03 / Payload coverage và E2EE UI

- [ ] Text, ảnh, tệp, captions, quote, forward, receipts/reactions/lifecycle có policy rõ theo capability official. Unsupported required-E2EE từ chối an toàn; không hạ plaintext.
- [ ] Group E2EE giữ trong phạm vi người dùng đã chốt: nghiên cứu distribution/rotation/member join-leave/multi-device; không mặc định Sender Keys khi chưa có bằng chứng. Nếu chưa interop thì chưa đạt mục tiêu full.
- [ ] Nối safety dialog thật từ header qua worker, fingerprint nguồn đúng session/account; banner identity change + verify action có persistence. Badge phân biệt required/pending/encrypted/decrypt-failed; không gắn xanh cho fallback error text.
- [ ] Tất cả receive paths: WS, polling fallback, preview/history/import theo source hợp lệ; ngăn duplicate decrypt làm advance ratchet hai lần. Cache plaintext đã xử lý phải tôn trọng privacy/lifecycle.

**Nghiệm thu SEC-01–03:** vector chéo với implementation tham chiếu, sau đó fork ↔ Zalo PC/Mobile chính thức với tài khoản thử nghiệm. Text/ảnh/file hai chiều, offline/out-of-order/duplicate/restart/mất ACK/rotate/reset identity/group membership. Ghi build/OS/client official, bước thử, kết quả, giới hạn; không tuyên bố “server không đọc được” chỉ từ HTTP capture hoặc test loopback.

#### ACCOUNT-01 / WP16-3: Multi-account đầu cuối

- [ ] Account registry load/save qua worker, atomic/private storage, validate path và UID; không cho tên profile từ JSON trỏ tùy ý. Register profile khi login, remove/logout semantics rõ.
- [ ] Nối AccountSwitcher vào menu/sidebar thật, add/login/select/remove/error/loading. Payload trả selected identity/profile, không chỉ toast.
- [ ] Chuẩn bị/validate session đích trước; thất bại giữ tài khoản cũ. Khi commit switch, hủy/quarantine old socket/HTTP/media/call/timers, flush state đúng owner, gắn generation vào mọi event kể cả auth/upload.
- [ ] Reset UI identity/name/avatar, thread selection, search/draft, hidden/PIN gate, notifications, clipboard/cache/viewer, E2EE session manager. Kiểm tra settings/paths không còn root dùng chung sai account.

**Nghiệm thu:** A→B→A giữ phiên không QR lại; cold/warm tách riêng, mục tiêu warm switch <0.5 s; response A đến muộn không xuất hiện ở B; switch khi đang upload/call/sync, session đích corrupt/expired, restart giữa switch; không mất dữ liệu A khi rollback.

#### LOCK-01 / WP16-4: Windows Hello vận hành an toàn

- [ ] Cài đặt opt-in/disable/timeout và kiểm tra availability bất đồng bộ; không join trên UI. Confirm capability trước bật.
- [ ] UI→WorkerCmd→WinRT desktop/HWND integration phù hợp→GuiEvent owner/generation→lock state. Chỉ Verified unlock; cancel/unavailable/timeout vẫn khóa, kết quả auth cũ không mở account mới.
- [ ] Idle timer đánh thức render khi hết hạn dù app reactive/không có input; kiểm tra lock/unlock Windows, tray, suspend/resume. Overlay che cả viewer/native windows và notification content.
- [ ] Recovery logout về màn login và clear UI/cache theo policy, không tiết lộ dữ liệu cũ hoặc coi quét QR bất kỳ account là mở khóa account cũ. Không tự tạo PIN/password local chưa thiết kế lưu/hash/rate-limit.

**Nghiệm thu:** Windows Hello available/not-configured/cancel/failure, lock idle thực, focus/tray/resume, account switch trong auth, recovery và restart. Có API compile không thay thế native tests.

### Đợt D — Đóng nợ hệ thống rộng hơn và parity roadmap

- [ ] **REALTIME-01 / WP13-1..4:** test auth/heartbeat deadlines trước và sau pong đầu tiên, bounded ping write, reconnect rotation/backoff/jitter, stale epoch, replay/offline catch-up, commit cursor sau persist; polling fallback có điều kiện readiness/coverage đúng. Battery/background heartbeat chỉ thay theo contract. Đo latency <200 ms trong môi trường chuẩn; mục tiêu reconnect <3 s khi có network-restored signal, tách khỏi backoff 30 s khi chưa biết mạng phục hồi.
- [ ] **VOIP-01 / WP14-1..3:** tích hợp APM/AEC với waveform render reference, NS/AGC có capture path và double-talk test; energy gate hiện tại không tính AEC3. Backend H.264 software thật khi MFT không khả dụng; screen/window capture 30 fps, adaptation/CPU/battery đo trên ARM64. Native ignored tests phải chạy trên thiết bị hỗ trợ, không bỏ ignore rồi giả pass headless.
- [ ] **LEGACY-01 / specs 001..012:** reconcile từng requirement và checkbox cũ; nhóm/quyền/community; revoke/expiry/protected-media/reactions; V1/BinNet/history/import rollback; DPI/theme/emoji/group lớn; customization; stranger/privacy; call multi-device; account/group lifecycle; rich messaging. Spec 010–012 tick đầy đủ vẫn cần spot-check runtime/verification, không dùng checkbox làm bằng chứng duy nhất.

Số task đang mở tại baseline: spec001=6, 002=5, 003=5, 004=6, 005=6, 006=1, 007=2, 008=1, 009=1; spec010–012=0. Tổng **33 chưa tick**, không tự kết luận 33 lỗi/missing features.

- [ ] **PERF-01:** profile UI/layout/DB/network trên workload thực rồi sửa điểm nghẽn. Cache layout nếu có lợi, key theo message revision/width/DPI/theme/font/language/mentions; budget/invalidation và tránh đưa render cache vào serialized ChatMessage.
- [ ] **QUALITY-01:** quét stubs/allow(dead_code)/unused payload, nhưng review caller runtime thật. Static regex audit hiện chỉ cảnh báo một nhóm lỗi, không chứng nhận No UI Stubs. Bổ sung integration regression cho mọi lỗi P0/P1.

## 4. Thứ tự, phụ thuộc và checkpoint phát hành

1. Đợt A trước: khóa regression gửi tin và bảo vệ dữ liệu.
2. Đợt B là ưu tiên hoàn thiện 0.015.x; SEC-01 research được chạy song song bởi người riêng nhưng không nối E3 vào stable send.
3. ACCOUNT/LOCK và SEC-02 triển khai khi hợp đồng tương ứng rõ. **SEC-03 UI state/event/loading/error/identity-warning có thể làm song song ngay với SEC-01**, tận dụng dialog hiện hữu; fingerprint official và badge successful-encryption chỉ dùng sau xác thực trạng thái/contract, không quảng bá verified trước interop.
4. Đợt D bắt đầu inventory ngay từ SYS-01, sửa và nghiệm thu song song theo module; không chờ full E2EE mới kiểm tra VoIP/specs cũ.
5. Release gate cuối chỉ mở khi tất cả yêu cầu trong ma trận đạt. Không phát hành full 0.016 từ “foundation complete”.

Đề xuất gói giao hàng (tên logic, số tag chốt sau audit release): **hotfix hệ thống → 0.015.x hoàn thiện media/transfer → 0.016 security/account/lock → system acceptance**. Nếu vẫn dùng 0.015.3–0.015.5 thì đó là số bản mới cho các gói thực sự hoàn thành; không tái dùng tag 0.015.2 hay đưa tính năng 0.016 vào hotfix rồi gọi là tương đương bản cũ.

Không chốt ETA 1–3 ngày cho toàn bộ hệ thống. Sau SYS-01 và E2EE protocol spike mới ước lượng theo module, nguồn lực và tình trạng tài khoản/thiết bị; blocker server/key/entitlement không giải quyết bằng tăng lượng code.

Hiệu chỉnh đề xuất Claude: timebox SEC-01 spike 1–2 ngày công để xuất evidence/unknowns và đánh giá khả thi, không cam kết phải có interop sau timebox. Ước tính “5–8 ngày” chỉ có thể là giả thuyết cho tập con đã chốt, không phải ETA full roadmap gồm media/group/native. Phân mức severity riêng với release scope: link-preview routing là regression P1, downgrade là ưu tiên P0; WP15/DoH chưa đủ không được đẩy thành optional polish để báo100%.

## 4.1. Chỉ dẫn bàn giao AI thực thi sau audit

- Đọc `AUDIT_AI_REPAIR_GUIDE_2026-11-08.md` §2 để không lặp nhận định sai từ guide Claude: upload backend tồn tại, badge đã có render nhưng thiếu tính đúng, signature verification hiện fail-closed ở engine và bị fallback ở outgoing, switcher/Hello chưa verified.
- SYS-02/SYS-03 → A01/A02; SEC-01..03 → A03..A14 và A37..A41; REALTIME-01 → A15..A17; SYS-04/LEGACY-01 → A18/A19/A33/A34; ACCOUNT-01/LOCK-01 → A20..A22; NET-01/TRANSFER-01/MEDIA-01..03 → A23..A30; VOIP-01 → A31/A32; SYS-05/QUALITY-01/REF-01 → A35/A36.
- Dùng bảng hành động **E00..E12** và **ER01..ER22** tại hướng dẫn §3.2–3.3 để giao việc E2EE. Sửa correctness nội bộ làm song song nghiên cứu contract; không chờ capture mới sửa counter/rollback/serialization, không dùng local tests thay interop. A38 phân biệt mutation RAM với persistence DB; A39 không tự coi missing AD là exploit đã chứng minh; A40 phân biệt row-isolation với file-isolation.
- PERF-01 → OPT-01..08; dọn scaffolding theo §8 báo cáo sau khi có đường runtime thay thế, không xóa chức năng roadmap hoặc public API chỉ vì rg không thấy caller nội bộ.
- Mỗi task phải có trigger, scope nhỏ nhất, nguồn parity, regression fail-trước/pass-sau, migration/owner/cancel/idempotency, check thực chạy và native gap. Giữ task IDs cũ, không mass-tick specs hoặc sửa code như side effect của việc cập nhật báo cáo.

## 5. Ma trận nghiệm thu và điều kiện xuất bản

| Tầng | Bằng chứng bắt buộc |
|---|---|
| Chất lượng mã | cargo check --locked; strict clippy; full cargo test --locked; format touched code; test regression đúng bug; dependency/license audit và security exceptions có owner/decision. |
| Tích hợp | UI action đi đủ WorkerCmd → backend/DB → GuiEvent payload → App state → render trong production; cancellation/errors/late events cũng được xử lý. |
| Giao thức | Fixture/vector lấy từ nguồn độc lập và cross-client; schema/endpoint/cipher không dựa route index hoặc unit test tự sinh. |
| Native ARM64 | GUI smoke, light/dark, DPI, keyboard/IME, media playback/PDF, microphone/speaker/headphones, thiếu MFT, Hello và recovery. |
| Resilience | Mất mạng/DNS/proxy, sleep/resume, crash/restart, ACK mất/trùng/đảo thứ tự, disk full/locked, corruption, account switch giữa tác vụ. |
| Hiệu năng | p50/p95 latency, request count, memory/file-size scaling, render CPU/idle wakeups, gallery/video startup/seek, warm account switch. Ghi dataset và điều kiện đo. |
| Phát hành | 4-point versions khớp exact tag, CHANGELOG đúng tính năng runtime, không gitlink orphan/secrets, CI ARM64 đúng SHA xanh, assets/checksum/version verified; Git tag riêng không đồng nghĩa published Release. |

Mỗi test thiết bị ghi build SHA, platform, official-client version, scenario, expected/actual, pass/fail và bằng chứng đã khử dữ liệu riêng tư. Các bài ignored ghi lý do và nơi chạy; không tính chúng là pass.

## 6. Các điều kiện còn phụ thuộc bên ngoài

- E2EE cần client official đúng phiên bản, tài khoản test có capability/entitlement và thiết bị peer để handshake/media/group. Nếu chưa có thì làm fixture/design/hardening độc lập, giữ trạng thái interop blocked.
- Windows Hello, VoIP, MFT/software fallback và hiệu năng cần máy Windows ARM64 đúng khả năng; GitHub Actions không thay thế sinh trắc học hoặc âm thanh thật.
- Tiêu chí tương tự Zalo gốc áp dụng theo ma trận tính năng/capability đã chốt; không hứa quyền server hoặc tính năng ngoài phạm vi. Không có bằng chứng thì ghi Unknown/AMBIGUOUS, không báo 100%.

## 7. Tài liệu và nguồn đối chiếu

- Antigravity audit ngày 08/11/2026: nguồn phát hiện; các kết luận được chấp nhận/hiệu chỉnh tại §2, không mặc định là bằng chứng nghiệm thu.
- `docs/ROADMAP_PHASE_2.md`, `docs/ROADMAP_ZALO_PARITY.md`, `docs/releases/v0.015-arm-plan.md`, `docs/releases/v0.016-arm-plan.md`, `specs/*/{spec,plan,tasks,verification}.md`.
- `docs/ZALO_REFERENCE_ANALYSIS.md`, `docs/AI_WIRING_DISCIPLINE.md`, `docs/SPEC_KIT.md`, `docs/standards/security-and-cryptography.md`.
- Google DoH JSON API: https://developers.google.com/speed/public-dns/docs/doh/json (type là một RR type; A và AAAA truy vấn riêng).
- Signal signature formats: https://signal.org/docs/specifications/xeddsa/ (tham chiếu khái niệm, không chứng minh scheme Zalo).
- Signal Double Ratchet: https://signal.org/docs/specifications/doubleratchet/ (đối chiếu state/error semantics; dùng đúng revision của implementation target, không tự nâng protocol Zalo theo spec mới).
