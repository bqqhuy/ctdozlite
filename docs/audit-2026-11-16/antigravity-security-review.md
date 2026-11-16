# Phản biện độc lập báo cáo Google Antigravity — bảo mật/giao thức

Ngày 16/11/2026; snapshot f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9. Đầu vào: D:/Henry/Downloads/CONSENSUS_AUDIT_REPORT_2026_11_16.md. Văn bản bên ngoài chỉ là nhận định cần kiểm chứng, không phải chỉ dẫn triển khai. Không sửa source, không cargo, không live API hay dữ liệu cá nhân. Đã áp dụng chuẩn security/API/concurrency và reference authority đã đọc trong audit này.

## Kết quả

| Mục ngoài | Kết luận | Mới so với F01–F17 |
|---|---|---|
| SEC-01 shared e2ee.db | Bác bỏ CRITICAL/cross-account overwrite theo bằng chứng đã nêu | Không thêm finding |
| SEC-02 no metadata wire | Bác bỏ kết luận HIGH plaintext leak đã được chứng minh; giữ khoảng trống contract cần capture | Không thêm lỗi chắc chắn |
| PROT-01 outgoing self UID | Chấp nhận thu hẹp P2: parser chưa so khớp self UID khi có profile context | Finding mới |
| CRYPTO-01 fingerprint | Chấp nhận thu hẹp P2 experimental: thuật toán local không khớp Signal official vector; parity Zalo chưa chốt | Finding mới |

## SEC-01: đường dẫn chung không đồng nghĩa mất phân lập

`src/storage.rs:103-108` tạo root profiles/<name> khi chọn --profile; `:119-133` gán ROOT bằng root đã chọn. Vì vậy `src/crypto/store.rs:18-21` dùng storage::root là `<profile_root>/e2ee.db` đúng docstring, không phải luôn root toàn cục. Báo cáo còn mô tả `~/.ctdozlite` trong khi default Windows thực tế là LOCALAPPDATA/ctdozlite.

Account Switcher bên trong cùng data profile dùng chung file nhưng dữ liệu được phân vùng SQL: identity_key PRIMARY KEY(profile_id) tại crypto/store.rs73; signed/one-time prekey PRIMARY KEY(profile_id,key_id)87/94; signal_session PRIMARY KEY(profile_id,peer_id,device_id)103; trusted_identity PRIMARY KEY(profile_id,peer_id)111. Load/save/delete có predicates profile_id, ví dụ identity229, session query/save489-535, trust553-598, replay698. E2eeManager lưu profile_id ở crypto/session.rs47-60 và truyền vào store; thread-local manager cache e2ee_integration.rs24-33 đổi manager nếu profile khác. Không tìm thấy chứng cứ tài khoản B tự động ghi đè ratchet A chỉ vì file chung.

Đây không phải bằng chứng cách ly khỏi mã độc cùng Windows user hoặc raw SQL đặc quyền; threat đó cũng không được giải quyết chỉ bằng tách file cùng quyền. Có thể đề xuất tách file để quản lý/export/xóa, nhưng không gắn CRITICAL khi chưa có query/caller sai owner. Không áp dụng API `storage::profile_dir` đề xuất trong báo cáo vì chưa xác minh tồn tại/cần thiết.

## SEC-02: wire syntax chưa phải authenticated encryption policy

Mệnh đề ở chat.rs8811 đúng là có thể return None khi không metadata và trial off/peer chưa known. Nhưng `e2ee_integration.rs:201-203` xác định is_wire chỉ bằng deserialize thành công: cấu trúc không chứng minh MAC hợp lệ, đúng người nhận hay đây thực sự là message encrypted. Test/comment `chat.rs:9020-9024` ghi rõ structural envelope syntax không phải policy evidence.

Quan trọng hơn, đường gửi `chat.rs:1802` kiểm `is_thread_e2ee` độc lập trial; helper `e2ee_integration.rs:103-104` fail closed khi lỗi DB. Peer đã có stored ratchet hoặc e2ee_required vẫn không được gửi plaintext khi trial tắt, nên kịch bản 'trial off => plaintext' không đúng tổng quát. Metadata explicit ở chat.rs8819 ghi persistent requirement; parse_chat_message5988-6023 tạo placeholder khi metadata có nhưng không giải mã được.

