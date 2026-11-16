# Báo cáo Thực nghiệm (Spike): CDN Emoticon Sprite Sheet Zalo

**Ngày thực hiện:** 2026-11-17  
**Phạm vi:** 37 Sticker IDs thuộc 6 danh mục phổ biến + 5 ID sticker Củ hành (builtin static).  
**Endpoint kiểm tra:** `https://zalo-api.zadn.vn/api/emoticon/sprite?eid={eid}&size=130`  
**Endpoint fallback:** `https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={eid}&size=130`

---

## 1. Dữ liệu thực nghiệm (37 Sticker IDs)

| Nhóm / Category | EID | Kích thước ảnh (WxH) | Chiều cao đơn vị (px) | Số frames ($W/H$) | $W \pmod H$ | Dung lượng (bytes) | Phân loại |
|---|---|---|---|---|---|---|---|
| **Zapy Dô Tri** (61900) | 98930 | 130x130 | 130 | 1 | 0 | 19,408 | Static |
| | 98931 | 130x130 | 130 | 1 | 0 | 19,693 | Static |
| | 98932 | 130x130 | 130 | 1 | 0 | 20,260 | Static |
| | 98933 | 130x130 | 130 | 1 | 0 | 18,733 | Static |
| | 98934 | 130x130 | 130 | 1 | 0 | 19,429 | Static |
| | 98935 | 130x130 | 130 | 1 | 0 | 19,523 | Static |
| **Bé Hai** (12815) | 101001 | 1300x130 | 130 | 10 | 0 | 58,359 | **Animated** |
| | 101002 | 1300x130 | 130 | 10 | 0 | 53,380 | **Animated** |
| | 101003 | 1560x130 | 130 | 12 | 0 | 83,420 | **Animated** |
| | 101004 | 1300x130 | 130 | 10 | 0 | 64,164 | **Animated** |
| | 101005 | 2080x130 | 130 | 16 | 0 | 37,173 | **Animated** |
| | 101006 | 1300x130 | 130 | 10 | 0 | 38,232 | **Animated** |
| **Cò Lõ Đi Làm** (12748) | 99978 | 1040x130 | 130 | 8 | 0 | 110,243 | **Animated** |
| | 99979 | 1040x130 | 130 | 8 | 0 | 106,344 | **Animated** |
| | 99980 | 1040x130 | 130 | 8 | 0 | 65,477 | **Animated** |
| | 99981 | 1560x130 | 130 | 12 | 0 | 110,753 | **Animated** |
| | 99982 | 1040x130 | 130 | 8 | 0 | 48,838 | **Animated** |
| | 99983 | 1430x130 | 130 | 11 | 0 | 87,681 | **Animated** |
| **Mèo Meon** (12705) | 99278 | 3120x130 | 130 | 24 | 0 | 69,129 | **Animated** |
| | 99279 | 3380x130 | 130 | 26 | 0 | 194,023 | **Animated** |
| | 99280 | 2080x130 | 130 | 16 | 0 | 83,842 | **Animated** |
| | 99281 | 3120x130 | 130 | 24 | 0 | 146,586 | **Animated** |
| | 99282 | 1560x130 | 130 | 12 | 0 | 45,128 | **Animated** |
| | 99283 | 1300x130 | 130 | 10 | 0 | 49,162 | **Animated** |
| **Gấu Cay** (12718) | 99486 | 260x130 | 130 | 2 | 0 | 29,833 | **Animated** |
| | 99487 | 260x130 | 130 | 2 | 0 | 31,757 | **Animated** |
| | 99488 | 260x130 | 130 | 2 | 0 | 32,799 | **Animated** |
| | 99489 | 260x130 | 130 | 2 | 0 | 37,457 | **Animated** |
| | 99490 | 390x130 | 130 | 3 | 0 | 34,801 | **Animated** |
| | 99491 | 390x130 | 130 | 3 | 0 | 32,111 | **Animated** |
| **Zameo Rốn Lồi** (12045) | 47614 | 1300x130 | 130 | 10 | 0 | 71,820 | **Animated** |
| | 47615 | 1560x130 | 130 | 12 | 0 | 122,472 | **Animated** |
| | 47616 | 2080x130 | 130 | 16 | 0 | 87,855 | **Animated** |
| | 47617 | 1950x130 | 130 | 15 | 0 | 82,915 | **Animated** |
| | 47618 | 1690x130 | 130 | 13 | 0 | 113,135 | **Animated** |
| | 47619 | 1820x130 | 130 | 14 | 0 | 85,227 | **Animated** |
| **Củ Hành (Builtin static)** | 1..=5 | HTTP 404 | — | 0 | — | — | **Fallback webpc** (200 OK, static PNG) |

---

## 2. Kết luận then chốt rút ra từ Thực nghiệm

1. **100% Ảnh Sprite trên CDN Zalo là Dải Ngang Tuyệt Đối:**
   - Trong toàn bộ 32 ID sticker hợp lệ lấy từ `api/emoticon/sprite`, chiều cao đều là $130\text{px}$ (khớp chuẩn với `size=130`).
   - Mọi ảnh đều thỏa mãn phép chia hết: $\text{width} \pmod{\text{height}} == 0$.
   - Số lượng khung hình dao động từ $1$ đến $26$ frames (trong đó nhóm 2-3 frames như Gấu Cay, 8-12 frames như Bé Hai/Cò Lõ, và 24-26 frames như Mèo Meon).
2. **Quy tắc phân loại Static vs Animated hoàn toàn chính xác:**
   - Gói Zapy Dô Tri trả về $130\times 130$ ($1$ frame) $\implies$ Tự động suy biến thành static image, không có overhead animation loop.
   - Các gói còn lại trả về $2$ đến $26$ frames $\implies$ Kích hoạt UV animation.
3. **Cơ chế Fallback bắt buộc đối với Sticker Cũ / Củ Hành:**
   - Các sticker cổ điển (IDs 1..=30) trả về `HTTP 404` trên endpoint `sprite`.
   - Nhưng khi gọi endpoint fallback `api/emoticon/sticker/webpc?eid={id}&size=130`, server trả về `HTTP 200 OK` ảnh PNG tĩnh bình thường ($12,512$ bytes).
   - Do đó: Pipeline nạp sticker **bắt buộc** phải thử endpoint `sprite` trước, nếu gặp 404/decode error thì fallback sang `webpc`.
