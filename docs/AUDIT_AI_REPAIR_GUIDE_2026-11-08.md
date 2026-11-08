# Hướng dẫn hành động hợp nhất: audit hệ thống và E2EE cho AI tiếp nhận

Ngày kiểm tra: **08/11/2026, Asia/Saigon**. Baseline: **`af2dccf406c8339100fdab183f590faea1ab0711`**, nhánh `main`.

Đây là **báo cáo và kế hoạch sửa**, không phải bản vá hoặc chứng nhận phát hành. Không sửa mã ứng dụng, test, manifest hay workflow trong lượt audit này. Hai tài liệu chưa tracked đã có khi bắt đầu là `PLAN_SYSTEM_COMPLETION_2026-11-08.md` và `V0_016_COMPLETION_GUIDE.md`; không coi chúng là nội dung của commit baseline.

**Đây là điểm vào bàn giao thống nhất cho AI thực thi:** đã gộp audit hệ thống, phản biện Claude/Antigravity và audit E2EE chuyên sâu. Bắt đầu bằng §3.1 (finding mới), §3.2 (bảng hành động E2EE), rồi §9 (kế hoạch chung và nghiệm thu). [Kế hoạch tổng thể](PLAN_SYSTEM_COMPLETION_2026-11-08.md) là bản đồ roadmap tham chiếu; AI không cần ghép lại các câu trả lời trong chat. Các ID `SYS/SEC/MEDIA/TRANSFER/NET/ACCOUNT/LOCK/REALTIME/VOIP/LEGACY/PERF/QUALITY` được giữ nguyên; ID `Axx` là finding, `E00..E12` là task thực thi E2EE, không phải requirement mới hoặc phần trăm hoàn thành.

Roadmap/spec/ADR và bằng chứng gốc vẫn là nguồn phạm vi/quy tắc. Khi guide Claude hoặc báo cáo cũ mâu thuẫn trạng thái với bằng chứng trong tài liệu này, dùng kết luận đã kiểm chứng ở đây và ghi discrepancy; không cộng hai bộ checklist như hai tính năng riêng. Không lấy tài liệu này thay cho đọc code và official evidence trước sửa.

## 1. Kết luận và phạm vi kiểm tra

**Không phát hiện lỗi biên dịch trong bộ kiểm tra đã chạy, nhưng phát hiện nhiều lỗi logic, bảo mật, tích hợp và nghiệm thu. Không đủ cơ sở gọi v0.015.x hoặc v0.016 là hoàn thành toàn bộ roadmap.** E2EE có lỗi nội bộ xác định được ngay cả trước khi xét khả năng tương thích Zalo. Media/transfer/Hello/multi-account có phần chỉ tồn tại dưới dạng scaffolding hoặc trạng thái UI.

Đã inventory **158 file Rust trong `src/`**; rà sâu các đường gửi/nhận, crypto, session/diagnostics, socket/queue, SQLite/lifecycle/search, upload/download/cache, GUI/worker/account/lock, video/audio/call và release workflows. Kiểm tra chéo roadmap Phase 2, release plans và `specs/001..012`; spot-check thêm Business/LAN/2FA/device/poll/todo/zCloud. Không tuyên bố đã chứng minh mọi dòng, mọi native branch, mọi đường TUI hoặc toàn bộ thư viện vendor là không lỗi.

Không có `graphify-out/graph.json` hoàn chỉnh; chỉ có sidecar/cache từ lần trích xuất trước. Vì vậy audit dùng đọc mã/caller trực tiếp, không dùng đồ thị chưa hoàn chỉnh làm chứng cứ và không dựng lại đồ thị để thay đổi phạm vi.

### 1.1. Kiểm tra đã thực sự chạy

| Kiểm tra | Kết quả và giới hạn |
|---|---|
| `cargo test --locked` | **712 passed, 0 failed, 11 ignored**. Trong đó lib 649 pass/10 ignored; integration 63 pass/1 ignored; binary/doc-tests không có test. |
| `cargo clippy --all-targets --locked -- -D warnings` | Pass. Đây không phải chứng nhận logic/giao thức. |
| `cargo test --locked --lib -- --ignored --list` | Liệt kê 10 bài lib bị bỏ qua; không chạy chúng. |
| `rustc -vV` | Rust **1.99.1**, host **x86_64-pc-windows-msvc**. Không phải build/nghiệm thu ARM64 native. |
| Link lúc chạy full tests | Có cảnh báo **LNK4199** cho `/DELAYLOAD:mfplat.dll`, `mfreadwrite.dll`, `mf.dll`, `mfplay.dll` vì không có import tương ứng trong test binary. Không gán thành compiler error, cũng không báo toàn bộ công cụ zero-warning. |
| `cargo fmt --all -- --check` | Không chạy được: toolchain thiếu `cargo-fmt`/rustfmt. Chưa kết luận format đạt hay lỗi. |
| `cargo audit`, `cargo machete` | Công cụ chưa cài. Chưa kiểm chứng advisory/dependency unused; không suy ra dependency an toàn hoặc chắc chắn dư. |
| Git index | Không thấy gitlink mode `160000` hiện tại; không tái báo lỗi orphan worktree đã sửa. |
| Probe không dùng tài khoản | Sub-audit DoH: `type=A,AAAA` trả HTTP 400 ở ba endpoint cấu hình; Google A/AAAA riêng trả 200. SQL in-memory: lỗi FK khi promotion ID có row phụ thuộc; đo VM steps lịch sử trên SQLite Python 3.50.4, chưa phải bundled rusqlite. |
| Audit E2EE chuyên sâu sau đó, cùng SHA | Chạy lại `cargo test --locked --lib crypto::`: **31 passed, 0 failed, 0 ignored**, 628 filtered out. Bao gồm cả AES/MD5 transport; không gọi tất cả31 bài là Signal interop tests. Đây là rerun một tập con, không cộng31 vào712. |
| Probe ratchet qua library build hiện tại | Harness đặt trong thư mục tạm ngoài repository, dữ liệu giả và state trong RAM: nhận n=2 rồi n=3 lỗi; skipped-key JSON lỗi; state thay đổi sau bad MAC; old-chain delayed message thất bại sau DH transition. Output và giới hạn ở §3.1. Không sửa test/source trong repository. |

Các ignored test lib gồm live call, queue/history/image/live-update benchmarks, official phone-codec interoperability, phone import performance, indexed search 632k rows, gallery navigation benchmark, native H.264. Integration ignored: controlled media retry benchmark. Không chạy `--ignored` toàn cục vì một số bài cần thiết bị/capture/live setup.

**Chưa chạy:** GUI tương tác/native Hello, âm thanh/hình ảnh thật, E2EE fork ↔ client official, upload resume server, sleep/resume, ARM64 performance, remote CI/release asset provenance. Không đọc session/DB/cookie/capture riêng tư của người dùng. Các regression dưới đây là **yêu cầu cần viết**, không phải các test đã pass trong lần audit.

### 1.2. Cách đọc mức độ

- **P0:** ưu tiên sửa ngay trước phát hành tiếp, do nguy cơ mất tính bí mật hoặc sai delivery quan trọng. Đây là mức ưu tiên dự án, không phải CVSS hay bằng chứng đã có tấn công thực tế.
- **P1:** sai chức năng/bảo mật/dữ liệu đáng kể; chặn release chứa tính năng bị ảnh hưởng.
- **P2:** robustness, hiệu năng, tài liệu hoặc phần nghiệm thu còn thiếu. **P2 không có nghĩa được bỏ khỏi mục tiêu 100%.**
- **S:** xác nhận từ source/caller; **R:** có probe tổng hợp hoặc public-endpoint reproduction; **G:** khoảng trống nghiệm thu/contract; **L:** defect tiềm ẩn trong code chưa nối runtime. Không biến L thành sự cố người dùng đã tái hiện.

## 2. Phân tích và phản biện phản hồi Claude

| Nội dung Claude | Đánh giá và quyết định |
|---|---|
| Upload backend có checkpoint | **Đúng.** Kế hoạch Codex trước tại §2.1/TRANSFER-01 đã ghi điều này. Nhận xét rằng bản kế hoạch ấy vẫn kết luận “backend hoàn toàn chưa wire” là không phản ánh nội dung hiện tại; đó là sai sót của các báo cáo cũ hơn. Giữ backend hiện hữu, sửa finalize/resume/cancel/contract. |
| Doc viewer thiếu cả worker pipeline | **Đúng.** Thiếu action, bounded read/download, result event, state và production renderer; không chỉ thêm nút. |
| Có thể làm E2EE UI song song với protocol capture | **Đồng ý.** UI loading/error/policy/pending-identity và contract event làm song song được. Fingerprint chính thức, badge “đã mã hóa”, verify action phải dựa trạng thái xác thực thật; không có nút giả hoặc fingerprint tự chế được quảng bá đã tương thích. Kế hoạch được làm rõ, không bắt mọi UI chờ interop. |
| 31 unit tests nghĩa là foundation đủ tốt để tích hợp | **Chỉ đúng ở mức tiếp tục nghiên cứu.** Có mã nền, nhưng parser cắt chữ ký 64→32 byte, skipped-key JSON không persist được, nhận WS bỏ qua decrypt, OTP selection/rotation và concurrent state còn sai. Không được dùng 31 roundtrip tests để chứng nhận engine đúng hoặc production-ready. |
| Guide nói verify signature có thể bị bỏ qua | **Sai với code hiện tại:** `crypto/session.rs:330` trả false khi key/signature không hợp lệ và caller abort; chỗ downgrade là `outgoing.rs:115`. Lỗi cần sửa thêm là parser/signing-identity contract, không sửa bằng cách bỏ kiểm tra chữ ký. |
| P0 link preview, P2 toàn bộ media/DoH | Cần tách severity và release scope. Link routing là regression chức năng P1, không tự động P0 bảo mật. DoH proxy bypass, executable attachment, cache partial và lỗi preview contract không thể gộp là “polish”. WP15 thiếu vẫn chặn tuyên bố hoàn thiện 0.015.x. |
| Ước 5–8 ngày hoàn thành v0.016 | Chưa có căn cứ cho **toàn bộ mục tiêu**. Capture/entitlement, group/media E2EE, native Hello, retry/crash và WP15/WP14 có critical path riêng. Dùng 5–8 ngày nếu muốn như giả thuyết cho một tập con đã giới hạn, không là ETA full release. Timebox protocol spike rồi ước lượng lại theo output. |
| WP16-3/4 “Complete / Verified” trong guide | Không được chấp nhận: switcher chưa có caller, lock còn lỗi thực thi và không có record native acceptance. Status “70%” cũng thiếu mẫu số requirement. |

