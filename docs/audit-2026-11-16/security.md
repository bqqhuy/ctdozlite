# Audit độc lập: bảo mật, giao thức và transport

Ngày: 2026-11-16. Snapshot: `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`, nhánh main. Chỉ đọc nguồn; không gọi Zalo, không đọc session/database người dùng, không sửa code, không chạy cargo (parent phụ trách).

## Phạm vi và cách kiểm

Đọc chuẩn security/cryptography, API/error handling, concurrency, reference authority; rà session.rs, login.rs, qr.rs, socket.rs, updater.rs, storage.rs, logger.rs, diagnostics.rs, crypto/{session,store,prekey,keystore,ratchet,wire}. Những kết luận dưới là phân tích nguồn hiện tại; không phải nghiệm thu thiết bị hay bằng chứng E1 về server.

## SEC-01 — P2: WebSocket write không deadline làm vô hiệu liveness/cancellation

Nguồn: `src/socket.rs:857`, `:898`, `:974`, `:1017`; outer task `:637-695`.

Điều kiện: một write heartbeat/Pong/first ping/GET_MSG chờ socket writable khi peer/path không tiếp nhận dữ liệu. Các `.send(...).await` này nằm bên trong handler của `tokio::select!`, không được timeout. Sau khi một arm được chọn, select không còn poll `tx.closed()` hay liveness deadline đến khi handler xong. Task spawn bị bỏ JoinHandle và outer loop await trực tiếp run_once, không có timeout bao ngoài. Vì thế việc drop receiver hoặc hết liveness deadline không bảo đảm reconnect/cancel đúng hạn; có thể giữ task/connection cũ và mất realtime.

Đối chứng: một số write khác đã có timeout 5s (`:844`, `:868`, `:985`, `:1028`); chúng không bao bọc các call còn thiếu. Không khẳng định mạng thông thường luôn tái hiện; đây là lỗi conditional với backpressure/half-dead write.

Đề xuất: helper write có deadline chung và cancellation; kiểm offline bằng sink cố ý Pending vô hạn, assert deadline và receiver-close chấm dứt. Phạm vi parity: cơ chế giới hạn nội bộ, không đổi server contract.

## SEC-02 — P2: Diagnostic QR in raw response chứa dữ liệu nhạy cảm

Nguồn: `src/qr.rs:406`; raw error paths `:155`, `:161`, `:247`, `:275`, `:282`; `src/session.rs:1413-1415`.

`get_raw` in trực tiếp `truncate(&text,200)` khi `s.verbose`; hàm phục vụ cả reqqr, scan và confirm. `extract_zpw_sek` ngay cùng module hỗ trợ session token trong JSON ở top/data/data.account, nên đây là dữ liệu có thể chứa credential chứ không chỉ metadata. Prefix 200 byte không phải redaction; token đặt sớm trong JSON vẫn bị in nguyên. `redact_secret` còn giữ 8 ký tự đầu, nên không che hoàn toàn token ngắn. Các lỗi raw response cũng có thể mang nội dung server vào thông báo.

Đối chứng: logger.rs có diagnostics::preview nhưng nhánh QR này dùng eprintln trực tiếp, nên bộ scrub logger không bảo vệ. Main cấm dev khi E2EE trial nhưng không cấm verbose cho QR login bình thường. Chưa thu log thực tế, không khẳng định đã có sự cố lộ credential.

Đề xuất: chỉ log status/size/error code, hoặc scrub cấu trúc trước truncate, dùng synthetic JSON có zpw_sek/token/phone/name ở đầu chuỗi để kiểm. Chuẩn repository cấm secret trong log kể cả dev. Không cần thay giao thức.

## SEC-03 — P2, giới hạn E2EE experimental: verifier không gắn signing key với DH identity

Nguồn: `src/crypto/prekey.rs:79-87`; `src/crypto/session.rs:557-576`, `:308-345`.

