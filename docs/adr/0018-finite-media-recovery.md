# ADR-0018: Phục hồi media có giới hạn và cache theo định danh

- Status: accepted
- Date: 2026-11-03
- Supersedes: ADR-0011 retry-after-error policy; extends ADR-0015.

## Context

Người dùng chỉ thấy biểu tượng ảnh/video, bấm không có phản hồi. URL có thể thiếu, không truy cập được hoặc body không phải media; không đủ bằng chứng nói tệp bị xóa. FILE renewLink được xác nhận trong mã PC; HTTP 200/JSON lỗi được xử lý theo cấu trúc, không giả định chuỗi Invalid signature hay một nguyên nhân hết hạn duy nhất.

## Decision

Phân loại network/timeout/HTTP/JSON/content/decode/local; tự retry chỉ lỗi tạm thời, tối đa ba lần giãn thời gian. Lỗi cố định giữ trạng thái tới khi người dùng thử lại. Card/viewer có trạng thái lỗi; click thiếu URL báo rõ. JSON attachment hợp lệ vẫn lưu bình thường; chỉ visual media mới bị từ chối khi body là JSON/HTML lỗi.

Renew một lần cho URL không truy cập được hoặc JSON lỗi có cấu trúc, gộp theo account/thread/message, cache kết quả ngắn 60 giây, giới hạn 128 flight. Không coi mọi JSON, lỗi giải mã, hoặc thất bại renew là mất tệp. Giữ nguồn lịch sử gốc và lưu projection renewed_media riêng, cập nhật có guard account/recall; replay cũ không ghi đè projection mới. Cache thumbnail/preview dùng hash định danh thread/message/attachment-index/quality, bên trong namespace account; chưa triển khai UI nhiều attachment trong một tin.

Không đính kèm cookie vào CDN. URL mới phải qua allowlist/redirect policy. Renewal không suy đoán khóa E2EE. Media không đủ client ID hoặc adapter được báo lỗi rõ. Video cache là playback copy tạm, lease loại trừ active viewer khi dọn; tổng byte admission trong process 512 MiB, đóng viewer giải phóng. User-saved files không bị prune và không thuộc ngân sách này.

zCloud khác cuộc hội thoại Cloud của tôi. Không thêm cơ chế zCloud hay cam kết lấy lại tệp trước khi thử một tệp thật trên account người dùng. Không tự chạy API zCloud trong kiểm thử.

## Verification and rollback

Fixture JSON bất kể message string, finite retry, HTTP 200 JSON cho video nhưng JSON file vẫn lưu, cache key tách thread/attachment/quality, renewal URL allowlist, giữ metadata/recall. Thử thực tế đúng file còn mở trên Zalo chính thức. Rollback không xóa user-saved files hoặc lịch sử.

## Clarification after resumed verification

Production diagnostics use typed transfer_diagnostics events, a bounded mailbox and rotated JSONL files under diagnostics; logger::log_error alone is dev-only. Only counts, numeric codes, enums and a hashed session tag are accepted. The ordinary redacted diagnostics module is preserved.

Media-download allowlist is separate from photo recognition and authenticated destinations. It adds dlmd.me/dlfl.vn confirmed in PC connect_src_rgx and PR #70. Legacy URL/filename projections do not rewrite source rows or reclassify ordinary text. Other offline-catch-up features in unmerged PRs remain separate proposals.