**Các chỉ dẫn nguy hiểm/không chính xác khác trong `V0_016_COMPLETION_GUIDE.md` cần AI sau hiệu chỉnh:**

1. Không chạy `SELECT * FROM identity_key` để chia sẻ debug; không xuất private blobs, kể cả DPAPI-protected. Chỉ schema, count, key ID và metadata đã khử nhạy cảm.
2. Không xóa `e2ee.db` thật để test reset. Dùng profile/tài khoản test riêng, snapshot có kiểm soát và test peer reset đúng chiều; reset local không tự chứng minh local nhận `identity_changed` từ peer.
3. Không khẳng định spec Signal “public domain” hoặc mọi đoạn mã có thể copy; kiểm tra license tài liệu/mã cụ thể. Tham khảo spec khác với cấp phép implementation.
4. Không suy ra “server không đọc được” hoặc “không metadata” chỉ vì request có base64 ciphertext. Metadata routing vẫn có thể tồn tại; cần đánh giá end-to-end key ownership và payload coverage.
5. Không mặc định Group = SenderKey, device_id=1 là official, hay hoãn group sang v0.017 để đạt 100% trái scope đã chốt.
6. Không tạo `SafetyNumberModal` thứ hai: đã có `gui/safety_number_dialog.rs`; hoàn thiện hoặc thay thế có kế hoạch, tránh renderer trùng.
7. Không thêm generic retry cho mutation/E2EE. Retry phải theo trạng thái delivery/idempotency và ciphertext đã persist.
8. `crypto::session::tests` không đồng nghĩa toàn bộ 31 crypto tests; ghi chính xác lệnh, số bài, phạm vi và dữ liệu test.

## 3. Finding bảo mật và E2EE — liên kết SYS-02, SEC-01..03

### A01 — P0 / S: Tự hạ E2EE về plaintext và nguy cơ gửi trùng

**Vị trí:** `src/outgoing.rs:115`, `src/chat.rs:8297`; payload quote/file tại `outgoing.rs:176`, `:229`.

Mọi `Payload::Text` 1-1 thử E2EE, bất kỳ lỗi nào cũng gửi endpoint thường, không dựa policy required/plain/unknown. Signature/DB fail làm hạ cấp; timeout sau khi request đã đến có thể gây gửi lần hai. Quote/attachment không có enforcement chung.

**Sửa:** dispatcher dựa capability/policy có provenance; required-E2EE phải fail closed cho mọi payload/entrypoint (GUI, worker, TUI, scheduler, retry, forward/quote/media). Giữ đường tin thường khi policy xác nhận thường; không gửi thử để dò capability. Chưa có contract E2EE thì chặn tính năng required-E2EE rõ ràng, không hạ plaintext.

**Regression:** lỗi signature, key DB, network trước/sau ACK, unsupported encrypted media đều **0 request thường**; retry giữ client ID và envelope; pending/failed không được hiển thị encrypted-success.

### A02 — P1 / S: Nhánh link preview 1-1 không được chạy

**Vị trí:** `src/outgoing.rs:112`. `if !job.is_group` đứng trước `else if extract_first_url`, nên link text 1-1 không qua link path. Không suy rộng mọi forward/quote là lỗi giống nhau.

**Sửa:** phân policy rồi payload, không dùng thử E2EE như nhánh mặc định. Preview của required-E2EE không được lộ URL qua dịch vụ preview hoặc fetch bên thứ ba ngoài policy đã xác minh.

**Regression:** ma trận plain/E2EE/unknown × direct/group × text/link/quote/file; assert endpoint, request count và privacy, không chỉ UI title.

### A03 — P1 / S: Chữ ký bị cắt 64 xuống 32 byte

**Vị trí:** `src/crypto/prekey.rs:108`, `:160`; `src/crypto/session.rs:337`.

`decode_key(value, 64)` kiểm tra input64 nhưng luôn trả `[u8;32]`, rồi `signature.to_vec()` chỉ32. Verifier yêu cầu64 nên bundle qua production parser không verify được, dù test tạo RemoteBundle trực tiếp có thể pass.

**Sửa:** decoder exact-length theo kiểu/const generic cho key32 và signature64, không cắt. Encoding theo contract; không nới verifier để làm test xanh.

**Regression:** fixture ký hợp lệ → serialize/parse production → verify thành công; signature31/32/63/65 và tamper phải reject.

### A04 — P1 / S+G: Bundle tự xuất thiếu signing identity và chưa chứng minh binding

**Vị trí:** `crypto/keystore.rs:34`, `:100`; `crypto/prekey.rs:23`; `crypto/session.rs:333`.

Identity DH và Ed25519 sinh độc lập; bundle/upload không mang signing identity dùng ký. Verifier fallback coi DH bytes như Ed25519 public key. Đây là inconsistency nội bộ; thêm signing key tùy ý vào JSON không chứng minh binding với peer hoặc tương thích server.

**Sửa:** xác minh representation/signature scheme official (kể cả khả năng XEdDSA), device identity binding và vector. Gate integration E3. Không “sửa” bằng chấp nhận khóa ký bất kỳ trong bundle.

**Regression:** generate→export→parse→verify bằng production helpers, không inject signing key chỉ trong test; thay DH/signing identity phải bị phát hiện theo scheme target.

### A05 — P1 / S: Không reject non-contributory DH

**Vị trí:** `crypto/ratchet.rs:74`; `crypto/keystore.rs:201`; `crypto/session.rs:288`.

Kết quả X25519 được đưa thẳng vào KDF. Input low-order/all-zero có thể tạo secret không phụ thuộc private key. Nguy cơ forged plaintext trong decoder nếu payload đó tới được; **chưa chứng minh được đường tấn công xuyên server Zalo**.

**Sửa:** mọi DH phải trả Result và reject non-contributory shared secret trước KDF/state mutation, theo implementation/spec target. Không tự dùng so sánh secret thiếu constant-time.

**Regression:** low-order vectors ở X3DH và ratchet, malformed key, state/prekeys/trust giữ nguyên khi reject.

### A06 — P1 / S: Skipped-key map không lưu JSON được

**Vị trí:** `crypto/ratchet.rs:181`; `crypto/store.rs:250`.

`HashMap<SkipKey, Vec<u8>>` có key struct; serde_json không serialize object key dạng này khi map nonempty. Out-of-order decrypt có thể thành công trong RAM rồi fail khi save qua manager.

**Sửa:** representation persisted dạng entries có version, exact key lengths/counters và bounds; migration cho state hiện hữu; giữ memory map nếu cần lookup nhanh.

**Regression:** manager nhận3 trước, close/reopen, nhận1/2; tất cả đúng một lần. Bao gồm corrupt entry và transaction rollback.

### A07 — P1 / S: Chọn sai OTP, consume trước authentication

**Vị trí:** `crypto/session.rs:123`, `:286`; `crypto/wire.rs:39`; `crypto/store.rs:216`.

Receiver lấy OTP thấp nhất; envelope không mang optional selected OTP ID. No-OTP handshake với receiver còn pool cũng sai secret. Key bị DELETE trước unprotect/MAC; message giả có thể làm mất key.

**Sửa:** không mở rộng wire tự chế để ship; chốt field official. Lookup đúng signed/one-time key ID, load không xóa, sau authenticated decrypt mới commit consume + ratchet + trust decision + inbox dedup trong giao dịch ngắn.

**Regression:** OTP thứ2, no-OTP khi pool không rỗng, đảo thứ tự hai handshake; bad MAC/unprotect/commit fail không thay hàng nào.

### A08 — P1 / S: Concurrent load/encrypt/save có thể tái dùng key/counter

**Vị trí:** `e2ee_integration.rs:22`; `crypto/session.rs:270`; `gui/worker.rs:1153`.

Registry lock chỉ bao get_manager; mỗi lần lại mở Connection mới. Hai job có thể đọc cùng state rồi ghi đè advancement. Một manager nằm giữ trong map nhưng không được dùng lại; không phải singleton crypto operation như comment.

**Sửa:** ownership/serialization theo account-peer-device, atomic first-use và revision/CAS hoặc actor single writer. Không khóa global xuyên HTTP await, không thay bằng một mutex toàn app làm chặn peer khác.

**Regression:** barrier hai send cùng state, concurrent initialization, receive/send race, DB busy/crash; counter/envelope khác nhau, một identity duy nhất và reopen nhất quán.

### A09 — P1 / S: State tiến trước delivery, thiếu durable encrypted outbox

**Vị trí:** `crypto/session.rs:251`, `:270`; `chat.rs:8319`; `outgoing.rs:53`.

Sau encrypt đầu đã lưu session; request không đến peer thì lần sau gửi Whisper không có handshake. Retry regenerate ciphertext, Job không giữ prepared envelope.

