# BÁO CÁO ĐỒNG THUẬN KIỂM TOÁN (CONSENSUS AUDIT REPORT)
## Tính năng: Animated Sticker Sprite Sheet (Zalo Emoticon CDN & Local GIF Stitching)

**Ngày hoàn thành:** 2026-11-17  
**Nhánh kiểm toán:** `feature/animated-stickers-sprite`  
**Commit cơ sở:** `14743b6` (Zalo CDN emoticon sprite endpoint & spike evidence)  
**Commit hoàn thiện:** `4c9b616` (Bản vá củng cố theo khuyến nghị của 3 Pro Auditor)  
**Trạng thái đồng thuận:** **100% UNANIMOUS APPROVE (CẢ 3 SUBAGENT GEMINI PRO PHÊ DUYỆT)**

---

## I. THÀNH PHẦN HỘI ĐỒNG KIỂM TOÁN GEMINI PRO

1. **Gemini Pro Auditor 1 (`6038300f-fe69-47ec-bc3e-137e9723da1d`):**  
   *Chuyên trách:* Architecture, Protocol Parity with Zalo PC/Web & CDN Endpoints.  
   *Đánh giá:* **APPROVE**
2. **Gemini Pro Auditor 2 (`fea32344-3e04-4344-8193-dfb17c3e9e50`):**  
   *Chuyên trách:* UI & Rendering Invariants, UV Mathematics, Timing, Viewport Culling & UX.  
   *Đánh giá:* **APPROVE**
3. **Gemini Pro Auditor 3 (`f0929827-39b2-48f8-96bd-4e9e64ddb748`):**  
   *Chuyên trách:* Memory Management (GPU/RAM), SSRF/Security, Concurrency & Defensive Boundary Protection.  
   *Đánh giá:* **APPROVE**

---

## II. TỔNG HỢP KIỂM TOÁN THEO CÁC TRỤ CỘT KỸ THUẬT

### 1. Protocol Parity & Endpoint Centralization (Kiểm chứng bởi Auditor 1)
- **CDN Emoticon Sprite:** Hàm `sticker_sprite_url(id)` trong `src/endpoints.rs` trỏ về `https://zalo-api.zadn.vn/api/emoticon/sprite?eid={id}&size=130`.
- **Thực nghiệm E1 (Spike probe):** Khảo sát 37 sticker ID từ 6 bộ sticker Zalo phổ biến xác nhận 100% sprite sheet trả về là **dải ngang tuyệt đối** chiều cao $130\text{px}$, chiều rộng $N \times 130\text{px}$ ($N \in [1..26]$).
- **Chu kỳ khung hình:** `ZALO_SPRITE_FRAME_DELAY = 80ms` (12.5 fps), khớp chuẩn hành vi Zalo Web/PC.
- **Pipeline nạp & Fallback an toàn:**
  - *Củ hành cổ (`cid == 0`, `sid 1..=30`):* CDN sprite trả về 404, pipeline bypass endpoint sprite và ưu tiên `builtin_sticker_thumbnail(sid)` rồi fallback sang `legacy_sticker_image(sid)`.
  - *Ami Fat Cat (`cid == 99901`):* Giải mã offline từ nhị phân tĩnh nhúng sẵn (`bundled_stickers::decode_gif_sprite(sid)`). Không rò rỉ I/O mạng, bảo mật offline 100%.
  - *Sticker thông thường:* Tải từ `sticker_sprite_url(sid)`, tự động fallback sang `legacy_sticker_image(sid)` nếu lỗi HTTP/mạng.
  - *Non-divisible / Tĩnh ($N=1$):* Khi kích thước ảnh không chia hết hoặc số frame $> 64$, tự động suy biến về 1 frame tĩnh, không crash, không loop.

### 2. UI & Rendering Invariants (Kiểm chứng bởi Auditor 2)
- **Toán học UV Coordinate:**
  $$\begin{cases} u_0 = \dfrac{i}{N} \\ u_1 = \dfrac{i + 1}{N} \\ v_0 = 0.0 \\ v_1 = 1.0 \end{cases}$$
  Đảm bảo trích xuất đúng từng frame đơn hình vuông từ dải ngang.
