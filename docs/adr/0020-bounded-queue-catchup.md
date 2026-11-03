# ADR-0020: Giới hạn lấy tiếp hàng đợi khi cursor không tiến

- **Status:** accepted
- **Date:** 2026-11-03
- **Deciders:** Yêu cầu bổ sung bản thử ARM64

## Context

PR upstream #69/#70 có catch-up HTTP/WS, nhưng ghi nhận thực tế trong PR cho thấy một queue có thể lặp trang hàng nghìn lần khi cursor đứng yên. Đây là hàng đợi tin nhắn máy chủ, không phải phiên sync điện thoại. Fork trước đây giới hạn HTTP ba trang mỗi nhịp, nhưng vẫn có thể yêu cầu lại ở nhịp kế; WS tự lấy tiếp khi `more=1` không có giới hạn.

## Decision

Mỗi bộ nhận HTTP hoặc queue WS giữ tập cursor đã dùng trong RAM. Chỉ tiếp tục khi cursor thay đổi và chưa xuất hiện trong chuỗi hiện tại; tối đa 64 cursor được chấp nhận để lấy tiếp trong một chuỗi. Cursor gồm ID và các boundary IDs, chuẩn hóa số/chuỗi và thứ tự tập ID; không lấy nội dung tin hoặc reqId làm bằng chứng tiến triển. Cursor thiếu, lặp/quay vòng hoặc vượt ngân sách làm chuỗi nghỉ 30 giây. Không tăng cursor giả, không xóa dữ liệu hay checkpoint, không tự yêu cầu đồng bộ điện thoại.

HTTP vẫn có tối đa ba trang mỗi lần poll; khi nghỉ không gửi HTTP request cho receiver đó. WS dừng chuỗi pull-next tương ứng nhưng tiếp tục nhận push, control và queue khác. Sau thời gian nghỉ, phản hồi WS tiếp theo có thể tiếp tục; không tạo request timer riêng để tự khởi động lại WS pull. HTTP là đường nhận dự phòng hiện có. Giới hạn này không chứng minh hay sửa khoảng trống lịch sử trên máy chủ.

## Consequences

- Chặn vòng lặp vô hạn và bộ nhớ theo chuỗi có giới hạn.
- Trường hợp backlog rất lớn hoặc cursor khác thường có thể chậm tối đa thời gian nghỉ; cần thử tài khoản thật.
- Không port cơ chế ghi cursor định kỳ của PR: cursor bền vững chỉ được bổ sung khi có thể gắn ACK với transaction ghi tin, tránh nhảy qua batch chưa commit.

## Verification

Fixture tổng hợp kiểm tra cursor lặp/quay vòng/thiếu, reset lặp, boundary IDs, chuỗi hoàn tất và ngân sách 64 cursor. Benchmark giả lập 1.800 phản hồi giống nhau so với guard; không gọi mạng hoặc dùng tài khoản thật.