**Sửa:** giao dịch persist state advancement + immutable envelope + idempotency/delivery status trước network; handshake pending theo contract; unknown delivery reconcile, không rollback ratchet tùy tiện hoặc gửi plaintext.

**Regression:** drop request đầu trước delivery; mất ACK; crash sau prepare/trước send/sau send; restart retry peer vẫn decrypt và không duplicate row.

### A10 — P1 / S: Realtime receive không truyền account context để decrypt

**Vị trí:** `gui/worker.rs:1033`, `:2351`; `chat.rs:6120`, `:5814`.

WS/poll gọi merge wrapper với profile None; chỉ một số preview-fetch path truyền context. “Đã thêm helper” chưa là integration mọi receive path.

**Sửa:** ingestion chung owner+generation+source, authenticated decrypt trước persist/publish, dedup trước advance; history/import không tự decrypt envelope đã xử lý lần nữa.

**Regression:** cùng fixture qua WS/poll/initial/backfill, output text/state đồng nhất; packet trùng, stale owner và self-device echo không dùng sai peer.

### A11 — P1 / S: Trust bị ghi đè và replay handshake có thể rollback session

**Vị trí:** `crypto/session.rs:251`, `:283`; `crypto/store.rs:314`; `chat.rs:5816`.

Outgoing bỏ identity-changed result; incoming overwrite trusted key và bỏ warning. Type3 luôn tạo session mới; replay handshake no-OTP có thể ghi đè receiving state đã tiến.

**Sửa:** separate trusted/pending identity, provenance và verify decision. Nhận diện handshake/replay đã xử lý trong atomic inbox+state commit; đừng chặn mọi type3 một cách mù vì retransmission/thiết bị mới có contract riêng.

**Regression:** valid replacement identity báo banner bền vững; replay initial sau nhiều ratchet không lùi state, không deliver lần hai; failed auth không thay trust.

### A12 — P1 / S: Lifecycle ký/prekey và reorder qua DH boundary chưa đủ

**Vị trí:** `crypto/session.rs:88`, `:139`, `:285`; `crypto/store.rs:152`; `crypto/ratchet.rs:325`, `:362`; `e2ee_integration.rs:58`.

Signed key rotate dùng cùng ID thay row cũ; receiver lookup current key bỏ ID. Refill chỉ được test gọi; upload fail bị nuốt, allocator dựa row còn lại có thể reuse sau hết pool. DH transition chưa preserve old chain đến `header.pn`.

**Sửa:** SEC-01 allocator bền vững, rotation/lookup tách biệt và grace-retention theo contract; registration/refill ACK explicit. SEC-02 save skipped old chain trước reset, giới hạn tổng skipped keys/counter/expiry.

**Regression:** delayed old-SPK handshake sau rotate, drain/restart/refill không reuse ID, upload fail observable, old-chain delayed message qua DH transition decrypt được.

### A13 — P1 / G: Official E2EE contract và UI trạng thái chưa nghiệm thu

**Vị trí:** `crypto/prekey.rs:7`; `crypto/wire.rs:3`; `crypto/safety.rs:38`; `gui/safety_number_dialog.rs:17`; `gui/messages.rs:1334`, `:1482`; `chat.rs:8342`.

Wire custom, server schema E3. Safety generator chỉ pair keys, chưa có vector official cho identifiers/version. Dialog không có caller/fingerprint load. **Badge đã có render**, nên nói “badge hoàn toàn chưa viết” sai; nhưng bool có thể bật cả khi decrypt helper trả error text, không chứng minh encrypt/decrypt thành công.

**Sửa:** SEC-01 protocol spike bắt đầu sớm; UI làm song song theo state machine plain/required/pending/encrypted/decrypt-failed/identity-changed. Không tự chốt scheme từ route. Payload text/media/group và peer-device addressing phải nằm trong matrix đã chốt.

**Regression/acceptance:** vector độc lập + fork↔official hai chiều trên tài khoản thử nghiệm; UI failed không có success badge; safety button→worker→correct peer fingerprint→verify action. Không quảng bá full E2EE trước evidence.

### A14 — P1 / S: Diagnostic dump và corruption handling chưa fail-safe

**Vị trí:** `session.rs:662`; `logger.rs:109`; `session.rs:1071`, `:1131`; `crypto/store.rs:131`, `:177`, `:221`.

Dump ghi body gốc cả khi dump_raw=false. Log error có nhánh giữ reqwest URL hoặc server error_message chưa scrub; chỉ là đường có thể lộ dữ liệu nếu response/error chứa nhạy cảm, không tuyên bố đã có leak. Stored blobs ngắn bị slice panic; consume key có thể delete trước unprotect.

**Sửa:** scrub schema-aware/allowlisted diagnostics và credential exclusion kể cả dev; raw capture riêng có policy tài khoản test và không vào Git. Exact-length decode trả corruption error; không tự regenerate identity hoặc fallback plaintext khi DB lỗi.

**Regression:** synthetic nested credential/body/error/query canaries vắng mọi sink; short/oversized blobs trả Err không panic/delete. Không debug bằng dump identity table.

### 3.1. Bổ sung audit E2EE chuyên sâu và tự phản biện

Giữ A01..A36 để không làm mất liên kết task. A37..A41 dưới đây bổ sung lỗi/biên còn thiếu; A06/A12 được nâng bằng chứng từ source sang reproduction cho các trường hợp cụ thể đã probe, không suy rộng mọi trường hợp out-of-order đều hỏng.

### A37 — P1 / R: Counter nhận không theo chain key sau khoảng nhảy

**Vị trí:** `crypto/ratchet.rs:337`, `:374` (`skip_over_recv_chain`).

Skip loop tiến chain key và lưu khóa bỏ qua nhưng không tăng `recv_count`; decrypt chỉ `+=1`. Probe khởi tạo phiên mới, gửi4 message cùng chain: nhận index2 thành công nhưng counter=1 (expected3); nhận index3 tiếp theo thất bại. Test hiện có chỉ nhận2→0→1 nên không phát hiện tin mới sau gap bị hỏng. Đây là lỗi độc lập với A06: sửa serializer không sửa được counter.

**Hướng sửa:** skip và counter phải chuyển đồng bộ trong candidate state, sau authenticated decrypt counter đại diện vị trí kế tiếp. Không chỉ gán `recv_count=header.n+1` trước authentication rồi lưu; validate stale index/overflow và rollback.

**Regression:** 2→3→0→1; 0→2→3→1; khoảng nhảy biên MAX_SKIP; tiếp tục gửi hai chiều và reopen DB. Assert plaintext, counter, skipped entries và không dùng lại message key.

### A38 — P1 / R: API ratchet không rollback state khi authentication lỗi

**Vị trí:** `crypto/ratchet.rs:301`, `:313`, `:325`, `:337`.

Probe bad-MAC trả Err nhưng serialized state trước/sau khác nhau. Nhánh skipped-key còn remove entry trước verify; nhánh DH có thể thay root/chain/counters trước MAC. **Giới hạn quan trọng:** Whisper path của `E2eeManager` hiện load object riêng rồi bỏ nếu decrypt Err, nên không kết luận mọi bad MAC đã persist state hỏng. PreKey path vẫn có mutation DB sớm A07; direct caller hoặc actor reuse object sẽ gặp lỗi A38 ngay.

**Hướng sửa:** decrypt trên candidate, chỉ thay state sống sau authentication và bước persistence cần thiết thành công; hoặc API trả transition bất biến cho coordinator commit. Candidate clone phải nằm trong memory budget và được xóa secrets khi bỏ; không giữ một state đã mutate trong cache sau commit failure. Không copy mã giả spec vào production mà thiếu rollback boundary.

**Regression:** bad MAC ở current/old-skipped/new-DH paths; ciphertext hợp lệ nhưng padding invalid; decrypt Err không thay state/skipped key/trust/DB. Gửi lại ciphertext đúng sau lỗi vẫn giải mã được.

### A39 — P1 / S+G: Authenticated context và wire metadata chưa có contract hoàn chỉnh

**Vị trí:** `crypto/ratchet.rs:469` (`compute_mac`), `crypto/wire.rs:39`, `crypto/session.rs:254`.

MAC hiện bao DH/counters/ciphertext, không nhận AD chứa identities/context; wire version/type/registration metadata chưa có authenticated layout được chứng minh. `start_session` đặt registration_id của remote vào field mà wire comment mô tả sender registration. Đây là inconsistency và khoảng trống chứng minh, không tự kết luận đã có unknown-key-share exploit hoặc có thể sửa mọi metadata mà MAC vẫn được nhận.

**Hướng sửa:** xác minh official bytes/encoding/key-prefix/direction của registration và device IDs; định nghĩa AD/header authentication theo protocol target. Không tự thêm trường/MAC/cipher mới để hợp thức hóa prototype rồi gửi server. Kiểm thử context binding ngay cả khi local sender/receiver cùng dùng custom format.

**Regression:** context/account/peer/device/version/header mismatch phải reject theo scheme đã chốt; registration sender/recipient đúng bằng fixture độc lập, không chỉ serialize rồi deserialize bằng cùng code.

### A40 — P1 / S: Durable crypto store chưa có guard/migration và addressing đầy đủ

**Vị trí:** `crypto/store.rs:20`, `:38`, `:69`, `:258`, `:270`; `crypto/session.rs:47`.

E2eeManager::open luôn dùng `storage::root()/e2ee.db`, phân account bằng row profile_id; không phải mỗi profile_id tự có file DB riêng. Device hardcode1 ở reads/writes; migration bootstrap không có transaction bao toàn bước và không reject version mới hơn. Không có bằng chứng row-scoping hiện tại tự làm lẫn account; vấn đề là roadmap/file-isolation/device guarantees chưa được đáp ứng và future schema write thiếu guard.