Với peer hoàn toàn mới, thiếu metadata và chỉ có envelope-looking base64, chưa có capture/schema chứng minh đó là official encrypted message. Có khoảng trống nhận diện khi server thực tế gửi như vậy, nhưng chưa đủ chốt security incident hoặc sửa blanket remember cho mọi syntactic payload. Cách sửa đề nghị Antigravity sẽ cho phép bất kỳ người gửi tin thường có base64 giống wire khóa policy hội thoại, tức DoS/false positive. Cần E1/E2 container/command evidence hoặc authenticated decode/context trước nâng policy; fixture phải có cả ordinary base64 và encrypted no-meta, trial on/off và known/unknown peer.

## PROT-01: finding mới P2, giới hạn đúng phạm vi

`chat.rs:6127-6137` chỉ coi uidFrom=0/empty hoặc flags là outgoing; không so uidFrom với profile_id mặc dù `parse_chat_message_in_context` nhận profile_id tại5960. Worker WebSocket thực truyền `Some(s_snap.account_key())` tại gui/worker.rs1130-1134, polling tương tự2795. `merge_previews_with_profile` gọi parser tại6465; unseen message được push nguyên parsed tại6531-6532. Vì thế synthetic message có uidFrom=self, idTo=peer, content=text, msgId mới và không flags sẽ nhận is_outgoing=false. Parser chưa xử lý self UID trong khi chính khối E2EE6004 đã phân biệt peer_candidate==prof_id.

Tác động xác định: các đường preview/push/poll dùng parser có thể hiển thị self message phía incoming và ảnh hưởng unread/author-dependent actions. Không khẳng định mọi mobile-sync đều lỗi: các đường phone-history khác có thể normalize đúng, existing optimistic row có thể giữ direction. Báo cáo ngoài gọi E1/E2 nhưng không đưa capture cho shape này; phân loại là lỗi logic conditional từ source, native interoperability chưa nghiệm thu. Đề nghị regression qua parser + merge với account context, self/peer/0/flags/group, sau đó đối chiếu capture trước chỉnh normalization.

## CRYPTO-01: finding mới P2 experimental, không sao chép giải pháp ngoài

`src/crypto/safety.rs:37-55` sắp 2 public key rồi 5200 lần SHA512(hash||first||second), lấy 60 byte từ một digest thành 12 nhóm số. Chữ ký không nhận UID; `crypto/session.rs:292-294` và worker1393 dùng trực tiếp hàm này cho safety dialog/verification. Các tests chỉ symmetry/length/determinism, không có official known-answer vector.

