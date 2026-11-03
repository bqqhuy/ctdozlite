# Control queue drain — 03/11/2026

Bằng chứng từ mã 76fe200 và log do người dùng cung cấp, không lưu log thô vào repository:

- Danh sách hội thoại nhận idx 0/1 và dấu cuối.
- Message round ghi 154 descriptor, max idx 185; không có isLast. 32 idx chưa xuất hiện trong log ở dải 0..185. Log admission cũ là best effort nên không được đồng nhất tuyệt đối thiếu record với thiếu control runtime.
- idx 185 lúc 13:39:24.423, idx 74 lúc 13:39:27.394 giờ Việt Nam; chênh 2,971 giây. Failed lúc 13:41:27.431, sau control cuối 120,037 giây.
- R2 socket whitelist bao gồm CMD_CTRL_PULL_CTRL=603; CatchupGuard cắt chuỗi sau 64 cursor dù tiến, không timer resume. Whitelist còn không follow queue 610 trong fixture auth có sẵn.
- Tái hiện bằng module guard cũ: 64 continuation được chấp nhận, phản hồi sau đó bị chặn dù cursor khác nhau. Không có bằng chứng log rằng đúng 64 trang 603 đã tới trong lần lỗi thật.

Thay đổi chỉ vào việc đọc tiếp các queue đã được server cấp và chẩn đoán. Giữ pull_frame(first=true,lastId=1,preIds=[]), pull_next_frame(first=false,lastId,preIds), ID lệnh, subCmd, auth, mã hóa/signing hiện có. Đọc lại dùng cursor cuối đã biết của request, không tạo cursor giả. Các frame có reqId cũ không điều khiển request đang chờ mới.

Theo shared-worker Zalo PC 26.11.10, checkRequestfulfilled vẫn dựa idx/isLast. takeUndeliveredScopes là cơ chế request-scoped có điều kiện, chưa port. Không dùng mặc định from=0/to=MAX làm bằng chứng đã nhập toàn bộ.

Nghiệm thu thủ công: Windows ARM64 + iPhone; mở Zalo trên điện thoại, xác nhận sync; ghi nhận phase/round summary/queue decision/batch stages/dispose. Cần đủ mọi idx và import xong, không chỉ nhìn thấy tổng. Hủy và chạy lại phải không còn downloader/receiver cũ. Tránh xóa dữ liệu test đang lỗi trước khi lấy log; không đưa session, URL hoặc nội dung chat vào báo cáo.