**Hướng sửa:** chọn canonical account/profile/device address cùng ACCOUNT-01; migrate an toàn thay vì đổi path làm mất khóa cũ. Guard schema newer, transaction+version bump atomic, validate persisted state. Nếu inbox/outbox còn nằm messages.db riêng, phải có quyết định rõ về atomicity: cùng store cho các metadata cần giao dịch hoặc recovery journal có idempotency/reconciliation; không tuyên bố transaction qua hai connection là atomic. Đánh giá WAL/crash semantics trước chọn attached-DB transaction.

**Regression:** fresh/current/newer/partial schema, crash migration, duplicate names/different UIDs, A/B owner và device separation, restart pending outbox. Không xóa DB người dùng để “migrate thành công”.

### A41 — P2 / S+L: Giới hạn state, secret memory và kiểu dữ liệu còn lỏng

**Vị trí:** `crypto/ratchet.rs:43`, `:137`, `:181`, `:281`, `:430`; `crypto/keystore.rs:21`; `crypto/store.rs:250`, `:278`.

MAX_SKIP mới bound gap mỗi lần, chưa bound tổng skipped entries/bytes/age. Counter `+=1` chưa checked; root_from_secret nhận slice rồi pad/truncate thay vì exact32. Một số buffer KDF/serialized state/root-chain/skipped/signing-private chưa zeroize; cloned secrets sống lâu hơn cần thiết. Public arrays/Vec được deserialize chưa validate toàn bộ invariants. Windows DPAPI bảo vệ blobs at rest, không tự bảo vệ plaintext RAM hay chống rollback state từ backup.

**Hướng sửa:** validated types/counters và limits trước allocation/derivation; state version và validator sau decode; Zeroizing/Drop cho secret buffers phù hợp, hạn chế clone. Root input exact32. Chính sách backup/restore không cho tái sử dụng state gửi cũ một cách mù; mất/corrupt keys phải báo rõ, không regenerate hoặc fallback plaintext. Non-Windows storage::protect hiện chỉ copy bytes: chỉ được coi là test/unsupported-security target nếu chưa có secure backend, không quảng bá DPAPI cross-platform.

**Regression:** total cap nhiều chain, MAX counters, malformed skipped key length, oversize state, wrong root length, retention/expiry và restore/recovery policy; kiểm tra source/ownership về zeroize, không hứa chứng minh sạch mọi bản sao RAM bằng unit test.

#### Kết quả probe ghi nhận ở cùng baseline

```text
receive_n2_ok=true
recv_count_after_n2=1 expected=3
skipped_json_saves=false
receive_n3_ok=false
tampered_rejected=true
state_unchanged_after_bad_mac=false
new_chain_ok=true
old_chain_delayed_ok=false
```

Các probe dùng cùng library đã build và khóa/tin giả, không dùng session/live Zalo. Đây là reproduction local, không phải cross-client interop. AI sửa phải biến từng scenario thành regression trong repository, xác nhận fail trước/pass sau; không chỉ chép output vào verification. Không phụ thuộc thư mục temp của phiên audit còn tồn tại.

#### Các kết luận cần giữ cân bằng khi thực hiện

- **Có foundation, nhưng có lỗi nội bộ:** không nói “chưa có PreKey/Ratchet”, cũng không nói “chỉ còn UI/manual”.
- **CBC+HMAC không tự là lỗ hổng.** Không thay GCM để khớp dòng roadmap khi chưa có protocol evidence. Constant-time HMAC verify đang dùng verify_slice là điểm nên giữ.
- **31 test pass không là31 E2EE interop tests.** Có cả AES/MD5 transport; deterministic/roundtrip không thay known-answer độc lập hoặc durable-state tests.
- **Không bỏ kiểm tra chữ ký để chữa parser.** Verifier hiện reject; nơi hạ plaintext là outgoing. XEdDSA được mô tả trong X3DH chuẩn, chưa đủ chứng minh Zalo version đang dùng scheme nào.
- **DPAPI có thật nhưng không mã hóa toàn DB hoặc solve atomicity.** Metadata có thể không encrypted; storage row-isolation khác file-isolation; không tự gọi shared file là cross-account breach.
- **UI làm song song được**, nhưng success badge/fingerprint phải có provenance. Required-E2EE chưa hỗ trợ phải fail closed; group/media thiếu vẫn là blocker scope full, không tự hoãn để tăng completion.

### 3.2. Bảng hành động E2EE hợp nhất cho AI

Tất cả task sau đang **chưa thực hiện trong lượt lập hướng dẫn**. P0/P1 là ưu tiên, không phải thứ tự số tuyệt đối. “Không phụ thuộc official contract” nghĩa là được sửa correctness local/viết synthetic regression; không cho phép thay đổi server wire tùy ý.

| Task / owner đề xuất | Finding / phụ thuộc | Hành động và output phải giao | Regression / điều kiện đóng |
|---|---|---|---|
| **E00 — Send-policy coordinator / SYS-02** | A01/A02; bắt đầu ngay | Inventory mọi send entrypoint; policy plain/required/unknown có nguồn, gate E3; không fallback plaintext; pending/unknown delivery typed. Giữ ordinary-send đúng cho verified plain. | Zero ordinary requests khi required-E2EE signature/DB/ACK/unsupported-payload lỗi; direct/group/link routing không regression. Gate tạm không tính E2EE complete. |
| **E01 — Protocol evidence / SEC-01** | A04/A13/A39; chạy song song E00 | Pin official client/bundle version/hash; request/response prekey, ciphertext, signatures, encoding, AD/MAC/KDF, device addressing, capability và lifecycle. Giao bảng field→source E1/E2/E3, fixture sanitized và unresolved blockers. | Evidence độc lập đủ xác định contract mục tiêu; route index không đủ. Timebox hết mà thiếu evidence thì output blocker, không đoán contract để đóng. |
| **E02 — Key parser / SEC-01** | A03/A04/A05; A39/A41; E01 cho server contract | Exact-length key/signature decoding, encoding disambiguation, checked IDs/error envelope, non-contributory DH rejection; identity-signature binding theo target. Không nuốt missing upload ACK thành success. | Generate→export→parse→verify không test-only injection; malformed/hex/base64/tampered/low-order fixtures, không state mutation. |
| **E03 — Ratchet core / SEC-02** | A37/A38/A12/A41; local correctness làm ngay | Counter/skip/old-pn transition đúng, candidate rollback, checked overflow và total skip budget; validation trước crypto work. | 2→3→0→1; 0→2→3→1; delayed previous-chain; bad MAC ở3 paths không đổi state; boundaries/replay/overflow. |
| **E04 — State codec & migration / SEC-02** | A06/A14/A40/A41; phối hợp E03 | Persisted skipped entries versioned, exact blob/state validator, schema transaction/newer guard; canonical paths/address. Chuẩn bị recovery/migration không reset identity. | Nonempty skipped map serialize→DPAPI→DB→reopen→decrypt; corrupt/oversize/newer-schema safe Err; legacy fixture migration rollback. |
| **E05 — Peer operation ownership / SEC-02** | A08/A40; E03/E04 | Actor/single-writer hoặc equivalent revision protocol theo account-peer-device, atomic first identity/session initialization; remove misleading manager registry sau migrate caller. | Barrier concurrent sends/first-use/receive; không key/counter reuse/lost update; peer khác vẫn tiến; không giữ global lock/network await. |
| **E06 — PreKey lifecycle / SEC-01** | A07/A12; E01/E02/E04/E05 | Lookup đúng OTP/SPK ID kể cả absent-OTP; allocator bền vững, retention/rotate, registration/refill pending/ACK; consume sau auth trong unit commit được xác định. | OTP thứ2/no-OTP/reverse handshakes, stale SPK sau rotate, drain+restart không reuse IDs, bad MAC/unprotect/commit failure không mất keys. |
| **E07 — Durable inbox/outbox / SEC-02** | A09/A11/A40; E04/E05/E06 | Persist immutable envelope+clientId+delivery/pending-handshake cùng state advancement bằng transaction hoặc recovery protocol đã review; atomic inbox dedup/anti-replay và reconciliation khi ACK không chắc. | Crash trước/sau prepare/send/ACK/commit; restart không mất handshake, không encrypt lại cùng operation hoặc deliver replay; không rollback send state tùy tiện. |
| **E08 — Unified ingestion / SEC-03** | A10/A13; E03..E07; phụ thuộc ACCOUNT owner boundary | WS/poll/history/import/self-echo dùng owner+generation+peer-device mapping; decrypt/dedup/persist trước UI. Typed -29/-30 pending/decrypt failure; không encode lỗi thành plaintext-success. | Cùng fixture mọi receive path ra cùng message state; stale account/event, duplicate và replay không advance ratchet2 lần; terminal errors có payload. |
| **E09 — Trust & UI / SEC-03** | A11/A13; UI contract làm song song E01/E03; persist theo E04/E07 | Trusted/pending identity và verify policy; nối SafetyNumberDialog sẵn có qua worker; banner bền vững; enum trạng thái thay bool-only success; official fingerprint theo E01. | Identity đổi không silently overwrite; safety đúng owner/peer/device; failed/pending không success-lock; verify/dismiss đúng nghĩa và tồn tại qua restart. |
| **E10 — Payload & group/device coverage / SEC-03** | A01/A13/A39/A40; E01/E07/E08 | Matrix text/link/quote/forward/image/file/reaction/receipt/group+devices; implement theo capability official hoặc từ chối required-E2EE rõ ràng. Không suy SenderKey từ route. | Contract fixtures và fork↔official text/ảnh/file hai chiều; group join/leave/key distribution khi scope yêu cầu; không bỏ payload ngoài matrix. |
| **E11 — Secret lifetime & diagnostics / SYS-02, SEC-02** | A14/A41; làm song song, finalize sau ownership | Redaction tất cả sinks, bounded decoded buffers, zeroize/copy review, DPAPI/context, backup/restore/isolation và recovery documentation. | Canary secrets vắng logs/dumps; no panic/corrupt-key fallback; không dump private DB rows; native storage checks đúng user. |
| **E12 — Independent verification / QUALITY-01, SYS-05** | E00..E11 | Review mật mã độc lập, vectors từ nguồn khác, full regression/native matrix, exact build provenance; reconcile roadmap/spec statuses theo evidence. | Fork↔official + ARM64 + crash/reorder/concurrency/replay tests recorded; no blockers bị đổi thành pass; release gate đúng SHA. Không tự publish khi task chỉ được giao audit/sửa. |

