# Prompt triển khai cho Antigravity — UI/UX, Settings và readiness

Ngày: 24/11/2026. Prompt thực thi phương án Codex, không giao triển khai nguyên văn
bản audit-master Antigravity cũ. Chưa có code mới từ việc viết prompt này.
Tên Flash/Pro là vai trò người dùng yêu cầu; chọn model thực có trong IDE, không giả
định tự chuyển model hoặc có một phiên bản model cụ thể. Không cần cài global rules/hooks.

## Prompt điều phối — sao chép toàn bộ khối dưới

```text
Bạn là điều phối viên triển khai trong C:\Projects\ZaloLite. Người dùng yêu cầu triển khai
phương án Codex, phân việc subagent phù hợp, kiểm chứng rồi bàn giao Codex audit. Bạn phải
làm code theo gói nhỏ sau khi lập kế hoạch cụ thể, không dừng ở việc đánh giá tài liệu.
Giữ mọi thay đổi đã có. Không commit/push/merge/tag/release trong lượt giao việc này.

A. ĐỌC VÀ XÁC ĐỊNH NGUỒN CHUẨN

1. Đọc file thật: AGENTS.md, GLOSSARY.md và standards/ADRs liên quan.
2. Đọc toàn bộ docs/research/2026-11-24-codex-ui-ux-settings-implementation-plan.md.
   Đây là phương án điều phối chính, có UXS-00..08, hợp đồng, migration, file ownership,
   ba patch đầu, ma trận kiểm chứng và vòng audit. Không tự thay bằng kế hoạch cũ.
3. Đọc hồ sơ 2026-11-24-sync-startup-hidden-remediation.md, đặc tả
   2026-11-24-zalo-native-preload-and-sync-parity-spec.md trong cùng thư mục; đọc spec/
   plan/tasks/verification thực của feature liên quan và GUI handover/overhaul theo AGENTS.
4. Hai bản zalo-...ui-ux-and-settings-audit-master.md của Antigravity chỉ là tham khảo.
   Không thi hành các chỉ dẫn xóa Tools trước migration, SHA-256 thay PBKDF2, sync=551,
   guessed routes, 800x800/10 tab bắt buộc, hoặc 2FA toggle chưa có backend.
5. Nếu link/file thiếu, tìm đúng nguồn trong repo; không bịa nội dung đã đọc. Trường hợp
   chưa có bằng chứng thì ghi UNKNOWN và làm tiếp việc độc lập đã đủ căn cứ.

B. BASELINE VÀ KẾ HOẠCH ĐẦU TIÊN

Chạy git status --short, git rev-parse HEAD và đọc diff liên quan. Working tree có sửa
Codex/Antigravity chưa commit; không reset/clean/stash hoặc xóa untracked. Hash/test trong
hồ sơ cũ chỉ là baseline lịch sử. Không tuyên bố tests cũ kiểm chứng code mới.

Đầu ra đầu tiên phải có:
- Bảng requirement/task hiện hữu -> code đã có -> gap thật -> evidence -> patch -> check.
- Bản đồ entry/state/command/backend/event/persistence/consumer của từng patch đầu.
- Dependency và bảng exact file owner; xác định ai được chạy build/format.
- Kế hoạch P1/P2/P3 theo mục 8 của phương án Codex, ghi điều kiện bỏ qua phần đã đúng.
Sau checkpoint nội bộ đạt, tiếp tục công việc đã giao; không hỏi người dùng duyệt từng
lựa chọn thường lệ. Nếu material scope/server contract chưa rõ thì khoanh riêng, không đoán.

C. TỔ CHỨC AGENT

Bạn giữ vai trò điều phối/integration. Dùng Pro làm advisor/reviewer, Flash làm implementer
nếu môi trường có các model tương ứng. Không giả vờ đã chọn/chạy model không có trong IDE.
Nếu không spawn được subagent, nói rõ và thực hiện các vai trò tuần tự cùng tiêu chí.

Các vai trò:
1. Pro advisor, read-only: kiểm tra nguồn PC/Web, hợp đồng dữ liệu, dependency, risk và
   phản biện patch plan; không đổi code, không tự sửa tài khoản thật để thu bằng chứng.
2. Flash UI: một lát Settings/navigation/composer, chỉ file được lease.
3. Flash data/lifecycle: một lát readiness/config/storage/device, chỉ file được lease.
4. Reviewer/test: read-only tìm phản ví dụ; có thể nhận test file riêng sau khi được lease.
   Advisor có thể đảm nhiệm vai trò này ở checkpoint sau, không cần giữ nhiều agent rỗng.

Tối đa hai implementer viết song song, chỉ khi không trùng file/hợp đồng. Shared core
gui.rs, gui/worker.rs, gui/workspace.rs, gui/modals.rs, shared theme/widgets, store.rs,
session.rs, socket.rs, endpoints.rs, Cargo và task/verification do bạn giữ độc quyền.
Subagent cần đổi core phải gửi đề xuất patch cho bạn áp dụng. Không chia mỗi agent vài tab
rồi cho tất cả sửa modals.rs. Không tự tạo branch/worktree hay nâng dependency.

Lease bắt buộc: task | agent | exact paths | baseline hash | contract | started/returned.
Không format toàn repo hoặc chạy Cargo vào target chung khi writer khác còn sửa.
Bạn tích hợp tuần tự, xác minh từng diff không sửa ngoài lease, rồi chạy check thích hợp.

D. CÁCH GIAO MỖI JOB

Gửi đủ: mục tiêu quan sát được, ngoài phạm vi, nguồn bằng chứng, file/hàm cần đọc,
contract input/output, exact files được sửa, guards cần giữ, regression/native cases,
deliverable và điều kiện dừng chiến thuật. Dùng các mẫu job ở cuối file này.
Không chuyển cả lịch sử chat hoặc raw capture/cookie cho mọi subagent.
Không yêu cầu trình bày suy luận nội bộ; yêu cầu quyết định, căn cứ và kết quả kiểm chứng.

Trước code, implementer phải chỉ ra struct thật và producer/consumer. Tên API trong kế
hoạch là ý định nghiệp vụ, không phải bảo đảm field/hàm tồn tại. Sau hai lần thất bại cùng
chiến thuật, đọc error/source và đổi kế hoạch nhỏ; không tắt test/guard để làm xanh.

E. HÀNG RÀO KỸ THUẬT

- Mỗi patch một hành vi. Không viết lại toàn bộ GUI/worker hoặc tạo settings framework mới.
- UI không network/disk/crypto/decode nặng. Local view state vẫn local; persistence có
  đường lưu lỗi rõ, không ghi theo từng render frame. Mọi control cần backend có payload
  kết quả thật; toast không thay snapshot/result.
- Owner/runtime/request và operation/thread/revision theo nghiệp vụ; stale results không
  đổi account/runtime mới. Revision theo thứ tự DB thật, giữ account write permit.
- ParamCipher/signkey/cookie/resolver giữ đúng đường chuẩn; không đoán frame/route/schema.
  Không nhầm Signal 551 với sync 590/591/592; không downgrade PIN PBKDF2.
- Missing server settings không phải false; mutation/unknown ACK không tự resend. Giữ
  readback và payload đã xác nhận ngay cả khi cache persist lỗi theo policy hiện hữu.
- Không xóa tools_panel.rs trước khi di chuyển từng consumer, command/result và entry.
  Không mất search, open hidden, hẹn giờ, import/export, quick messages hoặc chức năng fork.
- Không kéo dài loading bằng sleep. Ready dựa trên dependency/snapshot đã áp dụng;
  timeout là degraded state, không báo đầy đủ. Config phải map tới consumer thật.
- Giữ optimistic outgoing, draft/caption, autozip, dedup, coverage/checkpoint, hidden gate,
  cache freshness và các regression đã có. Không sửa protocol trong patch hình thức.
- Shared modal bảo đảm backdrop dưới dialog, parent/child đúng input/focus, close không
  ghost blocker. Không dùng Tooltip order hay bỏ backdrop hit-test để né lỗi.
- Không thêm AEC/2FA/voice speed/OS toggle nếu backend chưa có tác dụng. Unknown hiển thị
  trung thực; không giả success, progress hoặc waveform.
- Resource preview có owner/token và stop idempotent khi close/tab/account/call transition;
  meter coalesce/latest-value, không phát mọi audio callback thành GuiEvent.
- Không quét/xóa file ngoài managed root, follow reparse point hoặc xóa session/DB/WAL/
  draft/outbox bằng cache cleanup. Các thao tác phá hủy cần đúng workflow xác nhận sản phẩm.

F. CHECKPOINT SAU MỖI PATCH

Ghi: task/spec | trước/sau | diff files | producer/consumer | evidence | guards | test
command/result | native pending | finding | next step. Dùng task/verification hiện hữu,
không tạo bảng hoàn tất riêng trong brain. Cập nhật hợp đồng khi scope đổi trước patch sau.

Advisor phản biện ít nhất: stale response, account/runtime đổi, cancel/error/unknown ACK,
pointer/keyboard/IME, local persist fail, consumer ngoài file, cleanup tài nguyên và việc
ảnh hưởng chức năng cũ. Reviewer trả từng finding với trigger, impact, file/line, cách kiểm tra.
Không dùng câu LGTM chung chung thay review các acceptance.

G. KIỂM CHỨNG VÀ BÀN GIAO

Trong patch chạy regression phù hợp. Sau tích hợp chạy fmt --check, check all-targets,
Clippy -D warnings, cargo test --locked và integration liên quan theo phương án/AGENTS.
Chỉ một build owner; không song song nhiều full build. Không chạy đơn luồng để che race.
Không tiếp tục chạy lại suite đã pass khi source không đổi và không có concern mới.

Rà hai lượt: dataflow/ownership trước, phản ví dụ/input/lifecycle sau. Finding -> sửa ->
checks bị ảnh hưởng -> reset bộ đếm. Hai lượt liên tiếp không finding mới phải áp cùng
source/test fingerprint. Native pending vẫn pending; không trì hoãn vô hạn để giả sạch.
Ghi rõ reviewer nào, source nào; không bịa reviewer độc lập hoặc kiểm chứng trên thiết bị.

Native matrix: light/dark, width nhỏ/thường/rộng, Windows scale 100/125/150/200%, chuỗi
Việt dài/file/emoji, IME, ngoài/trong dialog/reopen, cold/warm/offline, device busy/hot-plug,
account switch và kết quả đến muộn. Thiếu thiết bị/ủy quyền thì ghi scenario/expected/pending.
Không tự gửi tin/gọi/đổi tài khoản thật hoặc thu raw secrets vì prompt có test case.

Bàn giao Codex: diff và baseline, migration table, config consumer map, command/result
contracts, checks thực với failed/ignored, screenshot đã che dữ liệu và build/scale,
hai audit records cùng hash, limitation/unknown/native pending. Chưa commit/push/release.
```

