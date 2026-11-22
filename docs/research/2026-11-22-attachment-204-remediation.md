# Xử lý gửi tệp và mã 204 — 22/11/2026

Phạm vi: sửa luồng đính kèm đang được người dùng phản hồi, trên working tree
`main` tại `8e235f7`. Giữ các thay đổi đã có, không commit/push/release.

## Nguyên nhân có bằng chứng

Log xác nhận 204 là mã từ chối Zalo, không phải HTTP 204. Sáu job đính kèm lỗi
còn lưu gần đây gồm năm EXE và một MSI, đều chat cá nhân. Locale PC gốc mô tả
204 là không hỗ trợ định dạng. Chưa có capture phân biệt bước upload/gửi của
những lần cũ; không khẳng định mọi tệp PDF/ZIP/ảnh cũng bị cùng lỗi.

Phát hiện mã nguồn độc lập: fallback video thành tệp đặt `fType=2`, trong khi
PC/Web dùng 2 cho folder và 1 cho regular file. Đây là sai lệch cụ thể cần sửa,
không phải kết luận nó gây tất cả mẫu lỗi EXE/MSI.

Nguồn, vị trí mã và giới hạn: [re-notes/71](../../re-notes/71-attachment-204-and-regular-file-contract.md).

## Thay đổi

- Hiển thị đúng lý do 204 và hướng đóng gói ZIP; không đổi tên đuôi tệp để gửi.
- Nút **Tạo ZIP…** ở tệp gửi lỗi nối tới worker → đọc/đóng gói → event có
  owner/runtime/thread → attachment draft. Người dùng kiểm tra rồi bấm Gửi.
  Không gửi từ event, không xóa/sửa tệp gốc và không ghi đè văn bản đang soạn.
- ZIP giữ nguyên entry name và byte dữ liệu, dùng buffer giới hạn; kiểm tra
  nguồn thay đổi/kích thước và lưu vào temp riêng của account. Stored ZIP không
  giảm dung lượng. Thêm thư viện Rust `zip` pin 2.4.2, tắt default features để
  dùng ZIP writer/CRC chuẩn thay vì tự viết format archive.
- Fallback video theo regular-file protocol và render là File.
- Chẩn đoán mới phân biệt `photo_upload`, `photo_send`, `file_upload`,
  `file_send`, giữ typed refusal và không log tên/đường dẫn/nội dung/URL.

## Kiểm chứng

- `cargo check --locked -j 1`: qua.
- `cargo test --locked --lib -j 1`: **1052 passed, 0 failed, 11 ignored**.
  Gồm ZIP giữ tên Unicode/byte gốc, từ chối nguồn thay đổi, event ZIP chỉ tạo
  draft cho đúng account/runtime/thread và typed stage/refusal diagnostics.
- `cargo test --locked --test audit_regressions -j 1`: **33 passed, 0 failed**;
  kiểm tra DM/group, regular-file fallback, upload/submit 204 không tự lặp,
  cùng các fixture nhiều chunk/checksum đã có.
- `cargo clippy --all-targets --locked -j 1 -- -D warnings`, fmt và diff check: qua.
- `cargo build --locked --bin ctdozlite -j 1`: qua, debug ARM64 tại
  `target/debug/ctdozlite.exe`.
- Hai renderer smoke dữ liệu giả ở scale 1 và 1.5 đều exit 0, đã xem ảnh trong
  `target/attachment-204-verification-20261122/`. Scale 1 hiển thị đủ thông báo
  204 và nút ZIP. Scale 1.5 tại cửa sổ 1020×700 có header chồng và timeline
  chỉ thấy phần cuối của lỗi; không coi đây là nghiệm thu layout DPI cao.

Ứng dụng đã mở trước build vẫn chạy binary cũ được đổi tên trong `target/debug`;
không đóng tiến trình hay đổi phiên tài khoản. Cần mở binary mới để thử bản sửa.

Nghiệm thu thật còn cần: gửi TXT/PDF/ZIP và ảnh/video tới đích thử được phép,
xác nhận ACK và tải lại bằng Zalo gốc để so checksum; EXE/MSI bị từ chối phải
hiện đúng lý do, tạo ZIP vào draft rồi gửi có chủ đích. Kiểm tra cả DM/group,
tệp nhiều chunk, đổi tài khoản và lỗi mạng sau upload/submit.

Không coi fixture loopback hoặc ZIP roundtrip là chứng nhận server chấp nhận.

## Thử tài khoản thật tiếp theo trong cùng chat