**Đồ thị phụ thuộc tối thiểu:** E00 làm trước để chặn downgrade; E01 nghiên cứu song song E02/E03 local/E04 design/E09 UI contract/E11. E03+E04→E05→E06/E07→E08; E01 chốt trước khi thay official server format ở E02/E06/E10. E07/E08+E01→E09 final/E10; tất cả→E12. Nếu một task cần contract còn Unknown, ghi blocker tại đúng nhánh rồi tiếp tục nhánh độc lập; không giữ toàn dự án chờ capture.

**Ownership:** một người/agent điều phối `crypto/session.rs`, `e2ee_integration.rs`, `chat.rs` và `gui/worker.rs`; agent core/parser/store/UI giao patch nhỏ theo phạm vi, không cùng rewrite các file điều phối. Với state schema và outbox, review thiết kế atomicity trước edit; không làm từng bảng rời rồi vá retry ở cuối.

### 3.3. Regression E2EE cần bổ sung theo hành vi

Danh sách này là acceptance backlog, **chưa viết/chưa pass**, không chạy mạng Zalo trong default unit tests:

| ID | Scenario và assertion trọng tâm |
|---|---|
| ER01 | Production bundle parser giữ nguyên signature64 và verify được chữ ký hợp lệ; không chấp nhận32 hoặc tampered. |
| ER02 | Hex/base64 tương đương, invalid lengths/overflow/missing/string/nested error bị reject đúng. |
| ER03 | Reject low-order DH ở X3DH và ratchet, state/prekeys/trust không đổi. |
| ER04 | 2→3→0→1 và0→2→3→1 cho đúng plaintext/counter/skipped entries. |
| ER05 | Out-of-order qua E2eeManager, persist/reopen rồi nhận các tin còn lại, không chỉ test RAM. |
| ER06 | Delay message chain cũ, exchange reply, chuyển DH mới, nhận delayed thành công. |
| ER07 | Bad MAC/current-skipped-newDH và padding failure không đổi live state hoặc DB; ciphertext đúng sau đó vẫn decrypt. |
| ER08 | OTP thứ2, no-OTP khi receiver còn pool, hai handshake đảo thứ tự dùng đúng key ID. |
| ER09 | Bad handshake/unprotect/commit failure không consume OTP hay đổi trusted identity. |
| ER10 | Rotate SPK rồi nhận initial message dùng SPK cũ; đúng key retention, không side-effect rotate lúc lookup. |
| ER11 | Drain/refill/restart/upload failure: monotonic IDs, explicit registration pending/ACK và retry bounds. |
| ER12 | Concurrent send/receive/initialization có barrier, một identity và state progression nhất quán; độc lập peers. |
| ER13 | Drop initial trước delivery; crash/restart; retry giữ prepared handshake/envelope/client ID. |
| ER14 | ACK mất hoặc duplicate envelope: không plaintext resend, không deliver hai lần, không rollback ratchet. |
| ER15 | Replay initial no-OTP sau nhiều ratchet và replay old Whisper: state không lùi/advance sai. |
| ER16 | Required-E2EE với mọi payload/entrypoint lỗi: zero ordinary-send và không URL metadata leak. |
| ER17 | WS/poll/initial/backfill cho kết quả state tương đương; stale owner/device/self-echo không giải mã nhầm. |
| ER18 | Identity changed→pending/warning→verify; fingerprint peer/device đúng; failed/pending badge không báo encrypted-success. |
| ER19 | Schema newer/corrupt/short blob/oversize state/counter overflow/skip budget, fail closed không reset identity. |
| ER20 | Context/header/version/ID tamper theo official scheme bị reject; vectors từ independent implementation. |
| ER21 | Logs/dumps/GUI error không lộ synthetic credential/private/plaintext canaries; ownership và cancel cleanup đúng. |
| ER22 | Native/cross-client text/image/file/group/device/offline/reset/restart trên exact SHA; có kết quả và giới hạn từng scenario. |

