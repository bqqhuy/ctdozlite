"""
Generates high-fidelity visual UI demo mockups for the Zalo PC GUI/UX Overhaul.
Outputs crisp PNG images into docs/assets/:
- overhaul_overview_4zones.png
- phase1_nav_rail_sidebar.png
- phase2_two_tier_composer.png
- phase3_bubbles_reactions.png
- phase4_drawer_multiview.png
- phase5_contacts_modals.png
"""

import os
from PIL import Image, ImageDraw, ImageFont

ASSETS_DIR = os.path.abspath("docs/assets")
os.makedirs(ASSETS_DIR, exist_ok=True)

# Colors matching Zalo PC design tokens
COLOR_BRAND_BLUE = (0, 104, 255)
COLOR_BRAND_BLUE_HOVER = (0, 90, 224)
COLOR_BRAND_BLUE_LIGHT = (229, 241, 255)
COLOR_BRAND_BLUE_BG = (240, 247, 255)
COLOR_CANVAS_BG = (232, 238, 244)
COLOR_SIDEBAR_BG = (248, 250, 252)
COLOR_WHITE = (255, 255, 255)
COLOR_BORDER = (226, 232, 240)
COLOR_TEXT_PRIMARY = (8, 27, 58)
COLOR_TEXT_SECONDARY = (90, 105, 129)
COLOR_TEXT_MUTED = (148, 163, 184)
COLOR_ONLINE_GREEN = (16, 185, 129)
COLOR_BADGE_RED = (239, 68, 68)
COLOR_CARD_HOVER = (241, 245, 249)

# Fonts
FONT_PATH = "C:/Windows/Fonts/segoeui.ttf"
FONT_BOLD_PATH = "C:/Windows/Fonts/segoeuib.ttf"

def get_font(size, bold=False):
    path = FONT_BOLD_PATH if bold else FONT_PATH
    try:
        return ImageFont.truetype(path, size)
    except Exception:
        return ImageFont.load_default()

def draw_rounded_rect(draw, xy, radius, fill=None, outline=None, width=1):
    draw.rounded_rectangle(xy, radius=radius, fill=fill, outline=outline, width=width)

def draw_avatar(draw, center_x, center_y, radius, name_initials, bg_color=None, online=False):
    if bg_color is None:
        bg_color = (0, 104, 255)
    bbox = [center_x - radius, center_y - radius, center_x + radius, center_y + radius]
    draw.ellipse(bbox, fill=bg_color)
    font = get_font(int(radius * 0.9), bold=True)
    draw.text((center_x, center_y), name_initials, fill=(255, 255, 255), anchor="mm", font=font)
    if online:
        dot_r = radius * 0.3
        dot_x = center_x + radius * 0.7
        dot_y = center_y + radius * 0.7
        draw.ellipse([dot_x - dot_r - 2, dot_y - dot_r - 2, dot_x + dot_r + 2, dot_y + dot_r + 2], fill=COLOR_WHITE)
        draw.ellipse([dot_x - dot_r, dot_y - dot_r, dot_x + dot_r, dot_y + dot_r], fill=COLOR_ONLINE_GREEN)

