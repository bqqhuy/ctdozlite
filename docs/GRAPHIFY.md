# Graphify cho rà soát mã

Graphify là công cụ phát triển: lập đồ thị cấu trúc Rust để tra cứu module, symbol,
call và vùng chịu ảnh hưởng khi sửa. Không thêm dependency Cargo hoặc thành phần vào
bản cài Windows ARM64. Đồ thị hỗ trợ tìm đường đọc mã; không chứng minh quyền truy cập,
độ đúng giao thức, luồng runtime hoặc thay thế test/capture Zalo thật.

## Cài và chạy

Đã kiểm tra Graphify `0.9.103`, source commit
`16218bfc80d4ca2799734d045039f7ed1a5fa9fb` từ
[Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify).

```bash
python3 -m venv /tmp/ctdozlite-graphify-venv
/tmp/ctdozlite-graphify-venv/bin/pip install 'graphifyy @ git+https://github.com/Graphify-Labs/graphify.git@16218bfc80d4ca2799734d045039f7ed1a5fa9fb'
GRAPHIFY_BIN=/tmp/ctdozlite-graphify-venv/bin/graphify ./scripts/graphify.sh
```

Script chỉ chạy AST/Cargo cục bộ (`--code-only`), không gọi LLM hoặc gửi source lên
API. Không tự cài git hook, MCP server hoặc sửa cấu hình agent toàn máy.
`.graphifyignore` loại dữ liệu runtime, session, log, dump, vendor và build products.
Không thêm `--no-gitignore` để lấy dữ liệu bị bỏ qua vào đồ thị.

Kết quả nằm ở `graphify-out/` và đã được Git bỏ qua. Chạy lại sau thay đổi; AST cache
chỉ giúp giảm thời gian trích xuất, cần kiểm tra đồ thị có cập nhật trước khi dựa vào nó.
Khi chủ động xóa nhiều source, dùng `./scripts/graphify.sh --force` để cho phép đồ thị nhỏ lại.

## Tra cứu

Chạy tại root repo, dùng binary trong venv ở trên:

```bash
graphify query 'hidden_chat' --budget 1500
graphify affected 'src/hidden_chat.rs' --depth 2
graphify god-nodes --top 10
graphify tree --label ctdozlite
```

Nếu binary không có trên PATH, thay `graphify` bằng đường dẫn venv. `tree` tạo HTML
để xem cấu trúc; truy vấn có thể yêu cầu chọn symbol khi tên trùng. Các cạnh AST là
bằng chứng tĩnh, không coi cạnh suy diễn hoặc kết quả không tìm thấy là kết luận lỗi.
Không bật trích xuất ngữ nghĩa bằng LLM cho tài liệu/ảnh; roadmap được nhập có nguồn theo mục dưới.

## Kết quả áp dụng trên working tree v0.008

Lượt đầu sau khi loại vendor/bundle đối chiếu: 179 file code được nhận diện,
4.651 node, 11.366 cạnh, 175 cộng đồng. Đã tạo `graph.json`, `GRAPH_REPORT.md`,
`graph.html` và `GRAPH_TREE.html`; thử truy vấn hidden_chat thành công. Truy vấn
rộng bị cắt theo token budget, nên dùng file/symbol cụ thể trước khi kết luận.
Tên cộng đồng giữ dạng số (`--no-label`), không nhờ LLM đặt tên.
Cargo được đọc ở cấp workspace: 1 node, 0 cạnh; đây không phải toàn bộ dependency graph
của Cargo.lock. `src/phone_sync/mod.rs` chỉ khai báo module nên extractor không lấy
symbol ở file này; vẫn cần đọc các file con.

Script được kiểm tra cú pháp Bash; không đổi mã Rust hay chạy lại build ứng dụng.

## Roadmap và truy vết yêu cầu

Đã bổ sung cả bốn phiên bản theo cách gộp đã chốt:

