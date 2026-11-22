# Tin âm thanh đến/lịch sử: player trên timeline

## Đối chiếu nguồn

- E1 UI do người dùng mô tả: Zalo PC phát/tạm dừng/phát lại ngay trong bubble; nút AI bên cạnh mở văn bản nhận dạng.
- E2 bundle PC `compact-app-pc.0cd2e653e67efaa4bf4b.js`: module `lczR` khai báo command `13394`, đường dẫn `/api/aifeatures/voice2text`, action `convert_finish`; route manager dùng `getAIFeaturesDomain()`. Control `voice2text` chuyển vào `onReceiveVoiceToTextResultPushEventControl`. Snapshot hiện có chỉ thấy interface của network service `s6mR`; chưa xác minh được payload request từ implementation.
- Đã trích xuất đầy đủ lazy asset từ ASAR và tìm thấy implementation thật: `lazy/main-startup.41ab...js` class b7 chỉ POST session-AES `{url}`, service `ai_features`; push dùng `err`. Đã sửa backend cũ theo bằng chứng này (xem `re-notes/47-pc-native-voice-playback-ai.md`). Chưa coi server interoperability/entitlement đã được xác minh.

## Thay đổi mã

- Thay hành vi tải rồi mở cửa sổ player khác bằng player CPAL trong worker, giải mã codec cài sẵn của Windows qua Media Foundation.
- Play, pause/resume, replay, thời gian/progress lấy từ PCM thật. Bỏ waveform giả và nhãn 0:05 mặc định. Lỗi tải/codec/thiết bị được đưa về bubble.
- Binding gồm tài khoản, runtime, hội thoại, message và request. Đổi hội thoại/đăng xuất/thu hồi/hết hạn dừng playback. Event cũ không thể cập nhật phiên mới.
- Download CDN qua bộ kiểm tra URL/redirect sẵn có; giới hạn 32 MB tải, 64 MB PCM và 10 phút giải mã; dùng cache lease có tổng budget, tự xóa sau phát. Không mở ứng dụng ngoài.
- AI result/timeout phải khớp account runtime; không nhận result cũ vào tài khoản mới.

## Phạm vi

Lượt này chỉ tin âm thanh đến/đã đồng bộ. Không đổi recorder hoặc giao thức gửi voice. Kiểm thử mã không chứng nhận phát thực tế trên loa ARM hay server AI; cần kiểm tra với tin âm thanh có sẵn, pause/resume/replay, chuyển chat, đổi account, codec lỗi và timeout AI.