Người dùng mở binary mới và cho phép thử tệp sẵn trong Downloads vào Cloud của
tôi, sau đó tự thao tác GUI và yêu cầu assistant chỉ theo dõi log.

- GUI đã hiển thị ZIP 18,7 MB ở trạng thái Đã gửi.
- Hai lỗi installer mới được log xác định ở **file_upload / server_code 204**,
  không phải file_send hoặc HTTP 204.
- SQLite đọc-only xác nhận một PS1 và ZIP tạo từ MSI ở trạng thái
  **server_accepted**, có server reference và download metadata.
- Ba ZIP tạo tại account temp (hai lần MSI, một EXE) đã so SHA-256 entry với
  tệp gốc: byte trùng. Đây là kiểm tra ZIP trước upload, chưa phải tải lại.
- Zalo PC gốc đang logged out; chưa đối chiếu hiển thị/tải lại trên client đó.
  Chờ tệp tải lại để so checksum; không tự nâng thành nghiệm thu hoàn tất.
- Hộp chọn tệp phản hồi chậm/không xuất hiện trong các snapshot đầu. Có thao tác
  người dùng đồng thời nên chưa chốt đây là lỗi picker độc lập.

Kết luận có bằng chứng: ZIP và script gửi được trong ca thử; installer thô
bị từ chối định dạng. Chưa đủ để kết luận mọi tệp/nhóm/điều kiện mạng đã đạt.

## Bổ sung: Zalo PC có tự đóng gói executable

Truy tiếp worker gốc tìm được cơ chế EECX/EEXC: danh sách mặc định gồm
exe/cmd/bat/com/lnk/vbs, được server mở rộng qua restricted_ext_file. Khi cờ
enable_file_eecx bật, PC tạo ZIP thật mức nén 0, tên ngoài thêm `.zip`, upload
archive với file fType 1 và fdata.type 2 (enum khác với folder fType 2).
Vì vậy ZIP draft hiện tại là bước xử lý thực dụng đã gửi thành công, chưa phải
parity đầy đủ của native auto-pack. Không tự sửa protocol theo suy đoán giải
nén: phần nhận EEXC cần đối chiếu tin EXE từ client gốc mà người dùng sẽ gửi.
Chi tiết source/offset/hash: [re-notes/72](../../re-notes/72-restricted-extensions-and-native-executable-zip.md).

## Bubble hiện trước upload — delta sau thử PC gốc

Người dùng xác nhận PC tự chuyển EXE thành ZIP và hiện bubble tên/icon trước
trạng thái tải/gửi. Worker fork trước đây chờ persist_outgoing rồi mới phát
OutgoingState, và chờ chuẩn bị tệp trước mới công bố tệp tiếp theo. Sửa công bố
toàn bộ lô (giới hạn hiện có 50) trước semaphore/SQLite/metadata, giữ Active và
client ID trong luồng gửi. Disk/network vẫn chạy ở worker; chỉ server ACK mới
chuyển trạng thái Đã gửi. Tệp có intent cục bộ chưa có download URL không còn
hiện dòng Không còn đường dẫn tải tệp; media nhận về mất URL vẫn giữ cảnh báo.

Test hồi quy mới dùng 20 đường dẫn chưa tồn tại, nhận đủ payload pending có
tên/icon-kind/owner/runtime/thread/ID trước bất kỳ thao tác file/upload nào: qua.
`cargo test --locked --lib -j1`: **1053 passed, 0 failed, 11 ignored**.
Clippy all-targets với -D warnings, build debug ARM64, fmt và diff check: qua.
Renderer smoke cách ly dữ liệu exit 0, đã xem
`target/attachment-204-verification-20261122/bubble-before-upload.png`: bubble
tệp không còn cảnh báo sai mất URL. Ảnh tĩnh không chứng minh độ trễ; chưa có
nghiệm thu thời điểm bubble trên tài khoản thật của binary chứa delta này.

## Thứ tự tiếp tục từ phân tích PC/Web/Android

1. Khép nghiệm thu gửi tệp và thu mã lỗi theo chặng trên binary mới.
2. Alias: hydrate `enable_feature_alias_v2`, chọn API/phonebook theo phiên;
   đối chiếu contact có tên phonebook thật trước khi gọi là xong.
3. Composer: phân biệt legacy/column và geometry của từng box; đo cùng
   viewport/DPI thay vì thay 1/2 bằng 2/3 một cách máy móc.
4. Voice: nguồn m4a/AMR/AAC, seek/recovery và AI PC HTTP; giữ packet Android
   ngoài hợp đồng PC. Recorder/upload native nghiệm thu riêng.
