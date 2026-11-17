# Antigravity — phương án sửa gửi chậm và lỗi mạng

Ngày 18/11/2026, Asia/Saigon. Trạng thái: **đề xuất triển khai, chưa sửa mã/cấu hình**. Đọc cùng `AGENTS.md`, `GLOSSARY.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, standards API/error, concurrency, security, testing và các ADR liên quan gửi tin, identity, retry. Không tự commit/push/release từ tài liệu này.

## 1. Kết luận và phạm vi

Ưu tiên hai thay đổi độc lập: (A) chọn REST host theo service map thay vì ép host cố định; (B) giới hạn tổng thời gian chờ preview link. Sau đó mới thêm failover/retry có phân loại và reconciliation. Không tăng timeout, không đổi toàn bộ host sang một legacy host, không retry tất cả POST.

Snapshot đọc: HEAD `9339a75`, working tree có nhiều chỉnh sửa UX/UI chưa commit (bao gồm `src/chat.rs`); phải kiểm tra lại source và binary đang chạy trước triển khai. Không ghi đè thay đổi của agent khác. Tài liệu UX/UI liên quan: [bàn giao 17/11](ZALO_PC_UI_UX_ANTIGRAVITY_HANDOFF_2026-11-17.md).

## 2. Sổ bằng chứng

| ID | Nguồn | Phát hiện | Giới hạn |
|---|---|---|---|
| N01 | DNS/curl trên máy trong phiên khảo sát | `tt-chat-wpa.chat.zalo.me` không resolve ở hai lần curl; Resolve-DnsName báo DNS name does not exist | Kết quả tại thời điểm kiểm tra; không chứng minh DNS toàn cầu hay mọi lần gửi |
| N02 | curl HEAD không cookie, không params | Legacy `tt-chat3-wpa.api.zaloapp.com` và group `tt-group-wpa.chat.zalo.me` kết nối HTTPS, trả 404 ở `/` | 404 root chỉ chứng minh DNS/TCP/TLS/HTTP tới host, không chứng minh API gửi hoạt động/đúng schema/entitlement |
| F01 | `src/endpoints.rs` service chat; `session.rs::prefer_verified_rest` | Host chat mặc định là N01; cấu hình REST được ưu tiên lại về canonical host, giữ một số host discovered ở fallback | Chưa chứng minh host thực tế của request lỗi trong binary người dùng |
| F02 | `session.rs::send` | Retry candidate khi HTTP 404; GET được retry connect/timeout; POST connect/timeout trả lỗi | Không được suy ra không có bất kỳ retry nào ở mọi tầng khác |
| F03 | `session.rs` HTTP client | Connect timeout 10s, tổng request 35s; upload có override riêng | Không phải số đo latency thật; không áp chung cho upload |
| F04 | `outgoing.rs::TextRoute::Link`, `chat.rs::parse_link_metadata` | Gửi link chờ parselink; thiếu metadata có thể tiếp tục scrape OpenGraph. Scraper timeout 3.8s | GET fallback nhiều candidate có thể cộng thêm chờ; chưa đo toàn đường chạy thật |
| F05 | `outgoing.rs::failure_message` | is_connect ánh xạ sang “Không kết nối được mạng. Kiểm tra kết nối rồi gửi lại.” | Lỗi connect không đủ kết luận máy mất Internet; có thể DNS/proxy/TLS/host |
| F06 | `outgoing.rs::send`, `group_state.rs` | Trước gửi nhóm có await snapshot quyền; cache fresh 15s | Không bỏ kiểm tra quyền để giảm chậm; cần đo cache hit/miss |
| P01 | E2, bundle PC 26.11.12 trong app.asar cài máy | Domain Manager có danh sách host theo service; host đầu danh sách là main/active; enable_swap_domain điều khiển switching | Runtime feature/config có thể khác default |
| P02 | E2, Domain Manager + API wrapper | Nhánh HTTP 4xx/5xx gọi switchDomain; rotation vòng, cooldown 5s; lấy active domain sau 5 phút có thể revert main | Không chứng minh DNS failure cũng kích hoạt rotation; switch không đồng nghĩa replay ngay request vừa lỗi |
| P03 | E2, `_get`, `_post` | Timeout mặc định API 10s, có override | Không phải mọi request đều 10s, không phải latency live |
| P04 | E2, send queue / retryConfig | Retry theo network state, error groups, budget/count/backoff; URL có endpointInfo được resolve lại trước lần thực thi | Config từ server có thể override; chưa đọc effective config phiên hiện tại |
| P05 | E2, composer + ParserConfig + preview executor | Preview có thể bắt đầu lúc soạn, có cache; khi phải đợi preview lúc gửi, dùng max_delay_send_message mặc định 2s. Executor resolve phần kết quả hiện có khi hết budget; tin có thể giữ text nếu chưa có preview | Nhánh khác (ví dụ upload thumb) có logic riêng; chưa đo live hoặc xác nhận config effective |
| L01 | UI PC thật, người dùng cho phép 2 test vào Henry | Đã soạn test text; gọi Enter thì mất cửa sổ, chưa xác nhận gửi thành công; chưa gửi test link | **Không resend tự động**, không dùng lần này làm benchmark |
| L02 | Windows Application events 1000/1001 + người dùng xác nhận tự đóng | Zalo.exe 26.11.12.3564 có crash, thấy exception 80000003/c000041d | Chưa xác định root cause, không quy cho mạng hoặc công cụ; tách khỏi lỗi fork |

Không có capture traffic E1 gửi tin trong phiên này. Log fork mặc định tìm được chỉ có sync/media, không đủ đối chiếu lần gửi. Nhận định “nguyên nhân mạnh” khác với “đã xác nhận nguyên nhân toàn bộ lỗi”.

### Nguồn PC có thể tái lập

ASAR: `C:/Users/bqqhu/AppData/Local/Programs/Zalo/Zalo-26.11.12/resources/app.asar`.

| Entry | SHA-256 nội dung JS |
|---|---|
| `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js` | `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08` |
| `pc-dist/lazy/main-startup.41abf8e6af2a55551e3b.js` | `c29c9f0a7f0718c5b044764666b6fc5dec8b8eb65e178819b3cee3b1f93e3426` |

Chỉ số ký tự Python trong compact: send queue khoảng 1939000–1948500; composer parse gate khoảng 5056100–5058700; retry defaults khoảng 8058144 (setting_feature=0, max_retry_count=2, retry_err_no_network=false là **defaults**, không kết luận runtime); request helper 15037350; `_request` khoảng 15041711; `_post` 15043205; preview executor withTimeout 17450194; Domain Manager khoảng 17992000–17998800. Trong main-startup: ParserConfig max_delay_send_message mặc định `timeInMs("2s")` khoảng 1526659. Kiểm tra hash trước khi dùng offset; không commit bundle VNG đầy đủ hoặc dữ liệu tài khoản.

## 3. Phương án tối ưu theo thứ tự

### A. Chọn host đúng, giữ thay đổi nhỏ

1. Trace `apply_server_info` → Domains → prefer_verified_rest → URL builder → send; xem cả restore Session và ServiceResolver. Chỉ có một quyết định authoritative cho candidate order, tránh hai resolver lưu winner khác nhau.
2. Service map hợp lệ của tài khoản/phiên là nguồn ưu tiên; canonical trong endpoints.rs là bootstrap/compatibility fallback có bằng chứng, không tự ép đè discovered host hoạt động. Giữ allowlist/trusted URL, HTTPS, loại credentials/path bất hợp lệ và giới hạn candidate. Không suy host bằng đổi hậu tố domain.
3. Trước mutation, có thể xác minh đường kết nối candidate bằng cơ chế read-only phù hợp; cache health TTL ngắn theo account/runtime/service và single-flight, không probe mọi lần gửi. Không lấy HTTP 404 ở root làm chứng nhận API hợp lệ. Không chặn send thường bằng một loạt probe tuần tự.
4. Với lỗi chắc chắn trước gửi (ví dụ DNS resolution thất bại), đánh dấu candidate unhealthy tạm thời, chọn candidate hợp lệ khác trong tổng deadline; xác minh classification từ source error thực tế. `reqwest::is_connect()` riêng lẻ không đủ phân biệt tất cả giai đoạn an toàn replay.
5. Không copy quy tắc PC rotate mọi 4xx/5xx vào fork một cách máy móc: auth, rate limit, policy rejection, 404 route và host failure có ý nghĩa khác. Rotation candidate cho request sau khác với replay mutation hiện tại.
6. Invalidate routing/health khi service map mới, logout/account switch/runtime thay đổi. Không lưu credential hoặc query params trong health diagnostics.

### B. Tách preview khỏi critical path gửi

1. Khi soạn, debounce parse/cache metadata theo URL, hủy/bỏ stale result khi draft/thread/account đổi; không tự phát request theo mỗi ký tự. Tôn trọng privacy và policy URL hiện có.
2. Khi Send: dùng preview hợp lệ đã có. Nếu còn chờ, áp **ngân sách tổng hữu hạn**, đề xuất 2s theo PC default P05; đây là đề xuất parity, không phải effective config PC đã đo.
3. Hết budget mà thiếu preview: gửi text chứa link qua đường text thường đã có contract, không chặn thêm vì thumbnail/description. Cần tách explicit route/payload, tránh gọi lại route_text rồi tự vòng vào Link. Không dựng rich-link payload nửa hợp lệ. Giữ E2EE route fail-closed; không downgrade encrypted text.
4. Không tiếp tục OpenGraph scrape đồng bộ trên critical path sau budget. Background preview là tùy chọn có kiểm soát; không tự sửa/gửi lại message đã gửi chỉ để thêm preview nếu chưa có contract.
5. Giữ client message identity của cùng intent, snapshot text/mentions/quote; preview không được làm mất mention/reply. Test plain/direct/group/E2EE, nhiều link và URL không parse được.

### C. Timeout và retry theo trạng thái, không retry mù

Phân biệt budget chờ hàng đợi, group policy, preview, connection, HTTP attempt, body/ACK và tổng intent. Đề xuất request text ngắn có tổng attempt gần 10s sau đo/fixture; đừng đổi 35→10 cho mọi upload/download. Giữ một tổng deadline để số host/retry không nhân thời gian vô hạn.

| Lỗi/trạng thái | Chính sách đề xuất |
|---|---|
| DNS fail hoặc xác nhận chưa phát mutation | Failover có giới hạn sang candidate hợp lệ; giữ intent ID; ghi lý do |
| Timeout/reset sau khi có khả năng gửi request | Trạng thái chưa rõ; đối soát echo/ACK/history theo identity trước khi quyết định resend |
| Server trả rejection rõ | Hiển thị đúng auth/policy/rate limit/validation; không đổi host và retry vô điều kiện |
| Server echo tới trước HTTP response | Reconcile cùng client identity, không tạo bubble thứ hai; response muộn không đảo Sent thành Failed |
| Offline/đang kiểm tra mạng | Bảo toàn intent; hiển thị trạng thái phù hợp; chỉ resume theo policy đã thiết kế và bounded budget |
| User retry | Cùng intent/identity theo invariant repo; xác minh trạng thái hiện có, không chạy hai job song song |

PC có retry mutation có điều kiện; câu “PC không retry mutation” trước đó là quá rộng và không được dùng làm thiết kế. Tuy nhiên bản thân retry của PC không chứng minh fork có đủ dedup/reconciliation để bật tương tự.

### D. Đo và hiển thị lỗi đúng

Thêm diagnostics tối thiểu, typed và bounded: service key, candidate index/host được allowlist (không URL query), DNS/connect/TLS/attempt class, elapsed từng phase, cache hit, retry count, transport/server result và intent tag đã băm. Không log text/link thật, UID, cookie, params, keys hoặc full request/response. Đừng bật raw dump để chẩn đoán thường xuyên.

UI phân biệt đang chuẩn bị/đang gửi/chưa rõ xác nhận/bị từ chối. Không mặc định mọi connect error là máy mất Internet. Giữ thông báo ngắn cho người dùng, chi tiết kỹ thuật trong diagnostics. Không báo Failed chỉ vì chưa nhận ACK nếu echo đã xác nhận; không báo thành công từ HTTP 200 thiếu server confirmation.

Đo riêng group permission snapshot cache hit/miss; giữ kiểm tra quyền, chỉ tối ưu bằng metadata/cache invalidation hợp lệ. Không bỏ guard vì tốc độ.

## 4. Gói triển khai và kiểm chứng

| Gói | Thay đổi review được | Chứng minh cần có |
|---|---|---|
| 1 | Routing source of truth + diagnostics tối thiểu | Service map mới được tôn trọng; restore/account switch đúng; invalid map bị từ chối |
| 2 | Preview bounded + plain-text fallback | Preview treo không giữ Send quá budget đề xuất; chỉ một mutation; mentions/quote/E2EE không mất |
| 3 | Failure classification + bounded failover/reconciliation | DNS-before-send failover; ambiguous timeout không resend mù; late echo không nhân tin |
| 4 | Timeout tuning + UX status | Số đo trước/sau, phase timings; uploads không bị áp timeout text |

Dùng fake HTTP/DNS/transport hoặc fixture cách ly, không sửa DNS/hosts/firewall máy thật để giả lập. Các ca tối thiểu:

- Candidate đầu DNS fail, candidate sau thành công; lỗi tất cả; single candidate; service map đổi trong lúc pending.
- TLS/proxy failure; HTTP 401/403/404/429/5xx tách riêng; không gọi tất cả là offline.
- Request có thể được server nhận nhưng response mất; echo tới sớm/muộn; retry user liên tiếp; một intent không phát hai send đồng thời.
- Preview cache hit, miss, slow >2s, parselink lỗi, scrape treo, multi-link; deadline chung không bị reset theo host/phase.
- Group guard cache miss/invalidated; account switch/logout khi đang await; không có result chéo account.
- Không phát sinh live mutation trong test tự động; không mở session/DB người dùng trong fixture.

Native/live sau đó: xác định đúng fork executable/hash/version và data root; chạy PC ổn định trước. Đối soát tin test cũ trong Henry trước khi test tiếp vì lần Enter trước chưa rõ kết quả. Phiên này người dùng đã cho phép 2 tin test vào Henry nhưng chưa hoàn tất; tài liệu không tự cấp phép gửi không giới hạn. Đo text và link riêng; ghi thời điểm user-send, request-start, server-confirm/echo, UI-final. Không dùng thời gian tool screenshot làm RTT mạng. Runtime config PC, active host và traffic phải quan sát trực tiếp có redaction; không suy từ IP TCP hoặc domain constant.

Chạy check/test/lint phù hợp standards; bắt buộc `powershell scripts/audit-prepush.ps1` hoặc cargo check --locked trước commit/push theo AGENTS; fmt/test/clippy cho code liên quan. Kiểm tra compilation không chứng minh interoperability. Báo baseline và kết quả mới riêng.

## 5. Những điều vẫn chưa xác minh

1. Active host, candidate list, feature flags và retry_config **effective của phiên PC hiện tại**.
2. DNS error có kích hoạt rotation ở native networking/nhánh khác của PC không; nhánh rõ đã đọc chỉ là HTTP 4xx/5xx.
3. RTT/latency PC và fork cùng điều kiện; kết quả gửi tin test text, test link chưa thực hiện.
4. Host thực tế và error chain của những lần fork người dùng báo lỗi; DNS N01 là ứng viên mạnh, chưa kết luận toàn bộ root cause.
5. Chính sách dedup server khi cùng clientId được gửi lại qua host khác; cần evidence, không giả định exactly-once.
6. Root cause crash PC; Windows có events không chứng minh do mạng hay do thao tác automation.

## 6. Mẫu báo cáo Antigravity

Mỗi gói ghi: file/diff, source parity (Pxx/Fxx/Nxx), quyết định/fork deviation, check đã thực chạy, fake/fixture/native/live coverage, latency từng phase, số mutation/duplicates, rủi ro và UNKNOWN còn lại. Không đánh dấu complete từ unit test đơn thuần. Nếu thay protocol contract, bổ sung re-notes + fixture theo AGENTS; không dùng tài liệu này như capture E1.

Trạng thái bàn giao hiện tại: **chỉ tài liệu; chưa sửa mã, cấu hình, DNS, proxy hay timeout; chưa commit/push.**