# ==============================================================================
# 1. OVERVIEW: 4-ZONE ARCHITECTURE MOCKUP (1600x900)
# ==============================================================================
def generate_overview_mockup():
    w, h = 1600, 900
    img = Image.new("RGB", (w, h), COLOR_CANVAS_BG)
    draw = ImageDraw.Draw(img)

    # 1. Left Nav Rail (64px)
    rail_w = 64
    draw.rectangle([0, 0, rail_w, h], fill=COLOR_BRAND_BLUE)
    # User Avatar at top
    draw_avatar(draw, rail_w // 2, 40, 20, "H", bg_color=(255, 255, 255, 40), online=True)
    # Chat Nav Icon (Selected)
    draw.rounded_rectangle([10, 80, rail_w - 10, 124], radius=8, fill=(0, 90, 224))
    draw.text((rail_w // 2, 102), "💬", font=get_font(20), anchor="mm", fill=COLOR_WHITE)
    # Unread badge on chat icon
    draw.ellipse([rail_w - 20, 82, rail_w - 6, 96], fill=COLOR_BADGE_RED)
    draw.text((rail_w - 13, 89), "5", font=get_font(10, bold=True), anchor="mm", fill=COLOR_WHITE)
    
    # Other Nav icons
    icons = [("📒", 150), ("☑️", 204), ("☁️", h - 140), ("🧰", h - 90), ("⚙️", h - 40)]
    for icon, y in icons:
        draw.text((rail_w // 2, y), icon, font=get_font(20), anchor="mm", fill=COLOR_WHITE)

    # 2. Sidebar (320px)
    sb_x = rail_w
    sb_w = 320
    draw.rectangle([sb_x, 0, sb_x + sb_w, h], fill=COLOR_SIDEBAR_BG)
    draw.line([sb_x + sb_w, 0, sb_x + sb_w, h], fill=COLOR_BORDER, width=1)

    # Top search bar + buttons
    draw_rounded_rect(draw, [sb_x + 12, 14, sb_x + sb_w - 76, 46], radius=6, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((sb_x + 22, 30), "🔍", font=get_font(12), anchor="mm")
    draw.text((sb_x + 36, 30), "Tìm kiếm...", font=get_font(13), anchor="lm", fill=COLOR_TEXT_MUTED)
    
    # Action buttons [+👤] [👥+]
    draw_rounded_rect(draw, [sb_x + sb_w - 68, 14, sb_x + sb_w - 40, 46], radius=6, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((sb_x + sb_w - 54, 30), "👤+", font=get_font(12, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)
    draw_rounded_rect(draw, [sb_x + sb_w - 36, 14, sb_x + sb_w - 8, 46], radius=6, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((sb_x + sb_w - 22, 30), "👥+", font=get_font(12, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)

    # Filter strip
    draw.text((sb_x + 16, 68), "Tất cả", font=get_font(13, bold=True), fill=COLOR_BRAND_BLUE)
    draw.line([sb_x + 16, 82, sb_x + 56, 82], fill=COLOR_BRAND_BLUE, width=2)
    draw.text((sb_x + 76, 68), "Chưa đọc", font=get_font(13), fill=COLOR_TEXT_SECONDARY)
    draw.text((sb_x + sb_w - 70, 68), "Phân loại ▾", font=get_font(12), fill=COLOR_TEXT_SECONDARY)
    draw.text((sb_x + sb_w - 18, 68), "⋯", font=get_font(14, bold=True), fill=COLOR_TEXT_SECONDARY)
    draw.line([sb_x, 86, sb_x + sb_w, 86], fill=COLOR_BORDER, width=1)

    # Thread rows
    threads = [
        ("Cloud của tôi", "Bạn: File_bao_cao.docx", "10:24", True, False, 0),
        ("Nhóm Dự Án ZaloLite", "Nguyễn An: Đã cập nhật xong API", "09:45", False, True, 3),
        ("Trần Thị B", "Dạ em đã nhận được tài liệu rồi ạ!", "Hôm qua", False, False, 0),
        ("Lê Văn C", "[Hình ảnh]", "14/11", False, False, 1),
    ]
    curr_y = 94
    for i, (name, preview, time_str, is_cloud, is_group, unread) in enumerate(threads):
        row_h = 70
        is_sel = (i == 1)
        row_bg = COLOR_BRAND_BLUE_LIGHT if is_sel else (COLOR_WHITE if i % 2 == 0 else COLOR_SIDEBAR_BG)
        draw.rectangle([sb_x, curr_y, sb_x + sb_w, curr_y + row_h], fill=row_bg)
        av_color = (0, 104, 255) if not is_cloud else (99, 102, 241)
        initials = "☁" if is_cloud else ("N" if is_group else name[0])
        draw_avatar(draw, sb_x + 36, curr_y + row_h // 2, 22, initials, bg_color=av_color, online=(i==2))
        draw.text((sb_x + 68, curr_y + 16), name, font=get_font(13, bold=True), fill=COLOR_TEXT_PRIMARY)
        draw.text((sb_x + sb_w - 12, curr_y + 16), time_str, font=get_font(11), anchor="ra", fill=COLOR_TEXT_MUTED)
        draw.text((sb_x + 68, curr_y + 38), preview, font=get_font(12), fill=COLOR_TEXT_SECONDARY)
        if unread > 0:
            draw.rounded_rectangle([sb_x + sb_w - 28, curr_y + 36, sb_x + sb_w - 12, curr_y + 52], radius=8, fill=COLOR_BADGE_RED)
            draw.text((sb_x + sb_w - 20, curr_y + 44), str(unread), font=get_font(10, bold=True), anchor="mm", fill=COLOR_WHITE)
        draw.line([sb_x + 68, curr_y + row_h, sb_x + sb_w, curr_y + row_h], fill=COLOR_BORDER, width=1)
        curr_y += row_h

    # 3. Right Info Drawer (340px)
    dr_w = 340
    dr_x = w - dr_w
    draw.rectangle([dr_x, 0, w, h], fill=COLOR_WHITE)
    draw.line([dr_x, 0, dr_x, h], fill=COLOR_BORDER, width=1)

    # Drawer Header
    draw.text((dr_x + dr_w // 2, 24), "Thông tin hội thoại", font=get_font(15, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)
    draw.line([dr_x, 48, w, 48], fill=COLOR_BORDER, width=1)
    
    # Group Card
    draw_avatar(draw, dr_x + dr_w // 2, 85, 26, "N", bg_color=(0, 104, 255))
    draw.text((dr_x + dr_w // 2, 122), "Nhóm Dự Án ZaloLite", font=get_font(14, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)
    draw.text((dr_x + dr_w // 2, 140), "24 thành viên", font=get_font(12), anchor="mm", fill=COLOR_TEXT_SECONDARY)

    # 4 Quick Action circular buttons
    q_actions = [("🔔", "Tắt thông báo"), ("📌", "Ghim chat"), ("👥+", "Thêm TV"), ("⚙️", "Quản lý")]
    q_start_x = dr_x + 25
    q_spacing = 75
    for idx, (icon, lbl) in enumerate(q_actions):
        cx = q_start_x + idx * q_spacing + 20
        draw.ellipse([cx - 18, 160, cx + 18, 196], fill=COLOR_BRAND_BLUE_BG, outline=COLOR_BORDER)
        draw.text((cx, 178), icon, font=get_font(13), anchor="mm")
        draw.text((cx, 206), lbl, font=get_font(10), anchor="mm", fill=COLOR_TEXT_SECONDARY)

    # Drawer Accordions
    draw.line([dr_x, 225, w, 225], fill=COLOR_BORDER, width=1)
    accordions = [
        ("👥 Thành viên nhóm", "24 người"),
        ("📋 Bảng tin nhóm", "3 ghi chú, 1 bình chọn"),
        ("🖼️ Ảnh / Video", "Xem tất cả (48)"),
        ("📎 File", "Xem tất cả (12)"),
        ("🔗 Link", "Xem tất cả (8)"),
        ("🔒 Thiết lập bảo mật", "E2EE, Tự xóa, Ẩn chat"),
    ]
    acc_y = 232
    for title, sub in accordions:
        draw_rounded_rect(draw, [dr_x + 10, acc_y, w - 10, acc_y + 42], radius=6, fill=COLOR_CARD_HOVER)
        draw.text((dr_x + 20, acc_y + 14), title, font=get_font(12, bold=True), fill=COLOR_TEXT_PRIMARY)
        draw.text((w - 28, acc_y + 14), sub, font=get_font(11), anchor="ra", fill=COLOR_TEXT_MUTED)
        draw.text((w - 18, acc_y + 14), "›", font=get_font(14, bold=True), fill=COLOR_TEXT_MUTED)
        acc_y += 48

    # 4. Central Chat Canvas
    chat_x = sb_x + sb_w
    chat_w = dr_x - chat_x
    draw.rectangle([chat_x, 0, dr_x, h], fill=COLOR_CANVAS_BG)

    # Chat Header (64px)
    draw.rectangle([chat_x, 0, dr_x, 64], fill=COLOR_WHITE)
    draw.line([chat_x, 64, dr_x, 64], fill=COLOR_BORDER, width=1)
    draw_avatar(draw, chat_x + 36, 32, 20, "N", bg_color=(0, 104, 255))
    draw.text((chat_x + 66, 22), "Nhóm Dự Án ZaloLite", font=get_font(14, bold=True), fill=COLOR_TEXT_PRIMARY)
    draw.text((chat_x + 66, 42), "👤 24 thành viên · 4 người trực tuyến", font=get_font(11), fill=COLOR_TEXT_SECONDARY)
    
    # Chat Header Action Icons
    header_icons = ["👥+", "📞", "🎥", "🔍", "◫"]
    for idx, h_icon in enumerate(header_icons):
        hx = dr_x - 180 + idx * 36
        draw_rounded_rect(draw, [hx, 18, hx + 30, 48], radius=6, fill=COLOR_WHITE, outline=COLOR_BORDER)
        draw.text((hx + 15, 33), h_icon, font=get_font(12), anchor="mm", fill=COLOR_TEXT_PRIMARY)

    # Pinned banner
    draw.rectangle([chat_x, 65, dr_x, 95], fill=COLOR_WHITE)
    draw.line([chat_x, 95, dr_x, 95], fill=COLOR_BORDER, width=1)
    draw.text((chat_x + 16, 80), "📌 Tin nhắn đã ghim: Họp rà soát tiến độ v0.016 vào lúc 14:00 chiều nay", font=get_font(12, bold=True), fill=COLOR_TEXT_PRIMARY, anchor="lm")
    draw.text((dr_x - 20, 80), "3 ghim ▾", font=get_font(12), fill=COLOR_BRAND_BLUE, anchor="rm")

    # Message Bubbles
    # Message 1 (Incoming)
    draw_avatar(draw, chat_x + 30, 140, 16, "A", bg_color=(234, 88, 12))
    draw.text((chat_x + 54, 120), "Nguyễn An", font=get_font(11, bold=True), fill=COLOR_TEXT_SECONDARY)
    draw_rounded_rect(draw, [chat_x + 54, 134, chat_x + 440, 184], radius=12, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((chat_x + 68, 150), "Chào cả nhóm, mình đã hoàn thành xong bản phân tích GUI gốc!", font=get_font(13), fill=COLOR_TEXT_PRIMARY)
    draw.text((chat_x + 424, 172), "09:42", font=get_font(10), anchor="ra", fill=COLOR_TEXT_MUTED)

    # Reaction Pill on incoming message
    draw_rounded_rect(draw, [chat_x + 380, 172, chat_x + 434, 196], radius=10, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((chat_x + 407, 184), "❤️ 👍 3", font=get_font(10, bold=True), anchor="mm", fill=COLOR_BRAND_BLUE)

    # Message 2 (Outgoing)
    draw_rounded_rect(draw, [dr_x - 380, 220, dr_x - 20, 270], radius=12, fill=COLOR_BRAND_BLUE_LIGHT, outline=COLOR_BRAND_BLUE_LIGHT)
    draw.text((dr_x - 364, 236), "Tuyệt vời, để mình tiến hành triển khai Phase 1 luôn nhé.", font=get_font(13), fill=COLOR_TEXT_PRIMARY)
    draw.text((dr_x - 32, 258), "09:45", font=get_font(10), anchor="ra", fill=COLOR_TEXT_SECONDARY)

    # Floating Reaction Pill on outgoing message
    draw_rounded_rect(draw, [dr_x - 70, 258, dr_x - 26, 282], radius=10, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((dr_x - 48, 270), "👍 1", font=get_font(10, bold=True), anchor="mm", fill=COLOR_BRAND_BLUE)

    # Outgoing Delivery status below bubble
    draw.text((dr_x - 24, 292), "✓✓ Đã xem", font=get_font(10), anchor="ra", fill=COLOR_TEXT_MUTED)

    # 2-Tier Composer at bottom of chat
    comp_y = h - 140
    draw.rectangle([chat_x, comp_y, dr_x, h], fill=COLOR_WHITE)
    draw.line([chat_x, comp_y, dr_x, comp_y], fill=COLOR_BORDER, width=1)
    
    # Tier 1: Toolbar
    tb_icons = ["😊 Sticker", "🖼️ Ảnh", "📎 Đính kèm", "✂️ Chụp màn hình", "📇 Danh thiếp", "⏰ Nhắc hẹn", "⚡ Tin nhanh", "🎤 Thu âm"]
    tb_curr_x = chat_x + 16
    for tb_i in tb_icons:
        draw.text((tb_curr_x, comp_y + 18), tb_i, font=get_font(12), anchor="lm", fill=COLOR_TEXT_SECONDARY)
        tb_curr_x += 105
    draw.line([chat_x, comp_y + 36, dr_x, comp_y + 36], fill=COLOR_BORDER, width=1)

    # Tier 2: Editor input area + Send button
    draw.text((chat_x + 18, comp_y + 60), "Nhập tin nhắn tới Nhóm Dự Án ZaloLite...", font=get_font(13), fill=COLOR_TEXT_MUTED)
    
    # Action right: Emoji button + Send button
    draw.text((dr_x - 90, comp_y + 80), "😊", font=get_font(18), anchor="mm")
    draw_rounded_rect(draw, [dr_x - 70, comp_y + 64, dr_x - 18, comp_y + 98], radius=6, fill=COLOR_BRAND_BLUE)
    draw.text((dr_x - 44, comp_y + 81), "GỬI", font=get_font(12, bold=True), anchor="mm", fill=COLOR_WHITE)

    # Save
    out_path = os.path.join(ASSETS_DIR, "overhaul_overview_4zones.png")
    img.save(out_path, quality=95)
    print("Saved:", out_path)

# ==============================================================================
# 2. PHASE 1: NAV RAIL & SIDEBAR HEADER DEMO (1000x700)
# ==============================================================================
def generate_phase1_mockup():
    w, h = 1000, 700
    img = Image.new("RGB", (w, h), COLOR_CANVAS_BG)
    draw = ImageDraw.Draw(img)

    # Nav Rail (70px)
    rail_w = 70
    draw.rectangle([0, 0, rail_w, h], fill=COLOR_BRAND_BLUE)
    draw_avatar(draw, rail_w // 2, 45, 22, "H", bg_color=(255, 255, 255, 40), online=True)
    draw.rounded_rectangle([12, 90, rail_w - 12, 138], radius=8, fill=(0, 90, 224))
    draw.text((rail_w // 2, 114), "💬", font=get_font(22), anchor="mm", fill=COLOR_WHITE)
    draw.text((rail_w // 2, 170), "📒", font=get_font(22), anchor="mm", fill=COLOR_WHITE)
    draw.text((rail_w // 2, 226), "☑️", font=get_font(22), anchor="mm", fill=COLOR_WHITE)
    draw.text((rail_w // 2, h - 50), "⚙️", font=get_font(22), anchor="mm", fill=COLOR_WHITE)

    # Sidebar (360px)
    sb_w = 360
    draw.rectangle([rail_w, 0, rail_w + sb_w, h], fill=COLOR_SIDEBAR_BG)
    draw.line([rail_w + sb_w, 0, rail_w + sb_w, h], fill=COLOR_BORDER, width=1)

    # Top search bar
    sb_x = rail_w
    draw_rounded_rect(draw, [sb_x + 16, 16, sb_x + sb_w - 90, 52], radius=8, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((sb_x + 28, 34), "🔍", font=get_font(14), anchor="mm")
    draw.text((sb_x + 44, 34), "Tìm kiếm cuộc trò chuyện...", font=get_font(13), anchor="lm", fill=COLOR_TEXT_MUTED)
    
    # Quick action buttons [+👤] [👥+]
    draw_rounded_rect(draw, [sb_x + sb_w - 80, 16, sb_x + sb_w - 46, 52], radius=8, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((sb_x + sb_w - 63, 34), "👤+", font=get_font(13, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)
    draw_rounded_rect(draw, [sb_x + sb_w - 40, 16, sb_x + sb_w - 6, 52], radius=8, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((sb_x + sb_w - 23, 34), "👥+", font=get_font(13, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)

    # Clean tabs (No technical buttons)
    draw.text((sb_x + 20, 78), "Tất cả", font=get_font(14, bold=True), fill=COLOR_BRAND_BLUE)
    draw.line([sb_x + 20, 94, sb_x + 64, 94], fill=COLOR_BRAND_BLUE, width=3)
    draw.text((sb_x + 88, 78), "Chưa đọc", font=get_font(14), fill=COLOR_TEXT_SECONDARY)
    
    # Dropdowns: Phân loại ▾ and Menu ⋯
    draw.text((sb_x + sb_w - 94, 78), "Phân loại ▾", font=get_font(13), fill=COLOR_TEXT_SECONDARY)
    draw.text((sb_x + sb_w - 20, 78), "⋯", font=get_font(16, bold=True), fill=COLOR_TEXT_SECONDARY)
    draw.line([sb_x, 100, sb_x + sb_w, 100], fill=COLOR_BORDER, width=1)

    # Explanation Callout
    call_x = rail_w + sb_w + 30
    draw_rounded_rect(draw, [call_x, 50, w - 30, h - 50], radius=12, fill=COLOR_WHITE, outline=COLOR_BRAND_BLUE, width=2)
    draw.text((call_x + 20, 80), "Phase 1: Chuẩn hóa Nav Rail & Sidebar Header", font=get_font(18, bold=True), fill=COLOR_BRAND_BLUE)
    
    points = [
        ("✓ Left Nav Rail chuẩn Zalo PC:", "Nền xanh thương hiệu #0068FF, icon phẳng căn đều, avatar online."),
        ("✓ Đưa [+👤 Thêm bạn] & [👥+ Tạo nhóm] lên đỉnh:", "Nằm ngay cạnh thanh tìm kiếm, phản hồi tức thì."),
        ("✓ Xóa sạch các nút bấm kỹ thuật thô:", "Loại bỏ hoàn toàn các nút 'Hội thoại ẩn/PIN', 'Phân loại hội thoại' thô."),
        ("✓ Tab phân chia tối giản:", "Chỉ giữ 2 tab 'Tất cả' | 'Chưa đọc' kèm dropdown 'Phân loại ▾' và menu '⋯'."),
        ("✓ Không làm gián đoạn luồng dữ liệu (Zero UI Stubs):", "Kết nối trọn vẹn với WorkerCmd và SQLite local store.")
    ]
    py = 130
    for p_title, p_desc in points:
        draw.text((call_x + 20, py), p_title, font=get_font(14, bold=True), fill=COLOR_TEXT_PRIMARY)
        draw.text((call_x + 20, py + 22), p_desc, font=get_font(13), fill=COLOR_TEXT_SECONDARY)
        py += 60

    out_path = os.path.join(ASSETS_DIR, "phase1_nav_rail_sidebar.png")
    img.save(out_path, quality=95)
    print("Saved:", out_path)

# ==============================================================================
# 3. PHASE 2: TWO-TIER COMPOSER & CHAT HEADER (1000x700)
# ==============================================================================
def generate_phase2_mockup():
    w, h = 1000, 700
    img = Image.new("RGB", (w, h), COLOR_CANVAS_BG)
    draw = ImageDraw.Draw(img)

    # Chat Header (64px)
    draw.rectangle([0, 0, w, 64], fill=COLOR_WHITE)
    draw.line([0, 64, w, 64], fill=COLOR_BORDER, width=1)
    draw_avatar(draw, 40, 32, 20, "Z", bg_color=COLOR_BRAND_BLUE)
    draw.text((72, 22), "Ban Quản Trị ZaloLite", font=get_font(15, bold=True), fill=COLOR_TEXT_PRIMARY)
    draw.text((72, 42), "👤 8 thành viên · Trực tuyến", font=get_font(12), fill=COLOR_TEXT_SECONDARY)
    
    # 5 Flat Action Icons in Header
    h_icons = ["👥+", "📞", "🎥", "🔍", "◫"]
    for idx, h_icon in enumerate(h_icons):
        hx = w - 210 + idx * 40
        draw_rounded_rect(draw, [hx, 16, hx + 32, 48], radius=6, fill=COLOR_WHITE, outline=COLOR_BORDER)
        draw.text((hx + 16, 32), h_icon, font=get_font(13), anchor="mm", fill=COLOR_TEXT_PRIMARY)

    # Collapsible Pinned Banner
    draw.rectangle([0, 65, w, 100], fill=COLOR_WHITE)
    draw.line([0, 100, w, 100], fill=COLOR_BORDER, width=1)
    draw.text((20, 82), "📌 Tin nhắn đã ghim: Release build v0.016-arm đã hoàn tất đồng bộ", font=get_font(13, bold=True), fill=COLOR_TEXT_PRIMARY, anchor="lm")
    draw.text((w - 30, 82), "2 ghim ▾", font=get_font(13), fill=COLOR_BRAND_BLUE, anchor="rm")

    # Callout / Explanation in the middle
    mid_y = 120
    draw_rounded_rect(draw, [40, mid_y, w - 40, mid_y + 280], radius=12, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((60, mid_y + 30), "Phase 2: Khung Soạn Thảo 2 Tầng (Two-Tier Composer) & Header Phẳng", font=get_font(17, bold=True), fill=COLOR_BRAND_BLUE)
    
    p2_points = [
        ("• Header Phẳng 64px:", "Gồm đầy đủ thông tin hội thoại và cụm 5 nút chức năng chuẩn Zalo PC."),
        ("• Pinned Banner thả xuống:", "Gom tin ghim gọn gàng dưới Header, bấm mở danh sách thay vì mở egui::Window."),
        ("• Tầng 1 - Toolbar Tiện ích (36px):", "Icon phẳng hỗ trợ Sticker, Ảnh, Đính kèm file, Chụp màn hình, Danh thiếp, Nhắc hẹn, Ghi âm."),
        ("• Tầng 2 - Khung Soạn thảo & Gửi:", "Ô nhập liệu mở rộng, hỗ trợ kéo thả file, nút 'GỬI' xanh (#0068FF) hoặc icon '👍' Like khi rỗng.")
    ]
    py = mid_y + 70
    for title, desc in p2_points:
        draw.text((60, py), title, font=get_font(13, bold=True), fill=COLOR_TEXT_PRIMARY)
        draw.text((280, py), desc, font=get_font(13), fill=COLOR_TEXT_SECONDARY)
        py += 45

    # Two-Tier Composer at bottom (240px tall)
    comp_y = h - 220
    draw.rectangle([0, comp_y, w, h], fill=COLOR_WHITE)
    draw.line([0, comp_y, w, comp_y], fill=COLOR_BORDER, width=1)
    
    # Reply Preview Strip (if replying)
    draw.rectangle([0, comp_y, w, comp_y + 34], fill=COLOR_BRAND_BLUE_BG)
    draw.line([0, comp_y, 4, comp_y + 34], fill=COLOR_BRAND_BLUE, width=4)
    draw.text((20, comp_y + 17), "↩ Đang trả lời Nguyễn An: Chào cả nhóm...", font=get_font(12), anchor="lm", fill=COLOR_TEXT_PRIMARY)
    draw.text((w - 30, comp_y + 17), "✕", font=get_font(13, bold=True), anchor="rm", fill=COLOR_TEXT_MUTED)

    # Tier 1 Toolbar
    tb_y = comp_y + 42
    tb_items = [("😊", "Sticker"), ("🖼️", "Gửi ảnh"), ("📎", "Đính kèm"), ("✂️", "Chụp màn hình"), ("📇", "Danh thiếp"), ("⏰", "Nhắc hẹn"), ("⚡", "Tin nhanh"), ("🎤", "Ghi âm")]
    curr_tx = 20
    for icon, label in tb_items:
        draw.text((curr_tx, tb_y), f"{icon} {label}", font=get_font(12), fill=COLOR_TEXT_SECONDARY)
        curr_tx += 118
    draw.line([0, tb_y + 24, w, tb_y + 24], fill=COLOR_BORDER, width=1)

    # Tier 2 Editor Area
    ed_y = tb_y + 34
    draw.text((20, ed_y + 10), "Gõ tin nhắn, thả file hoặc nhấn Ctrl+V để dán ảnh...", font=get_font(13), fill=COLOR_TEXT_MUTED)
    draw.text((w - 110, ed_y + 20), "😊", font=get_font(20), anchor="mm")
    draw_rounded_rect(draw, [w - 90, ed_y + 4, w - 20, ed_y + 38], radius=6, fill=COLOR_BRAND_BLUE)
    draw.text((w - 55, ed_y + 21), "GỬI", font=get_font(12, bold=True), anchor="mm", fill=COLOR_WHITE)

    out_path = os.path.join(ASSETS_DIR, "phase2_two_tier_composer.png")
    img.save(out_path, quality=95)
    print("Saved:", out_path)

# ==============================================================================
# 4. PHASE 3: BUBBLES, REACTIONS & HOVER BAR (1000x700)
# ==============================================================================
def generate_phase3_mockup():
    w, h = 1000, 700
    img = Image.new("RGB", (w, h), COLOR_CANVAS_BG)
    draw = ImageDraw.Draw(img)

    # Header Card
    draw_rounded_rect(draw, [40, 20, w - 40, 80], radius=10, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((60, 42), "Phase 3: Bong Bóng Chat Bo Góc Theo Cụm, Reactions Nổi & Hover Bar", font=get_font(16, bold=True), fill=COLOR_BRAND_BLUE)
    draw.text((60, 64), "Chuẩn mực tương tác Zalo PC / Olaz: Bo góc mượt mà, reaction nổi đè mép đáy, thanh công cụ bám sát", font=get_font(12), fill=COLOR_TEXT_SECONDARY)

    # Chat Stream Demo Area
    area_y = 100
    draw_rounded_rect(draw, [40, area_y, w - 40, h - 30], radius=12, fill=COLOR_WHITE, outline=COLOR_BORDER)

    # Clustered Incoming Message Stream
    draw_avatar(draw, 75, area_y + 40, 18, "T", bg_color=(16, 185, 129))
    draw.text((105, area_y + 22), "Trần Văn B", font=get_font(12, bold=True), fill=COLOR_TEXT_SECONDARY)
    
    # Message 1 in cluster (top: 12px, bottom: 4px)
    draw_rounded_rect(draw, [105, area_y + 38, 480, area_y + 82], radius=12, fill=COLOR_SIDEBAR_BG, outline=COLOR_BORDER)
    draw.text((120, area_y + 54), "Đây là tin nhắn đầu tiên trong cụm gửi liên tiếp.", font=get_font(13), fill=COLOR_TEXT_PRIMARY)
    draw.text((465, area_y + 72), "10:14", font=get_font(10), anchor="ra", fill=COLOR_TEXT_MUTED)

    # Message 2 in cluster (middle: 4px)
    draw_rounded_rect(draw, [105, area_y + 86, 520, area_y + 130], radius=6, fill=COLOR_SIDEBAR_BG, outline=COLOR_BORDER)
    draw.text((120, area_y + 102), "Tin tiếp theo có bo góc 4px ở góc trên-trái để tạo nhịp điệu cụm.", font=get_font(13), fill=COLOR_TEXT_PRIMARY)
    draw.text((505, area_y + 120), "10:14", font=get_font(10), anchor="ra", fill=COLOR_TEXT_MUTED)

    # Floating Reaction Pill on incoming message
    draw_rounded_rect(draw, [450, area_y + 120, 514, area_y + 144], radius=10, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((482, area_y + 132), "❤️ 👍 2", font=get_font(10, bold=True), anchor="mm", fill=COLOR_BRAND_BLUE)

    # Hover Action Bar bám sát cạnh bong bóng
    draw_rounded_rect(draw, [530, area_y + 92, 660, area_y + 124], radius=16, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((545, area_y + 108), "↩", font=get_font(12), anchor="mm")
    draw.text((575, area_y + 108), "↪", font=get_font(12), anchor="mm")
    draw.text((605, area_y + 108), "☺", font=get_font(12), anchor="mm")
    draw.text((635, area_y + 108), "⋯", font=get_font(14, bold=True), anchor="mm")

    # Clustered Outgoing Message Stream
    out_y = area_y + 170
    draw_rounded_rect(draw, [w - 520, out_y, w - 80, out_y + 50], radius=12, fill=COLOR_BRAND_BLUE_LIGHT, outline=COLOR_BRAND_BLUE_LIGHT)
    draw.text((w - 504, out_y + 18), "Tuyệt vời, bong bóng phía gửi đi dùng màu #E5F1FF chuẩn Zalo!", font=get_font(13), fill=COLOR_TEXT_PRIMARY)
    draw.text((w - 96, out_y + 38), "10:15", font=get_font(10), anchor="ra", fill=COLOR_TEXT_SECONDARY)

    # Floating Reaction Pill on outgoing
    draw_rounded_rect(draw, [w - 140, out_y + 40, w - 86, out_y + 64], radius=10, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((w - 113, out_y + 52), "👍 1", font=get_font(10, bold=True), anchor="mm", fill=COLOR_BRAND_BLUE)

    # Outgoing Status Detached below bubble
    draw.text((w - 86, out_y + 76), "✓✓ Đã xem", font=get_font(11), anchor="ra", fill=COLOR_TEXT_MUTED)

    # Quick Emoji Hover Strip demo
    strip_y = out_y + 120
    draw_rounded_rect(draw, [w - 380, strip_y, w - 80, strip_y + 46], radius=23, fill=COLOR_WHITE, outline=COLOR_BORDER)
    emojis = ["❤️", "👍", "😆", "😮", "😢", "😠"]
    for i, emo in enumerate(emojis):
        draw.text((w - 355 + i * 48, strip_y + 23), emo, font=get_font(18), anchor="mm")

    # Explanatory Notes
    note_y = strip_y + 80
    draw.text((60, note_y), "Ưu điểm đột phá của Phase 3:", font=get_font(14, bold=True), fill=COLOR_TEXT_PRIMARY)
    notes = [
        "1. Trạng thái gửi ('Đã gửi / Đã nhận / Đã xem') tách hoàn toàn khỏi nội dung tin nhắn, không làm phình bong bóng.",
        "2. Reaction Pill nổi ở mép dưới bong bóng (-8px offset), bấm vào mở danh sách người thả cảm xúc chi tiết.",
        "3. Thanh Hover bám sát mép bong bóng thay vì neo cứng ở rìa màn hình, hỗ trợ Trả lời / Chuyển tiếp 1-click.",
        "4. Bo góc thông minh gom nhóm các tin nhắn liên tiếp từ cùng một người gửi."
    ]
    for i, note in enumerate(notes):
        draw.text((60, note_y + 26 + i * 24), note, font=get_font(12), fill=COLOR_TEXT_SECONDARY)

    out_path = os.path.join(ASSETS_DIR, "phase3_bubbles_reactions.png")
    img.save(out_path, quality=95)
    print("Saved:", out_path)

# ==============================================================================
# 5. PHASE 4: RIGHT INFO DRAWER MULTI-VIEW STACK (1000x700)
# ==============================================================================
def generate_phase4_mockup():
    w, h = 1000, 700
    img = Image.new("RGB", (w, h), COLOR_CANVAS_BG)
    draw = ImageDraw.Draw(img)

    # Draw Two Screens side-by-side: Overview Mode vs Sub-view Mode
    card_w = 440
    
    # Screen A: Overview Mode
    draw_rounded_rect(draw, [40, 20, 40 + card_w, h - 30], radius=12, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((40 + card_w // 2, 45), "Chế độ Overview (Mặc định)", font=get_font(15, bold=True), anchor="mm", fill=COLOR_BRAND_BLUE)
    draw.line([40, 70, 40 + card_w, 70], fill=COLOR_BORDER, width=1)
    
    draw_avatar(draw, 40 + card_w // 2, 110, 26, "Z", bg_color=COLOR_BRAND_BLUE)
    draw.text((40 + card_w // 2, 148), "Nhóm Dự Án ZaloLite", font=get_font(14, bold=True), anchor="mm", fill=COLOR_TEXT_PRIMARY)
    
    # 4 Quick Action buttons
    for idx, (icon, lbl) in enumerate([("🔔", "Tắt thông báo"), ("📌", "Ghim chat"), ("👥+", "Thêm TV"), ("⚙️", "Quản lý")]):
        cx = 75 + idx * 95
        draw.ellipse([cx - 16, 170, cx + 16, 202], fill=COLOR_BRAND_BLUE_BG, outline=COLOR_BORDER)
        draw.text((cx, 186), icon, font=get_font(12), anchor="mm")
        draw.text((cx, 214), lbl, font=get_font(10), anchor="mm", fill=COLOR_TEXT_SECONDARY)
    
    draw.line([40, 235, 40 + card_w, 235], fill=COLOR_BORDER, width=1)
    
    accs = [
        ("👥 Thành viên nhóm", "24 người"),
        ("📋 Bảng tin nhóm", "3 ghim, 1 bình chọn"),
        ("🖼️ Ảnh / Video", "Xem tất cả (48)"),
        ("📎 File", "Xem tất cả (12)"),
        ("🔗 Link liên kết", "Xem tất cả (8)"),
        ("🔒 Thiết lập bảo mật", "E2EE, Tự xóa tin"),
    ]
    ay = 245
    for title, sub in accs:
        draw_rounded_rect(draw, [55, ay, 40 + card_w - 15, ay + 42], radius=6, fill=COLOR_CARD_HOVER)
        draw.text((70, ay + 14), title, font=get_font(12, bold=True), fill=COLOR_TEXT_PRIMARY)
        draw.text((40 + card_w - 35, ay + 14), sub, font=get_font(11), anchor="ra", fill=COLOR_TEXT_MUTED)
        draw.text((40 + card_w - 22, ay + 14), "›", font=get_font(14, bold=True), fill=COLOR_TEXT_MUTED)
        ay += 50

    # Screen B: Sub-view Mode (Media Storage Stack)
    b_x = 40 + card_w + 40
    draw_rounded_rect(draw, [b_x, 20, b_x + card_w, h - 30], radius=12, fill=COLOR_WHITE, outline=COLOR_BRAND_BLUE, width=2)
    
    # Header with Back button [← Quay lại]
    draw.text((b_x + 35, 45), "←", font=get_font(18, bold=True), anchor="mm", fill=COLOR_BRAND_BLUE)
    draw.text((b_x + 55, 45), "Kho lưu trữ Media & File", font=get_font(15, bold=True), anchor="lm", fill=COLOR_TEXT_PRIMARY)
    draw.line([b_x, 70, b_x + card_w, 70], fill=COLOR_BORDER, width=1)

    # 3 Storage Tabs: Ảnh/Video | File | Link
    draw.text((b_x + 40, 94), "Ảnh / Video", font=get_font(13, bold=True), fill=COLOR_BRAND_BLUE)
    draw.line([b_x + 30, 110, b_x + 110, 110], fill=COLOR_BRAND_BLUE, width=2)
    draw.text((b_x + 160, 94), "Tệp tin (File)", font=get_font(13), fill=COLOR_TEXT_SECONDARY)
    draw.text((b_x + 280, 94), "Link liên kết", font=get_font(13), fill=COLOR_TEXT_SECONDARY)
    draw.line([b_x, 112, b_x + card_w, 112], fill=COLOR_BORDER, width=1)

    # 4x2 Media Grid
    gy = 130
    for r in range(3):
        for c in range(4):
            gx = b_x + 20 + c * 98
            gy_curr = gy + r * 98
            draw_rounded_rect(draw, [gx, gy_curr, gx + 90, gy_curr + 90], radius=8, fill=COLOR_BRAND_BLUE_BG, outline=COLOR_BORDER)
            draw.text((gx + 45, gy_curr + 45), "🖼️", font=get_font(20), anchor="mm")

    # Bottom Callout: Triệt tiêu egui::Window
    draw_rounded_rect(draw, [b_x + 20, h - 160, b_x + card_w - 20, h - 50], radius=8, fill=COLOR_CARD_HOVER)
    draw.text((b_x + 35, h - 140), "Triệt tiêu 100% egui::Window rời rạc:", font=get_font(13, bold=True), fill=COLOR_BRAND_BLUE)
    draw.text((b_x + 35, h - 116), "• Điều hướng trang nội bộ với nút [← Quay lại].", font=get_font(12), fill=COLOR_TEXT_PRIMARY)
    draw.text((b_x + 35, h - 94), "• Không còn cửa sổ nổi che khuất khung trò chuyện.", font=get_font(12), fill=COLOR_TEXT_SECONDARY)
    draw.text((b_x + 35, h - 72), "• Trải nghiệm mượt mà, tối ưu hiệu năng ARM64.", font=get_font(12), fill=COLOR_TEXT_SECONDARY)

    out_path = os.path.join(ASSETS_DIR, "phase4_drawer_multiview.png")
    img.save(out_path, quality=95)
    print("Saved:", out_path)

# ==============================================================================
# 6. PHASE 5: CONTACTS VIEW & CENTERED MODALS (1000x700)
# ==============================================================================
def generate_phase5_mockup():
    w, h = 1000, 700
    img = Image.new("RGB", (w, h), COLOR_CANVAS_BG)
    draw = ImageDraw.Draw(img)

    # Contacts Sub-navigation on left (300px)
    sub_w = 300
    draw.rectangle([0, 0, sub_w, h], fill=COLOR_SIDEBAR_BG)
    draw.line([sub_w, 0, sub_w, h], fill=COLOR_BORDER, width=1)

    # Search bar at top
    draw_rounded_rect(draw, [15, 16, sub_w - 15, 50], radius=8, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((30, 33), "🔍", font=get_font(13), anchor="mm")
    draw.text((45, 33), "Tìm kiếm bạn bè, nhóm...", font=get_font(12), anchor="lm", fill=COLOR_TEXT_MUTED)

    # 4 Standard Zalo Contacts Items
    c_items = [
        ("👥", "Danh sách bạn bè", True),
        ("🏘️", "Danh sách nhóm và cộng đồng", False),
        ("📩", "Lời mời kết bạn", False),
        ("📨", "Lời mời vào nhóm", False),
    ]
    cy = 70
    for icon, label, is_active in c_items:
        bg = COLOR_BRAND_BLUE_LIGHT if is_active else COLOR_SIDEBAR_BG
        draw_rounded_rect(draw, [10, cy, sub_w - 10, cy + 46], radius=8, fill=bg)
        draw.text((25, cy + 23), icon, font=get_font(15), anchor="mm")
        fg = COLOR_BRAND_BLUE if is_active else COLOR_TEXT_PRIMARY
        draw.text((50, cy + 23), label, font=get_font(13, bold=is_active), anchor="lm", fill=fg)
        cy += 52

    # Centered Modal with Dimmed Backdrop on the right
    bg_dim_rect = [sub_w, 0, w, h]
    # Simulate dimmed background overlay
    draw.rectangle(bg_dim_rect, fill=(15, 23, 42))
    
    # Centered Modal Box (520x440)
    modal_w, modal_h = 520, 440
    mx = sub_w + (w - sub_w - modal_w) // 2
    my = (h - modal_h) // 2
    draw_rounded_rect(draw, [mx, my, mx + modal_w, my + modal_h], radius=14, fill=COLOR_WHITE)

    # Modal Header
    draw.text((mx + 25, my + 30), "Tạo nhóm trò chuyện mới", font=get_font(16, bold=True), fill=COLOR_TEXT_PRIMARY)
    draw.text((mx + modal_w - 25, my + 30), "✕", font=get_font(15, bold=True), anchor="ra", fill=COLOR_TEXT_MUTED)
    draw.line([mx, my + 54, mx + modal_w, my + 54], fill=COLOR_BORDER, width=1)

    # Modal Body
    draw_rounded_rect(draw, [mx + 25, my + 75, mx + modal_w - 25, my + 115], radius=8, fill=COLOR_WHITE, outline=COLOR_BORDER)
    draw.text((mx + 40, my + 95), "Nhập tên nhóm...", font=get_font(13), anchor="lm", fill=COLOR_TEXT_MUTED)

    draw.text((mx + 25, my + 140), "Chọn thành viên vào nhóm:", font=get_font(13, bold=True), fill=COLOR_TEXT_PRIMARY)
    
    # Member rows with checkboxes
    mem_y = my + 165
    for name, tag in [("Nguyễn Văn A", "Ban Quản Trị"), ("Trần Thị B", "Thiết kế"), ("Lê Văn C", "Kỹ thuật")]:
        draw_rounded_rect(draw, [mx + 25, mem_y, mx + modal_w - 25, mem_y + 46], radius=6, fill=COLOR_CARD_HOVER)
        draw.rectangle([mx + 40, mem_y + 16, mx + 54, mem_y + 30], fill=COLOR_BRAND_BLUE)
        draw.text((mx + 47, mem_y + 23), "✓", font=get_font(10, bold=True), anchor="mm", fill=COLOR_WHITE)
        draw_avatar(draw, mx + 80, mem_y + 23, 15, name[0], bg_color=COLOR_BRAND_BLUE)
        draw.text((mx + 105, mem_y + 23), name, font=get_font(13), anchor="lm", fill=COLOR_TEXT_PRIMARY)
        draw.text((mx + modal_w - 40, mem_y + 23), tag, font=get_font(11), anchor="ra", fill=COLOR_TEXT_MUTED)
        mem_y += 52

    # Modal Footer
    draw.line([mx, my + modal_h - 60, mx + modal_w, my + modal_h - 60], fill=COLOR_BORDER, width=1)
    draw_rounded_rect(draw, [mx + modal_w - 190, my + modal_h - 48, mx + modal_w - 110, my + modal_h - 14], radius=6, fill=COLOR_SIDEBAR_BG, outline=COLOR_BORDER)
    draw.text((mx + modal_w - 150, my + modal_h - 31), "Hủy", font=get_font(13), anchor="mm", fill=COLOR_TEXT_PRIMARY)
    
    draw_rounded_rect(draw, [mx + modal_w - 95, my + modal_h - 48, mx + modal_w - 20, my + modal_h - 14], radius=6, fill=COLOR_BRAND_BLUE)
    draw.text((mx + modal_w - 57, my + modal_h - 31), "Tạo nhóm", font=get_font(13, bold=True), anchor="mm", fill=COLOR_WHITE)

    out_path = os.path.join(ASSETS_DIR, "phase5_contacts_modals.png")
    img.save(out_path, quality=95)
    print("Saved:", out_path)

if __name__ == "__main__":
    generate_overview_mockup()
    generate_phase1_mockup()
    generate_phase2_mockup()
    generate_phase3_mockup()
    generate_phase4_mockup()
    generate_phase5_mockup()
    print("All UI demo mockups generated successfully!")