Chạy regression có chủ đích trước rồi `cargo check --locked`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`; format/dependency/license/native checks tương ứng trước release. Đo time/CPU/bytes cho skipped-key limits và per-peer queue; tối ưu không được bỏ auth/checksum, unbounded queue hoặc thay protocol. Các command này là chỉ dẫn tương lai; lượt hợp nhất tài liệu không chạy lại test/code.

## 4. Realtime, dữ liệu và authentication

### A15 — P1 / S: Liveness chưa có deadline trước pong đầu

**Vị trí:** `socket.rs:654`, `:665`; `gui/worker.rs:1132`.

Authenticated epoch còn ready nhưng deadline chỉ enforce sau saw_pong; silent trước pong đầu không recovery, polling bị skip. Sleep arm cũng phải là deadline ổn định, tránh bị tạo lại qua mỗi select mà không đến hạn.

**Sửa:** bounded initial grace và silence/heartbeat deadlines đã xác minh; watch ready phản ánh khả năng phục vụ. Không đòi pong sai contract với server không echo nhưng vẫn có traffic.

**Regression:** fake auth rồi silent trước pong, sau pong, only-data no-pong, write stall, suspend; epoch kết thúc/fallback/catch-up đúng, không kill connection có inbound hợp lệ.

### A16 — P1 / S: Backpressure làm chặn socket control loop

**Vị trí:** `socket.rs:285`, `:838`.

Reader await send vào channel4096; consumer chậm khiến heartbeat/control/cancellation/deadline không được phục vụ.

**Sửa:** throttle pull dựa capacity, ưu tiên control/liveness, bounded backlog và replay từ cursor durable khi overflow. Không thay bằng unbounded channel hoặc drop message rồi advance cursor.

**Regression:** giữ consumer, vượt capacity; control/stop được xử lý, replay đủ/đúng thứ tự, cursor chỉ commit sau persist.

### A17 — P1 / S: Queue error bị coi completed; backoff không reset

**Vị trí:** `socket/queue_receiver.rs:90`, `:116`; `socket.rs:508`, `:549`.

Registered reply thiếu more sau lỗi có thể clear retry; backoff tăng cả sau phiên khỏe, lâu dần disconnect nào cũng chờ30s.

**Sửa:** validate envelope/error trước completion, giữ cursor, bounded read retry; reset backoff sau stable/auth condition có lý do, jitter, network-restored signal. Tách detection time với reconnect SLA.

**Regression:** error numeric/string/nested/missing-data→valid recovery; fail→healthy→disconnect không giữ maximum backoff vô lý.

### A18 — P1 / R: Promotion message ID vướng foreign key

**Vị trí:** `store.rs:601`; `message_lifecycle.rs:85`, `:95`.

UPDATE primary key local→server trong messages không có ON UPDATE CASCADE tương ứng ở message_expiry/restored_assets. SQL tổng hợp tái hiện `FOREIGN KEY constraint failed`; batch persistence có thể rollback.

**Sửa:** migration mới hỗ trợ atomic identity promotion của **mọi** dependent table/index/trigger; cân nhắc cascade hoặc explicit remap trong transaction. Không tắt foreign_keys để né lỗi; không copy-delete khiến restored_gc dọn asset đang dùng.

**Regression:** pending message có expiry, asset, cả hai→server echo; metadata/dedup/expiry/GC giữ đúng, migration fresh/existing/reopen/rollback. Chạy trên rusqlite bundled trước merge, không chỉ Python probe.

### A19 — P1 / S: Password và QR login race không có attempt generation

**Vị trí:** `gui.rs:4961`, `:5027`, `:2063`; `gui/worker.rs:3438`, `:3443`, `:3471`, `:1734`.

Password task giữ session lock xuyên network, QR chỉ cancel task QR; password success cũ có thể đến trước task QR logout làm UI logged-in nhưng worker đang QR.

**Sửa:** auth-attempt generation/cancel, build candidate session ngoài live owner lock, chỉ commit attempt còn hiện hành; scoped terminal event. Không chỉ disable tab UI rồi bỏ invariant worker.

**Regression:** barrier password→request QR→release password, chiều ngược, logout/retry khi đang auth; stale attempt không install credential/UI success.

### A20 — P1 / L+G: Multi-account chưa có đường switch runtime hoàn chỉnh

**Vị trí:** `gui/account_switcher.rs:24`; `gui/workspace.rs:214`; `gui/worker.rs:1671`; `gui.rs:2671`; `profiles.rs:33`, `:79`.

Avatar popover **có thật** tại modals.rs:4 và workspace.rs:3887. Nhưng không thấy caller AccountSwitcher.show hoặc construct SwitchProfile. Handler logout nguồn trước load đích; event bỏ uid, chỉ refresh, thiếu identity/cache/task reset. Path helper registry không validate như storage::validate_profile, save không atomic; chưa phải bằng chứng menu hiện tại đang làm lẫn account vì chain chưa được nối.

**Sửa:** ACCOUNT-01 chuẩn bị session đích+UID trước commit; rollback giữ nguồn; task cancellation/owner generation và UI/cache/draft/hidden/media/call reset. Nối registry lifecycle login/add/select/remove qua worker; canonical base/profile/account paths, không song song hai layout DB.

**Regression:** A→B→A, stale A events, corrupt/expired B, forged registry path, switch lúc upload/call/Hello; đăng nhập QR mới thêm registry đúng owner, không mất nguồn khi failure.

### A21 — P1 / S: Lock không bao toàn bộ render/read boundary

**Vị trí:** `gui.rs:4532`, `:4766`, `:4809`, `:4855`, `:2186`, `:2248`, `:2283`.

touch xảy ra trước timeout, input đầu sau idle có thể reset deadline. Dev Inspector render trước lock check vẫn tương tác khi khóa. check_events xử lý incoming dựa focus mà không xét unlocked, nên local unread selected thread bị clear dù bị che. Không suy ra đã gửi server read receipt nếu chưa trace riêng.

**Sửa:** xác định lock transition trước input/event/render, scheduled deadline qua request_repaint_after; “foreground reading” cần focused+visible+unlocked. Gate dev panel, viewer/native child windows, notification text và clipboard theo policy.

**Regression:** synthetic overdue input, locked inspector canary, focused locked receive giữ unread; no sensitive render/click behind overlay; native video window khi khóa.

### A22 — P1 / S+G: Hello blocking, không opt-in/recovery và thiếu desktop integration

**Vị trí:** `gui/auth_guard.rs:29`, `:61`, `:83`, `:130`; `gui.rs:1246`, `:5155`, `:2680`.

Spawn rồi join vẫn chặn GUI constructor; lock mặc định5min không enabled/capability gate; unlock thực bỏ qua availability check. Request không gắn owner/generation, UI tự spawn; thiếu đường an toàn khi Hello unavailable. Desktop dùng RequestVerificationAsync trực tiếp thay vì xác nhận interop HWND phù hợp.

**Sửa:** LOCK-01 opt-in/settings/capability bất đồng bộ; UI→worker→desktop consent adapter→owner+generation result; Verified mới unlock; logout/re-auth recovery không lộ dữ liệu. Không tự chế PIN hoặc dùng mật khẩu Zalo như credential local.

**Regression/native:** verifier slow/cancel/fail/unavailable, old result sau switch/logout, sleep/tray/idle; fake verifier unit trước, Win32 HWND + Hello thực sau. Không khẳng định tokio::spawn luôn panic: cần xét runtime thực.

## 5. Media, upload, download và an toàn nội dung

### A23 — P1 / R: DoH query sai và proxy bypass

**Vị trí:** `doh_resolver.rs:119`, `:50`; `media_download.rs:60`.

`type=A,AAAA` không hợp lệ; endpoint probe HTTP400. Resolver và pinned client no_proxy, trái normal media proxy policy; lookup có thể đi trực tiếp ngay hôm nay, download trực tiếp khi query được sửa.

**Sửa:** A/AAAA riêng, HTTP/DNS Status và type/owner/CNAME validation; preserve explicit proxy/fail-closed nếu không thể honor. DNS override giữ original hostname/SNI/certificate validation.

**Regression:** mock provider verifies đúng query; một family fail vẫn dùng family hợp lệ; proxy fixture không direct connection; DNS fail không đổi TLS validation.

### A24 — P1 / S: TTL/cache/dedup và test DoH chưa đúng

**Vị trí:** `doh_resolver.rs:64`, `:88`, `:100`, `:133`, `:156`; `media_download.rs:43`.

Max TTL thay vì TTL hợp lệ nhỏ nhất, clamp min60 kéo dài answer ngắn; tất cả lỗi negative600s kể cả transient, cache không bound/single-flight. DnsResponse bỏ Status/type. Test test_doh_resolve gọi internet thật rồi bỏ kết quả: pass dù resolver hỏng, vi phạm fixture-only.

**Sửa:** monotonic expiry/remaining TTL không kéo stale; NXDOMAIN khác timeout/SERVFAIL, bounded entries/cleanup, single-flight cancellation-safe, tổng deadline. Bound response JSON; không nhận địa chỉ local/private ngoài policy CDN. Pinned clients dùng cùng expiry; Range không có DNS path riêng khác policy.

**Regression:** no public-network test mặc định; fake HTTP/DNS failures/TTL0/low TTL/CNAME/AAAA/stampede/eviction/cancel. Test lỗi phải fail nếu fallback không hoạt động.

### A25 — P1 / S: Cache video có thể xóa partial đang tải

**Vị trí:** `video_player.rs:33`, `:43`; `media_download.rs:303`.

Lease đăng ký final path; download ghi random partial khác. Reserve video B có thể dọn partial A. OS có thể từ chối delete file mở, nhưng không được dựa vào đó làm ownership invariant.

**Sửa:** lease ownership cho final+partial từ lúc tạo tới drop, hoặc cache prune loại active partial có token; tránh bỏ mọi partial vĩnh viễn không GC.

**Regression:** hold A sau chunk đầu, reserve B và prune; A còn byte/commit thành công; cancel/crash orphan cleanup không đụng saved original.

### A26 — P1 / S: Mở attachment thiếu gate cho executable/script/shortcut

**Vị trí:** `gui/messages.rs:785`; `gui/worker.rs:1435`; `platform.rs:92`.

Mở tệp chung tải xong gọi shell theo association; platform chỉ check is_file. Không có attachment execution classification/Mark of the Web ở finalize. Đây là đường do người dùng bấm, **không gọi là zero-click RCE**.

**Sửa:** preview/open chỉ cho loại tài liệu phù hợp; executable/script/shortcut không tự chạy như document. Quy định save-only hoặc thao tác xác nhận rõ ràng phù hợp Zalo/reference; Windows attachment handling/MOTW và extension/content validation. Không tắt OS security prompt.

**Regression:** mock shell với harmless .exe/.cmd/.ps1/.lnk và double-extension/ADS names không execute từ ordinary Open; document hợp lệ mở được, test Windows handling riêng.

### A27 — P1 / S: Auto link preview fetch được local/private URL

**Vị trí:** `chat.rs:2648`, `:2812`; `gui/messages.rs:865`; `gui/worker.rs:2856`.

Third-party scraper nhận URL bất kỳ, redirects5, không kiểm địa chỉ private/loopback/link-local mỗi hop. Link trong message có thể trigger enrich gần viewport; loopback fixture sẵn có chứng minh scraper truy cập local. Chưa chứng minh attacker nhận được nội dung trả về hoặc credentials bị gửi ra ngoài.

**Sửa:** policy rõ cho automatic preview vs user-open, scheme/credentials/address validation trước và sau redirect/DNS theo transport phù hợp proxy; bounded timeout/body hiện có phải giữ. Không dùng client session để scrape; không leak required-E2EE URL ra server parser.

**Regression:** private/loopback IPv4/IPv6, redirect public→private, rebinding/proxy policy, excessive headers/body, safe public fixture. Cấp injection/test adapter cho loopback, không bypass production guard bằng cfg(test) mà không test policy thật.

### A28 — P2 / S: Cancel chưa ngắt queue và chờ headers

**Vị trí:** `media_download.rs:287`, `:289`.

Await semaphore không cancel; sau check, await response với timeout3600s; cancellation-aware chỉ ở body. Cancel có thể giữ slot lâu.

**Sửa:** cancellation signal được race ở queue/header/body/finalize, drop request giải phóng slot; total deadline và owner check trước publish. Không thêm polling sleep UI.

**Regression:** occupy slots rồi cancel queued request; loopback nhận connection không trả headers; slot giải phóng theo bounded cancellation SLA và không publish file cuối.

### A29 — P1 / S+L+G: Resume backend có, finalize và điều khiển chưa hoàn thiện

**Vị trí:** `chat.rs:2182`, `:2200`, `:2219`, `:2257`, `:2262`, `:2279`, `:2322`; `gui/worker.rs:1599`, `:1625`.

Ordinary attachment thực đi qua checkpoint load/save. Nhưng checkpoint delete sau chunk cuối trước process/file_done/send ACK; completed checkpoint cũng bị bỏ vì không persist upload result. Resume command chỉ status, cancel chỉ xóa row; chưa thấy caller construct chúng trong src. Resume server retention/token/checksum chưa xác minh; file cùng name/size có thể khác nội dung.

**Sửa:** một durable transfer state machine tích hợp outgoing, giữ owner/thread/clientId/source fingerprint/server token TTL/layout/final result/delivery; checkpoint sau ACK. Pause/cancel signal transfer thật, finalization reconcile trước cleanup. Không đổi1MiB lên4–8MiB chỉ để khớp roadmap.

**Regression/native:** fail final chunk/file_done/send ACK, restart, same-size source edit, expired token, cancel mid-request, switch account; 500MB ngắt50% đo bytes không upload lại phần đã ACK theo contract server.

### A30 — P1 / S+G: Video/gallery/document chưa đạt WP15

**Vị trí:** `gui/modals.rs:2441`, `:2743`, `:2758`, `:2768`; `media_download.rs:503`; `gui.rs:5851`, `:5915`; `gui/doc_viewer.rs:76`.

Video modal controls chỉ đổi local fields, duration0, native player riêng không nhận lệnh. Range helper chỉ có test caller, nhận2xx mà chưa validate Content-Range/offset; không có progressive source. Doc modal không có caller constructor/content event/production render (render call nằm cfg(test)). Gallery có zoom/pan/arrows/save/copy nhưng rotation/count/window acceptance chưa đủ.

**Sửa:** MEDIA-01..03 nối player command/state thực; Range206/Content-Range/200 fallback/416/validator/redirect/DoH cùng một download policy; document preview worker bounded và PDF page budget; gallery counter nguồn DB/neighbor thật. Native shell scope đã được đồng ý không chứng nhận full progressive WP15-2.

**Regression/native:** visible buttons phải ảnh hưởng playback thật; seek trên file200MB, cancel/stale source; preview text/PDF open/search/jump/close/errors; gallery0/1/N, rotate+pan, deleted neighbor/account switch. Hỗ trợ Office phải xác định requirement riêng, không tự hứa từ context roadmap.

## 6. VoIP, API phụ và release

### A31 — P1 / S: Sai scale RMS trên output I16/U16

**Vị trí:** `media.rs:1335`, `:453`.

Năng lượng tính sau convert sample thiết bị rồi cast thẳng f32; I16 không normalized, U16 silence có midpoint khác0. Gate dùng threshold normalized0.03, có thể suppress mic liên tục.

**Sửa:** lấy normalized playback float trước device conversion làm reference; channels/resampling/underrun silence nhất quán. Không chỉ tăng threshold cho từng device để che lỗi.

**Regression:** cùng waveform F32/I16/U16 cho RMS/gate tương đương, unsigned silence≈0, clipping và underrun. Native speaker/headset sau unit.

### A32 — P1 / G+S: AEC3, software codec và screenshare chưa đầy đủ

**Vị trí:** `media.rs:447`; `windows_video.rs:211`, `:748`; `call.rs:3059`, `:3844`.

Energy gating không phải adaptive echo cancellation waveform-reference; MFT encoder chưa có independent software fallback. Screen-share success mới signaling/toast, chưa capture→encode→transport frame. Normal CI bỏ native_h264_roundtrip.

**Sửa:** VOIP-01 hoàn thiện capture/render APM, double-talk; backend fallback thực với license review; screen/window stream lifecycle và remote render. Giữ trạng thái partial tới nghiệm thu; không đổi tên stub thành AEC3.

**Regression/native:** delayed echo+near speech, forced missing MFT encode/decode, remote moving screen30fps, deny/stop/account change. Không tuyên bố không echo tuyệt đối mọi phần cứng từ một bài test.

### A33 — P1 / S: Missing data bị biến thành “empty/disabled”, số âm wrap

**Vị trí:** `business.rs:33`, `:49`, `:72`; `account_recovery.rs:21`, `:57`; `device.rs:58`; `todo.rs:151`; `poll.rs:140`; `zcloud.rs:103`.

Một số helpers decrypt rồi parse defaults mà không checked envelope/data. 2FA missing trở thành false; catalog malformed thành empty; numeric signed cast u64/u32 có thể wrap (price=-1 thành u64::MAX; vote_count/size tương tự). Không kết luận mọi module API đều sai: mutation device/poll/todo đã có confirmed_mutation ở một số đường.

**Sửa:** typed Result cho payload required, Unknown/Unavailable khác false/empty, checked_data theo schema đã xác minh; TryFrom bounds không wrap. Không thay parser “tolerant” bằng invented field schema; thực fixture official quyết định aliases.

**Regression:** missing/null/string error/nested encrypted error, negative/overflow numeric, legitimate empty list, enabled=false thật; UI không nói 2FA tắt hoặc “không sản phẩm” khi response invalid.

### A34 — P1 / S+G: LAN discovery không chứng minh LAN transfer/parity

**Vị trí:** `lan_transfer.rs:58`, `:82`; `gui/worker.rs:3101`; `gui/tools_panel.rs:779`; `specs/012-business-lan-service-compat/tasks.md:15`.

Protocol `ZALO_LAN:v1:...` tự định nghĩa, broadcast tới8998 từ ephemeral socket; chưa thấy listener/responder transfer engine. LanTransferSession chỉ struct/tests, không production transfer caller. UI vẫn nói sẵn sàng truyền nhận/Zalo lân cận. Spec012 tick cả signaling và product update trong khi business.rs mới list.

**Sửa:** gắn AMBIGUOUS/fork deviation tới có contract evidence; không phát biểu tương thích Zalo từ roundtrip parser. Hoàn thiện peer lifecycle/authorization/actual byte transfer/limits/hash/cancel hoặc ghi rõ unavailable tới đủ scope. Reconcile task012 từng requirement, không đóng chỉ vì parser/state enum tồn tại.

**Regression/native:** hai client thật discovery+authorization+transfer bytes/end-to-end hash, spoof/timeout/cancel/firewall/IPv6; không bằng một message “scan complete”.

### A35 — P1 / S: Release provenance chưa bind tag với verified SHA

**Vị trí:** `.github/workflows/publish-verified-arm64.yml:54`, `:56`; `scripts/audit-prepush.ps1:40`, `:64`.

Workflow xác minh build head_sha nhưng chỉ check draft tagName; chưa peel Git tag tới EXPECTED_COMMIT. Script prepush chỉ warn regex stubs, print version, không chạy test/4-point enforcement như mô tả. Bốn version locations hiện tại **đang khớp** v0.016-arm/0.0.16; không báo mismatch không tồn tại. Tag v0.015.2 ở660d88d không chứng nhận HEAD.

**Sửa:** resolve lightweight/annotated tag tới verified SHA trước upload và trước publish; check immutable artifact provenance, exact product version/checksums. Prepush honest checks/failure semantics; regression quan trọng không chỉ regex. Giữ tag published immutable, phát hotfix mới thay vì rewrite history.

**Regression:** mismatched/moved tag reject, matching tag accept, old artifact no replacement, gitlink orphan blocked, exact4-point mismatch fail; remote asset/CI kiểm tra theo đúng SHA khi được giao release.

### A36 — P2 / S+G: Nợ nguyên tắc và tài liệu sai nguồn

Production host literals ngoài endpoints.rs có ở `chat.rs:1798`, `:1941`, `call.rs:922`, `socket.rs:308`, và các sticker templates. Đây là vi phạm centralization, **không tự chứng minh host hỏng**. Phase2 IDs group502/recall521/ping200 mâu thuẫn transport group521/ping2; không sửa transport để khớp doc sai. `re-notes/37-bridge-table-reference.md` cần pin source/checksum và phân biệt IPC renderer-main với WS/API; không giữ E2EE channel giả định thiếu artifact.

**Sửa/nghiệm thu:** inventory literal production/test/snapshot, chuyển production constants qua resolver không đổi host hành vi; doc đối chiếu nguồn E1/E2. Làm rõ INV-1 áp dụng authenticated Zalo API, không bọc ParamCipher/cookie vào DoH/CDN/update/third-party scraper. Audit comments/debt theo standard sau lỗi chức năng; không xóa rationale protocol cũ chỉ vì trông “legacy”.

## 7. Tối ưu thuật toán có mục tiêu, chưa thay đổi code

| ID / liên kết | Điểm nghẽn và cách sửa đề xuất | Bằng chứng/đo nghiệm thu |
|---|---|---|
| OPT-01 / PERF-01 | `store.rs:94`, `:1028`: index `(thread_id,ts)` chưa bao tie-break msg_id. Migrate `(thread_id,ts,msg_id)` hoặc biến thể đã EXPLAIN; review index cũ trùng trước drop. | Probe 100k equal-ts rows: query cùng output, VM steps khoảng3.40 triệu→1,000, bỏ inner temp sort. **SQLite Python3.50.4, không phải số ms ARM64 hay speedup toàn app.** Lặp bundled rusqlite, skew/ties/expiry, warm/cold p50/p95. |
| OPT-02 / PERF-01 | `store.rs:1124`, `:1129`: hydrate quét vector tìm thread từng row, worst O(T²); build ID→index O(T) expected; selected hydration query metadata bằng primary key. | Giữ precedence tên/group và include_unlisted, bench1k/10k threads, DB round trips/alloc; không giữ map index stale sau reorder. |
| OPT-03 / TRANSFER-01 | `upload.rs:134`, `store.rs:1565`, `chat.rs:2209`: contains/sort+serialize toàn completed vector mỗi chunk khiến tổng checkpoint work ít nhất O(C²). Sequential path dùng confirmed-prefix counter, validate legacy sparse state một lần. | Bench10k/20k chunks không upload network; so CPU+DB bytes+writes, crash durability. Không làm mất schema compatibility hoặc skip unACKed chunks; future parallel upload cần bitmap/ranges thay prefix. |
| OPT-04 / PERF-01 | `rich_text.rs:148`, `:180`: mỗi cut scan mọi style và binary-search offsets, worst khoảng O(B+S² log B); precompute ranges rồi sweep events/active style counts, cache layout bounded nếu profiling cho thấy có lợi. | Cap256 styles còn phải giữ ở every deserialization path; emoji/surrogate/overlap/mentions semantics không đổi. Bench stress và thường, key cache revision/width/DPI/theme/font/language; không persist GUI galley trong ChatMessage. |
| OPT-05 / MEDIA-03 | `gui/doc_viewer.rs:37`, `:149`: future production renderer duyệt toàn lines mỗi frame, lowercase/search toàn content mỗi key. Index line offsets một lần, virtualized show_rows, search debounce+generation/cancel; jump-to-result thật. | Hiện renderer chưa runtime nên chưa có measured user slowdown. Bench10k/100k lines, file-size caps, long line/Unicode, visible rows scaling; PDF page cache budget riêng. |
| OPT-06 / VOIP-01 | `media.rs:1278`, `:1331`, `:1462`: callback dùng mutex Vec/front draining; đổi bounded preallocated ring/single-producer-consumer phù hợp ownership. | Trước/sau callback deadline, underrun/drop/latency trên ARM64; không allocation/DB/log/network trong audio callback. Không hứa giảm CPU chưa đo. |
| OPT-07 / NET-01 | Bounded TTL caches, single-flight cùng host, reuse client đúng proxy/expiry; không tạo client/tables schema lại mỗi tin như get_manager hiện tại. | Stampede N requests→bounded queries, memory bound, cancellation cleanup, per-peer progress độc lập, requests count/latency. |
| OPT-08 / PERF-01 | `logger.rs:105`, `:140`: Vec.remove(0) và get_entries clone toàn log mỗi render; ring VecDeque+revision snapshot nếu đo đáng kể. | Cap hiện500 nên **P2 thấp**, không ưu tiên hơn correctness. Giữ ordering/selection/redaction, benchmark dev panel thay vì áp dụng blanket refactor. |

Không tối ưu bằng cách bỏ checksum/signature, tắt FK, giảm privacy, đổi chunk size/cipher/protocol, tăng queue vô hạn, memmap mọi file, hoặc rewrite async architecture toàn hệ thống.

## 8. Kế hoạch dọn code rác an toàn

**Không xóa mã trong lần audit.** “Không có caller trong repository” là ứng viên, không chứng minh public API không được dùng bên ngoài. Với tính năng bắt buộc roadmap, ưu tiên **hoàn thiện/nối rồi bỏ phần trùng**, không xóa requirement để báo hết dead code.

| Ứng viên | Bằng chứng và chỉ dẫn cho AI sửa |
|---|---|
| `E2EE_MANAGERS` và get_manager | Map giữ manager nhưng mỗi call trả connection mới; thay bằng ownership model A08. Xóa registry cũ sau caller migration; không giữ cả hai. |
| `AccountSwitcher`, app field, placeholder workspace | Chưa có show caller. Nối component này thay vì tạo switcher thứ2; dọn empty block và allow(dead_code) sau acceptance. |
| `SafetyNumberDialog` | Có renderer nhưng không load/caller. Nối component hiện hữu; không tạo SafetyNumberModal trùng theo guide Claude. |
| `DocViewerModal`, scroll field và test-only call | Nối pipeline production rồi remove unused state; không xóa renderer/PDF requirement để gọi WP15 done. Search arrows hiện highlight nhưng chưa scroll_to_rect đúng result. |
| VideoPlayerState controls | Bỏ control inert khỏi UX cho tới backend có thật hoặc nối ngay; không giữ fake toggles. Không xóa native player đang chạy chỉ vì modal mới chưa nối. |
| AppLockGuard.authenticate/availability alternate path | Hợp nhất đường unlock thật; availability phải dùng ở policy actual. time_until_lock nên nối timer, không xóa chỉ để tránh dead-code. |
| profiles path helpers vs storage/session paths | Chọn canonical storage model, migrate caller/dữ liệu, public API compatibility review trước remove. |
| range_request | Chưa player caller; hoàn thiện một validated streaming adapter dùng chung download policy, bỏ helper song song sau migration. |
| `rand_core` direct dependency, cuối Cargo.toml comment “E2EE integration helper” | Tìm không thấy direct rand_core symbol usage trong src; chỉ là candidate. Chạy machete/check feature/target+vendor và license trước remove; transitive dependencies vẫn có thể cần. |
| TODO/changelog comments, allow(dead_code), duplicate routes | Inventory theo owner/task; không mass-delete tests/unsafe wrappers/platform cfg/snapshots. Test-only panic hoặc protocol MD5/AES-CBC không tự là rác. |

Mỗi cleanup task phải ghi: production/test/public caller trước/sau, target cfg, requirement được giữ, migration/backward compatibility và check đã chạy. Không gộp format toàn repository vào patch logic.

## 9. Kế hoạch thực thi cập nhật và tiêu chí đóng việc

### 9.1. Thứ tự và phụ thuộc

1. **SYS-01 ngay:** requirement→entrypoint→backend→event/state→test→native evidence matrix. Reconcile cả33 task mở và task đã tick (đặc biệt spec012). Ghi Unknown cho contract không có nguồn, không tự đổi scope.
2. **Hotfix correctness/security:** A01/A02, A14/A18/A19/A21/A23/A25/A26/A27/A31 theo module ownership. Chặn production E2EE unverified; không cố nối E3 vào stable gửi tin. Việc gate không đồng nghĩa hoàn tất full E2EE.
3. **SEC-01 research chạy song song sớm:** timebox spike1–2 ngày công để thu/đọc evidence, xác định entitlement/format/khả năng interop. Kết thúc bằng artifact và blocker, không bắt buộc giả vờ đã có contract sau timebox. Có thể làm SEC UI state/events và synthetic regressions độc lập.
4. **Ưu tiên WP15:** NET-01/TRANSFER-01/MEDIA-01..03 hoàn thiện đầu cuối; kèm REALTIME-01 A15..17 và dữ liệu A18. Không chuyển các hạng mục này thành optional polish.
5. **WP16 production:** sửa A03..13 theo contract đã chốt và independent vectors, sau đó payload/device/group integration; ACCOUNT-01 A20 và LOCK-01 A21/A22 làm song song trên boundary owner generation chung.
6. **Đóng scope hệ thống:** VOIP-01 A31/A32, LEGACY-01 A33/A34, REF-01/A36, release A35; optimization OPT01..08 và cleanup theo số đo. Nghiệm thu native/cross-client và release gate cuối.

Các nhóm có thể chạy song song nhưng không cùng sửa `gui/worker.rs`/`chat.rs` khi chưa phân ownership rõ. Không `git add .` gom worktree/session, không tự merge/release như side effect của audit.

### 9.2. Mẫu task AI có thể thực thi

```text
Task ID: <SYS/SEC/... hiện hữu> + <Axx/OPTxx>
Baseline SHA + files/symbols: ... (đọc lại số dòng nếu HEAD thay đổi)
Requirement và nguồn parity E1/E2/E3: ...
Trigger / expected / actual: ...
Loại: source-confirmed / reproduced / latent / acceptance gap
Phạm vi sửa nhỏ nhất + phần không đổi: ...
Schema/state migration + ownership/cancellation/idempotency: ...
Regression fail trước / pass sau: ...
Lệnh/check đã chạy + số test pass/fail/ignored: ...
Native/server scenario + build/device/official-version + kết quả: ...
Rollback/recovery và giới hạn còn lại: ...
Status: chưa làm / một phần / đã có mã / qua kiểm thử / xác nhận thiết bị
```

AI tiếp nhận phải đọc AGENTS/standards/ADR liên quan, định nghĩa struct và caller thật trước edit. Không đánh dấu done chỉ từ tên command, file hoặc một enum; không mở network/account mutation trong unit test. Checkpoint state bất định phải báo rõ, không retry mutation tùy tiện. Không sửa accepted ADR bằng rewrite; đề xuất superseding ADR có lý do nếu cần.

### 9.3. Regression matrix tối thiểu trước đóng full scope

| Nhóm | Trường hợp bắt buộc |
|---|---|
| Security/send | required/plain/unknown × payload/entrypoint; no-downgrade, ambiguous ACK, duplicate/replay, independent vectors, low-order inputs. |
| Crypto persistence | parser→verify, nonempty skipped map+reopen, exact OTP/SPK IDs, consume rollback, concurrent sends, identity-change, pending-handshake outbox. |
| Data | PK promotion với dependent FK/triggers; migration fresh/current/newer/corrupt; history/search/expiry/revoke/hidden privacy và dedup. |
| Realtime | auth/pong/data deadlines, saturated consumer, queue error recovery, stable-backoff reset, offline catch-up cursor commit sau persist. |
| Account/auth/lock | concurrent login attempts, A/B switching+late events, Hello async/cancel/unavailable/HWND, overdue input, locked read/inspector/native viewer privacy. |
| Media/transfer | partial leases, queue/header/body cancel, blocked launchable attachment, safe URL redirects/DNS/proxy, resume finalization/restart, player controls/Range, PDF/text budgets. |
| Legacy/native | Business/2FA invalid vs empty, LAN thực, group policy, phone import/codec, VoIP/AEC/H264/screenshare, DPI/IME/theme. |
| Release | cargo check/clippy/test, format và dependency/license review; tag↔SHA↔artifact↔version/checksum; ARM64 native smoke và manual evidence đúng build. |

“100%” chỉ khi mọi requirement đã chốt có acceptance evidence tương ứng. Một bài ignored hoặc không có thiết bị vẫn là gap; UI tồn tại hoặc CI xanh không thay thế interop. Không hứa phần mềm tuyệt đối không có lỗi. Không tự hoãn requirement đang mở sang phiên bản khác để tăng tỷ lệ hoàn thành.

## 10. Nguồn kỹ thuật và ràng buộc

- Repository `AGENTS.md`, `GLOSSARY.md`, `docs/standards/*`, `docs/ZALO_REFERENCE_ANALYSIS.md`, `docs/ZALO_FEATURE_API_GAPS.md`, roadmap/release/spec artifacts tại baseline trên.
- [Google DoH JSON API](https://developers.google.com/speed/public-dns/docs/doh/json): `type` là một RR type; dùng truy vấn riêng để lấy A và AAAA.
- [Signal Double Ratchet](https://signal.org/docs/specifications/doubleratchet/): đối chiếu state/error/skipped-key semantics, không tự nâng protocol Zalo theo spec revision mới.
- [Microsoft UserConsentVerifier](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier?view=winrt-26100): hướng dẫn desktop sử dụng window handle và window-scoped interop; verify adapter Rust/native thật, không chỉ copy ví dụ .NET.

Olaz/libsignal-protocol-javascript is GPL-3.0 and used **for reference/algorithm study only — no code copied**. ZaloLite E2EE implementations are written in Rust from the Standard Signal Protocol spec.

Không sao chép mã GPL/whole artifact thiếu đánh giá license vào dự án permissive. Việc tham khảo Olaz và bảng450 IPC không tự xác minh API/WS/E2EE, không thay thế capture official và không cho phép lưu credentials trong fixture.