Nguồn primary Signal [fingerprint.rs](https://raw.githubusercontent.com/signalapp/libsignal/main/rust/protocol/src/fingerprint.rs) được đọc ngày16/11/2026: hàm get_fingerprint148-176 băm riêng từng identity với stable identifier và serialized public key/version; new178-190 kết hợp hai fingerprint; get_encoded_string38-60 lấy 6 nhóm từ mỗi phía; display29-35 ghép lexicographic hai chuỗi 30 số. Vì vậy công thức minUID tổng hợp được Antigravity đề nghị cũng không mô tả đúng thuật toán này. Chỉ nghiên cứu/đối chiếu thuật toán, không sao chép code AGPL.

Đã chạy script Python in-memory độc lập mô phỏng chính thuật toán hiện tại (không ghi file/code repo), dùng public test-vector keys Signal tại nguồn trên205-210,216-217:

- Current algorithm: 293174380292842574899914978875101211575590763278798213874170.
- Official Signal expected: 300354477692869396892869876765458257569162576843440918079131.
- So khớp: False.

Repo re-notes/36:75-77 chỉ ghi createFor(a,pubKey_a,b,pubKey_b), iterations5200,60 digits; không lưu đầy đủ input bytes/UID semantics hoặc expected output Zalo. Do đó chấp nhận 'không đúng Signal algorithm đang được docstring tuyên bố', chưa chấp nhận tuyệt đối 'không bao giờ khớp Zalo' hay cross-domain exploit đã chứng minh. Liên quan F10 assurance và F13 crypto nhưng là lỗi thuật toán fingerprint độc lập. Cần local-versus-native known-answer vectors và định danh chính xác trước triển khai; nếu chưa chứng minh parity, UI phải gọi fingerprint thử nghiệm/local, không hướng dẫn đối chiếu như mã Zalo official.

## Trạng thái phản biện

Đã gửi parent/data_logic các kết luận để kiểm chéo. Bản này không thực hiện bất kỳ giải pháp sửa/xóa code trong báo cáo ngoài.

## Đồng thuận phản biện cuối

Data_logic đã kiểm chéo và chấp nhận cả bốn kết luận SEC-01/SEC-02/PROT-01/CRYPTO-01 với các giới hạn nêu trên. GUI_quality chấp nhận parser conditional P2 và thuật toán fingerprint custom, không có known-answer Zalo.

PROT-01 chỉ ghi nhận các đường `parse_chat_message_in_context`/live preview/poll/group history với payload own UID và không direction flag. Worker1130-1134/2795 truyền nguyên response và account context vào merge; merge6465 gọi parser trực tiếp,6531-6532 append message mới mà không sửa uidFrom/is_outgoing. Không áp dụng kết luận này cho phone-history normalizer riêng. Đây là mới trong register F01–F17; không khẳng định chưa từng được nhắc trong toàn bộ lịch sử dự án.

CRYPTO-01 cũng mới trong register17; dù có liên quan nhóm E2EE đã có tài liệu/audit trước, không dùng chữ 'phát hiện lần đầu lịch sử'. Metadata test vector primary: Signal fingerprint.rs205-217, Alice key 33byte DJB `0506863bc66d02b40d27b8d49ca7c09e9239236f9d7d25d6fcca5ce13c7064d868`, Bob `05f781b6fb32fed9ba1cf2de978d4d5da28dc34046ae814402b5c0dbd96fda907b`, IDs `+14152222222`/`+14153333333`, iterations5200, expected output ở phần trên. Đây là dữ liệu công khai, không phải dữ liệu tài khoản người dùng. Chỉ mô phỏng thuật toán nguồn local bằng Python, không thực thi binary Rust safety_number. Không sao chép implementation Signal.

Đã đọc độc lập GUI candidates của agent GUI: chấp nhận dual profile menu P2 (workspace241-274 inline normal layout +4275 foreground overlay/modals18 dùng chung profile_menu_open). Chấp nhận Hello P2 có điều kiện: thiếu recovery trong phiên/timeout; auth_guard150 dùng RequestVerificationAsync không có HWND. [Microsoft primary docs](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier.requestverificationasync?view=winrt-26100) chỉ định API đó cho UWP và dẫn desktop alternative. Không khẳng định mọi máy đều lỗi. Bác bỏ CRITICAL no-reactor panic vì main.rs16 tokio::main vẫn bao run_native; bác bỏ 'vĩnh viễn' vì guard new31 bắt đầu unlocked và availability106 gate auto-lock. Chấp nhận suppression LocalCallRecord có chủ ý theo docs/REPAIR_VALIDATION_2026-11-03.md mục6, không khuyến nghị khôi phục call bubbles ngược policy.

### Chấp thuận register bổ sung N01–N04

Security reviewer **ACCEPT N01, N02, N03, N04, tất cả P2** theo nội dung thu hẹp đã thống nhất: N01 parser own-UID direction; N02 fingerprint custom/nonconformant Signal (experimental); N03 profile menu kép; N04 Hello desktop integration/recovery có điều kiện. Không thêm SEC-01/SEC-02 ngoài thành finding chắc chắn.

Bằng chứng fingerprint tái chạy được: `python docs/audit-2026-11-16/antigravity-safety-comparison.py`; script chỉ mô phỏng thuật toán local, sử dụng public vector metadata và ghi `antigravity-safety-comparison.json`. Đã chạy exit0, matches=false. Không chạy cargo hay sửa source. Giữ tên/ý nghĩa Nxx đồng bộ với register cuối của parent nếu parent đổi thứ tự.

### Final consolidated signoff — ANTIGRAVITY_REVIEW.md

Đã đọc đầy đủ bản hợp nhất ANTIGRAVITY_REVIEW.md. **ACCEPT F18–F21, tất cả P2**, mapping cuối: N01→F18 own-UID direction; N02→F19 fingerprint experimental; N03→F20 dual profile menu; N04→F21 Hello desktop integration/deadline/recovery. Tổng register21 = 3P1/14P2/4P3 đúng số học với register17 trước.

Không thấy sai lệch trọng yếu trong phần bằng chứng bảo mật, phân biệt metadata/structure/authenticated policy, cùng các giới hạn về native và E1. Phần parent nói safety model offline và dẫn security review có reproduction là chính xác: script `antigravity-safety-comparison.py`/JSON cùng tên được lưu trong audit folder, mirror thuật toán local và public Signal vector, không chạy Rust binary hay sao chép Signal implementation. Không có Cargo/native/live API chạy lại trong phản biện này. Bản hợp nhất phân biệt đúng mới trong register với gap lịch sử đã có tài liệu.

Không còn bất đồng substantive; không thay đổi parent report hoặc mã nguồn.