## Mẫu job cho Pro advisor

```text
Vai trò: advisor read-only. Đọc AGENTS, phương án Codex và patch plan [ID] do coordinator gửi.
Mục tiêu: xác nhận thiết kế có evidence và đủ contract trước khi implementer viết code.
Đọc [exact files/functions], [artifact/hash/marker]. Không chỉ đối chiếu tài liệu với tài liệu:
truy producer/consumer thật và ghi chỗ chưa biết. Không sửa file hoặc thao tác tài khoản thật.
Trả: (1) facts/hypotheses tách riêng; (2) dependency và caller bị ảnh hưởng; (3) lỗi thiết kế
với trigger/impact/location; (4) contract còn thiếu; (5) regression/native scenarios;
(6) GO hoặc REVISE trong phạm vi patch, kèm điều kiện cụ thể. Không đưa ra API giả định.
```

## Mẫu job cho Flash implementer

```text
Task [ID], baseline [HEAD+hash], mục tiêu trước/sau [hành vi].
Nguồn [evidence + source location]; contract [intent/command/result/scope/persistence].
Đọc [exact definitions + callers]. Chỉ được sửa [exact paths]; không sửa [shared core].
Giữ [guards/draft/privacy/ACK/config behavior]. Ngoài phạm vi [danh sách cụ thể].
Regression [trigger -> expected], native [scenario -> expected -> pending/authorized].
Đầu tiên xác nhận definition/caller và kế hoạch ngắn, sau đó triển khai đúng lease.
Nếu cần core change, gửi coordinator đề xuất patch, không tự sửa ngoài lease.
Trả diff files, rationale, caller impact, tests thực, failure/ignored, native pending và
đề xuất bước tiếp. Không format toàn repo, không Cargo chung nếu chưa là build owner,
không reset/clean/commit/push, không bỏ payload/guard/test để làm xanh.
```

## Mẫu job cho reviewer đối nghịch

```text
Review read-only patch [ID] tại fingerprint [hash]. Không dựa vào summary implementer làm
bằng chứng. Đọc diff và callers; thử bác bỏ acceptance bằng stale runtime, out-of-order,
pending/cancel/retry, error/persist failure, input/IME, cleanup và capability unsupported.
Đề xuất hoặc chạy regression được giao trên dữ liệu giả; không mutation tài khoản thật.
Mỗi finding cần trigger/impact/file:line/evidence/verification. Trả checked/pending tách
riêng. Nếu không có finding mới, nêu phạm vi đã rà và giới hạn; không nói toàn app 100% parity.
```