| Phiên bản | Khối giữ lại | Trạng thái trong đồ thị |
| :--- | :--- | :--- |
| v0.008-arm | R0/R8/R9/R10 | Có mã/fixture; nghiệm thu thiết bị còn mở |
| v0.009-arm | R11/R12/R13 | Kế hoạch |
| v0.010-arm | R14 | Kế hoạch |
| v0.011-arm | R15 | Kế hoạch |

Lớp này bao gồm roadmap, bốn release plan, progress v0.008, specs001–008 và bằng
chứng re-notes/ADR được liệt kê rõ trong `tools/graphify/roadmap.json`.
Không quét toàn ổ đĩa, log hoặc dữ liệu tài khoản để suy luận quan hệ.

Chuỗi truy vết: **release → khối → gap / spec → requirement → task / code / test**.
Các mã A01–A20, B01–B12, C01–C04 được kiểm tra không mất khỏi phân bổ.
B02/C02 có phạm vi ở nhiều khối; C04 là tương thích xuyên suốt. Mã yêu cầu/task
được đặt trong namespace từng spec, tránh lẫn A01 baseline với A-01 của spec008.
Dependency R14→R13 chỉ nói cần nghiên cứu mô hình khóa, không kết luận zCloud
và chat E2EE dùng cùng giao thức.

- `graphify-out/ROADMAP_REPORT.md`: bảng yêu cầu/mã/test và task còn mở.
- `graphify-out/ROADMAP_INDEX.json`: số liệu, phân bổ release và SHA256 tài liệu đầu vào.
- `graphify-out/graph.json`, `graph.html`, `GRAPH_TREE.html`: đồ thị đã gộp cả hai lớp.
- `graphify-out/ast/graphify-out/`: đồ thị AST riêng; giữ tách để lần cập nhật mã không
  deduplicate hoặc làm mất node roadmap. Dùng script để tái tạo, thay vì chạy `extract .`
  trực tiếp vào đồ thị đã gộp.

Ví dụ tra cứu có điểm xuất phát rõ, tránh kết quả quá rộng:

```bash
graphify query 'phạm vi v0.010' --seed 'roadmap:release:v0.010-arm' --depth 2 --budget 2000
graphify query 'PIN và test' --seed 'roadmap:requirement:008-account-group-state:A-02' --depth 1 --budget 2500
graphify query 'nghiệm thu cuộc gọi' --seed 'roadmap:task:007-call-multidevice:C-T090' --depth 1
```

Cạnh `implementation_context` là liên kết được rà thủ công trong manifest, không phải
chứng nhận đã đáp ứng toàn yêu cầu. `has_regression_source` kiểm tra đúng symbol AST
và có `#[test]` trong source, không tự chạy hay đánh dấu test qua. `mentions_code` chỉ
trích đường dẫn được task nhắc đến. Checkbox task được giữ nguyên; task checked
không thay trạng thái nghiệm thu trên thiết bị. Bảng gap là baseline v0.007.
Bindings có chọn lọc, thiếu cạnh không chứng minh không có implementation/test.

Sau khi sửa kế hoạch/spec/verification hoặc code, chạy lại `scripts/graphify.sh`.
Chỉ sửa nội dung tài liệu gốc và manifest, không sửa output để đánh dấu hoàn tất.
Ở đồ thị lớn Graphify có thể gom cộng đồng trong `graph.html`; `GRAPH_TREE.html`
và CLI vẫn cho tra cứu node chi tiết. Không đưa các output lớn vào Git.

Kiểm chứng lớp truy vết:

```bash
python3 -m unittest discover -s scripts/tests -v
bash -n scripts/graphify.sh
git diff --check
```

Đã kiểm tra tái tạo idempotent, loại node/cạnh task cũ, giữ task thiết bị mở,
namespace trùng ID, phát hiện thiếu gap/test binding và chặn đường dẫn ra ngoài repo.
Việc thêm lớp roadmap không đổi mã Rust hoặc trạng thái build/release.
