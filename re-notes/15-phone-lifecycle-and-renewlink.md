# Phone lifecycle and FILE renewal — 2026-11-03

Nguồn tĩnh: Zalo PC 26.11.10.3400 được người dùng cung cấp; không chạy installer. SHA-256 EXE `7166ea28925b3121369ffb25e87b4749a648f47a861272009dcb14094b5a5511`, ASAR `70f1b53c4db61321bf1b11c722a9049823711328b87e17b0d19338f83a5f6ed3`.

- `sync-v2-sub-worker.5c0110a3a8749b3aacde.js`, shared-worker export được format trong workspace research: handleResponse khoảng 9826, checkRequestfulfilled 9849. Dedup idx toàn syncId; isLast đặt totalBatch=idx+1. Fork kiểm tra chặt hơn bằng đủ từng idx, không chỉ size.
- Status enum SCpg: Active1, Idle2, WaitingConfirm3, Confirmed4, BypassConfirmed5, Reject6, Cancel7, Busy8, LowStorage9, Logout10. Idle/Active worker pause/resume wait. Reason enum 0 cancel/1success/2error/3outdated; journal cleanup phiên trước dùng OutDated. Không suy từ enum ra mọi cấu trúc event khác.
- Worker release đi qua data builder/session manager. Fork giữ phiên đến khi các công đoạn không còn cần nó; không khẳng định dispose sớm làm URL hết hạn.
- `compact-app-pc.5c0110a3a8749b3aacde.js`: static renewLink gửi FILE `/api/message/renewlink`, command12094; params toId/msgType/msgInfo(JSON string)/clientId/imei/isGroup. Forward clone types File1, Photo2, Video5 khác archive/live message codes. Cloud self dùng isGroup1. Response photo có oriUrl/normalUrl/hdUrl/thumbUrl; video có thumb và video.url. Client dùng encoded_params/common_params/post_form/maybe_decrypt chuẩn.
- PC modules CykU/QWlH/kEsE: removeJxlSignature gỡ jxlstatus, QWlH.h chuyển /jxl/ và .jxl sang /jpg/ và .jpg cho URL không chứa ?e2esession. Adapter giữ nguyên byte query khác (test token percent encoding); renewal chỉ gỡ flag, không đổi path. Không tự giải mã e2esession.

HTTP 200 kèm JSON lỗi chưa được nghiệm thu với link tài khoản thật. Parser chỉ nhận cấu trúc mã lỗi hoặc unexpected content, không suy ra tệp bị xóa. Không dùng chuỗi Invalid signature làm bằng chứng.

Fixture trong transfer.rs tái hiện: idx1 cuối có scopes B không có nghĩa A đang thiếu; idx0 tới sau vẫn phải được nhận. Metadata scope không xác nhận coverage hội thoại. Tests là dữ liệu giả; iPhone thật cần log phase/idx/last/scopes-count/error/status, không URL/khóa/body.

## Bổ sung từ PR upstream chưa merge

Ngày 03/11/2026: main vẫn ed550f6. PR #70 head `1dd4ee69d4a860a49b4a12dd550c82abaa681ce4`, commit `64fb9d5`, có mẫu CDN video dlmd.me và file dlfl.vn. Hai suffix cũng hiện trong `connect_src_rgx` của compact-app-pc và sync-v2-sub-worker 26.11.10. Tải media credential-free nhận hai họ này, giữ kiểm tra suffix/HTTPS/port/redirect. Không mở rộng nơi nhận cookie hoặc allowlist container điện thoại từ suy đoán.

Projection legacy chỉ nhận dạng các hàng URL/filename đủ dấu hiệu, không rewrite nguồn, không làm mất văn bản thường. Filename metadata giữ nguyên qua renewal dù URL mới có basename renewMedia.doc. Fixture kiểm host nhìn giống, credential URL và optional URL string trong renew request.

Chẩn đoán production đi qua transfer_diagnostics: event typed chỉ có số/enum/tag session đã băm, bounded channel và hai file JSONL khoảng 1 MiB/file. Không phụ thuộc logger dev; không nhận body/URL/khóa làm trường sự kiện.

## renewlink reply handling (Zalo PC 26.11.10 `compact-app-pc`, `__callClone` / `_callClone`) — 2026-11-03

- HTTP 200 with `data.error_code == 0`; `data.data` is AES-decoded then JSON-parsed into
  `{error_code, data}`. A non-zero inner code is a server rejection (`in2_<code>`).
- The inner `data` is dispatched by message type:
  `async` → `_awaitCtrlEvent` re-sends renewlink after a growing delay (bounded by
  `async_forward.timeout_wait_ctrl`); files use `fileUrl`; photos/videos use
  `normal` (→ `oriUrl`), `hd` (→ `hdUrl`), `thumb` (→ `thumbUrl`, a plain string); videos may
  also return `fileUrl`. Anything else is "cant handle this case".
- The fork accepts these plus the older `*Url` names, re-polls only on an explicit `async`
  flag (2/4/8 s inside one 15 s budget that also covers network time) and records a typed
  failure reason (`missing_data`, `wrong_type`, `decode_failed`, `url_blocked`,
  `server_rejected`) without URLs or server text.
- Field log 03/11: two requests got `server_code 0` then `parse_failed`, consistent with the
  `normal`/`hd`/`thumb` shape the old parser did not read. Needs a field re-test.