Parser chấp nhận signingIdentityKey độc lập; verifier lấy key này thay cho identity_key và chỉ kiểm chữ ký SPK. start_session sau đó X3DH và trust bằng identity_key khác. Do đó đổi identity_key trong bundle nhưng giữ signingIdentityKey, SPK, signature không làm verify_signed_prekey đổi kết quả. Bài test keystore hiện chỉ đổi signingIdentityKey nên không kiểm hướng thay identity_key. Kết luận chắc chắn ở mức local verifier: chữ ký không chứng thực DH identity đang được lưu trust; không khẳng định có exploit server hay MITM khả thi qua TLS.

Nếu hỗ trợ hai loại identity key là chủ ý, cần bằng chứng ràng buộc hai key (certificate/signature over both hoặc phép chuyển key kiểm chứng), không chỉ hai trường độc lập. Nếu contract chỉ một Curve25519 identity, từ chối mismatch và bỏ fallback production không có bằng chứng. Thêm negative test: tạo bundle hợp lệ, thay identity_key bằng key hợp lệ khác, giữ mọi phần còn lại, verify phải từ chối.

Parity: Unknown — AMBIGUOUS. Nguồn ghi schema synthetic/unverified; chỉ trial bật cả build/runtime mới đi network. Không nâng E3 thành hợp đồng Zalo.

## SEC-04 — P3: Salt password dùng time-seeded PRNG trái chuẩn CSPRNG

Nguồn: `src/login.rs:72-91`, caller `:188`; `re-notes/01_login.md:73-77` chỉ yêu cầu randomString(8).

Salt được sinh bằng LCG từ timestamp nanosecond XOR process ID, không CSPRNG. Đây là vi phạm rõ `security-and-cryptography.md` mục randomness; không có yêu cầu parity buộc LCG. Salt không phải khóa bí mật nên chưa đủ bằng chứng để kết luận giải mã password hay chiếm tài khoản.

Đề xuất dùng random::fill/OS RNG rồi chọn ký tự với rejection sampling; giữ độ dài và alphabet tương thích. Kiểm xử lý RNG failure thay vì fallback deterministic. Không đổi cipher/sign flow.

## Phản biện chéo

- Chấp nhận GUI voice leak P1: gui.rs LoggedOut/ProfileSwitched không hủy voice_recorder/voice_preview/recorded_voice_samples; workspace.rs7025 gửi samples không có owner sang thread/tài khoản hiện tại. Đây là account-isolation leak có đường thực thi, không chỉ code style.
- Chấp nhận reaction toggle P2: workspace.rs3050-3074 xóa reaction local nhưng WorkerCmd luôn chứa reaction không rỗng; chat.rs4263 quy định bỏ reaction là empty hoặc /-clear.
- Chấp nhận TUI delete P1 theo đồng thuận cuối: tui.rs1478 chỉ retain vector rồi persist snapshot; không gọi store delete/tombstone. Upsert không xóa row vắng, nên hydrate có thể khôi phục. Mức P2 sơ bộ được nâng vì đây là lỗi riêng tư của thao tác xóa đã báo thành công.
- Chấp nhận hidden PIN bypass trong TUI ở mức privacy display gate: ADR0034 không miễn TUI; yêu cầu giữ chat ẩn khóa offline, trong khi TUI không thực thi hidden gate. Không gọi đây là phá mã hóa DB vì ADR nói rõ DB/file không được mã hóa.

## Không ghi nhận thành lỗi chắc chắn

Không coi MD5 signkey/AES-CBC zero-IV hiện hữu là lỗi khi đây là compatibility contract. Không coi route index là schema đã chứng minh. Không khẳng định Signal protobuf/truncated MAC đảm bảo interoperable chỉ vì roundtrip local pass. Không gọi các key-public comparisons là secret timing bug. Không chạy audit dependency online hay công bố kết quả CVE chưa kiểm.