- **Khắc phục lỗi méo hình (Aspect Ratio Distortion):**
  Trong composer suggestions popup, bề rộng frame được tính theo `frame_w = tw / frames`. Khắc phục triệt để lỗi dải sprite 2400px bị nén dẹp thành đường chỉ mỏng 2.7px trước đây.
- **Stateless Animation Timing:**
  Sử dụng clock toàn cục `ui.input(|i| i.time)` modulo với `total_duration_ms`. Các sticker trên màn hình tự động đồng bộ mà không cần lưu timer nội tại trong widget.
- **Bất biến CPU / Viewport Culling:**
  `if ui.is_rect_visible(rect) { ui.ctx().request_repaint_after(time_until_next); }`
  Ngăn chặn triệt để CPU repaint loop khi tin nhắn sticker trôi ra ngoài màn hình cuộn. Egui trở về trạng thái idle 0% CPU.
- **Tương tác Hover:**
  Trong Sticker Picker Grid và Composer Suggestions, chỉ sticker đang hover hoặc được chọn bằng phím mới kích hoạt animation và timer repaint; các ô còn lại chỉ render frame 0 tĩnh.

### 3. Memory, Concurrency & Security (Kiểm chứng bởi Auditor 3)
- **Zero Duplicate VRAM Allocation:**
  `StickerSprite` giữ bản clone của `egui::TextureHandle` (bọc `Arc`). Không cấp phát thêm texture mới trên GPU, cả `sticker_textures` và `sticker_sprites` cùng trỏ tới 1 texture duy nhất.
- **Đồng bộ dọn dẹp bộ nhớ:**
  - LRU eviction qua `self.texture_budget`: Khi dung lượng vượt ngưỡng, loại bỏ đồng thời khỏi cả `sticker_textures` và `sticker_sprites`.
  - Dọn dẹp sạch khi Logout (`LoggedOut`), Chuyển hồ sơ (`ProfileSwitched`), Đổi tài khoản (`LoginSuccess`).
  - Khóa hội thoại ẩn (`hidden_panel.rs`): Đã bổ sung `self.sticker_sprites.clear()` đồng bộ cùng `self.sticker_textures.clear()`.
- **SSRF & Network Hardening:**
  - Hàm `download_sticker_image_bytes` kiểm tra allowlist strict qua `compatible_image_url` và `media_download::allowed`. Chặn mọi IP private / loopback / credentials.
  - Sử dụng `read_bounded_response` với giới hạn `IMAGE_BYTES` (64MB) chống decompression bomb.
  - Lấy permit đồng thời qua `image_network_permit(true)` ngăn chặn bão kết nối.
- **Arithmetic Defense:**
  - Kiểm tra `(acc + d_ms).saturating_sub(elapsed_ms)` phòng thủ overflow panic.
  - Toàn bộ phép chia đều có guard `.max(1)` hoặc kiểm tra kích thước `> 0`.

---

## III. BẢNG TỔNG HỢP KIỂM TOÁN TỪNG SUBAGENT

| Trọng tâm kiểm toán | Pro Auditor 1 (Protocol) | Pro Auditor 2 (UI/UX) | Pro Auditor 3 (Security/Mem) | Trạng thái chung |
| :--- | :---: | :---: | :---: | :---: |
| **Zalo CDN Sprite Parity** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **Fallback Pipeline (Củ hành, Ami, 404)** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **Worker Concurrency & Non-blocking UI** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **UV Coordinates & Aspect Ratio** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **CPU Viewport Culling & Clamping** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **VRAM & Texture Handle Sharing** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **SSRF & Bounded Network Download** | APPROVE | APPROVE | APPROVE | **ĐỒNG THUẬN (PASS)** |
| **Suite Verification (861 tests)** | 861/861 PASS | 861/861 PASS | 861/861 PASS | **100% PASS** |

---

## IV. QUYẾT ĐỊNH CUỐI CÙNG

Hội đồng 3 Gemini Pro Auditors chính thức **CHẤP THUẬN (APPROVE)** 100% đối với mã nguồn tính năng Animated Sticker Sprite Sheet.  
Đủ điều kiện sáp nhập vào nhánh chính `main` và đẩy lên GitHub repository `origin/main`.
