//! Optional outbound proxy for Zalo HTTP and WebSocket traffic.
//!
//! Configured once at startup from `--proxy URL` or `CTDOZ_PROXY`; never persisted, so proxy
//! credentials never reach `session.json`. Call media (UDP RTP) cannot use a TCP proxy.

use std::sync::OnceLock;

use anyhow::{Context, Result};
use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

static PROXY: OnceLock<Option<ProxyConfig>> = OnceLock::new();

/// Largest proxy `CONNECT` response header accepted before the tunnel starts.
const MAX_CONNECT_RESPONSE: usize = 8 * 1024;

/// Validated proxy endpoint.
#[derive(Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    url: reqwest::Url,
}

impl std::fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.redacted())
    }
}

impl ProxyConfig {
    /// Parse `http://`, `socks5://` or `socks5h://` proxy URLs; a bare `host:port` means HTTP.
    ///
    /// # Errors
    /// Rejects unsupported schemes, missing host or port, and URLs with a path or query.
    /// Error text never contains the credentials.
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        anyhow::ensure!(!raw.is_empty(), "Địa chỉ proxy đang trống");
        let with_scheme = if raw.contains("://") {
            raw.to_owned()
        } else {
            format!("http://{raw}")
        };
        let url = reqwest::Url::parse(&with_scheme)
            .map_err(|_| anyhow::anyhow!("Địa chỉ proxy không hợp lệ"))?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "socks5" | "socks5h"),
            "Chỉ hỗ trợ proxy http://, socks5:// hoặc socks5h://"
        );
        anyhow::ensure!(
            url.host_str().is_some_and(|h| !h.is_empty()),
            "Địa chỉ proxy thiếu máy chủ"
        );
        anyhow::ensure!(
            url.port_or_known_default().is_some()
                && (url.scheme() == "http" || url.port().is_some()),
            "Địa chỉ proxy thiếu cổng"
        );
        anyhow::ensure!(
            matches!(url.path(), "" | "/") && url.query().is_none() && url.fragment().is_none(),
            "Địa chỉ proxy không được chứa đường dẫn"
        );
        reqwest::Proxy::all(url.as_str())
            .map_err(|_| anyhow::anyhow!("Không tạo được cấu hình proxy"))?;
        Ok(Self { url })
    }

    /// Proxy URL with any username/password replaced by `***`, safe for logs and UI.
    pub fn redacted(&self) -> String {
        let mut url = self.url.clone();
        if !url.username().is_empty() || url.password().is_some() {
            let _ = url.set_username("***");
            let _ = url.set_password(Some("***"));
        }
        url.to_string()
    }

    fn host(&self) -> &str {
        self.url.host_str().unwrap_or_default()
    }

    fn port(&self) -> u16 {
        self.url.port_or_known_default().unwrap_or(1080)
    }

    fn credentials(&self) -> Option<(String, String)> {
        (!self.url.username().is_empty()).then(|| {
            (
                percent_decode(self.url.username()),
                percent_decode(self.url.password().unwrap_or_default()),
            )
        })
    }
}

fn percent_decode(s: &str) -> String {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Read `--proxy URL` / `--proxy=URL`, falling back to the `CTDOZ_PROXY` environment variable.
///
/// # Errors
/// Fails when the flag has no value, is repeated, or the URL is invalid.
pub fn from_args(args: &[String], env: Option<String>) -> Result<Option<ProxyConfig>> {
    let mut found: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        let value = if args[i] == "--proxy" {
            i += 1;
            Some(
                args.get(i)
                    .filter(|v| !v.starts_with('-'))
                    .context("--proxy cần địa chỉ, ví dụ: --proxy socks5://127.0.0.1:1080")?
                    .as_str(),
            )
        } else {
            args[i].strip_prefix("--proxy=")
        };
        if let Some(value) = value {
            anyhow::ensure!(found.is_none(), "--proxy chỉ được dùng một lần");
            found = Some(value);
        }
        i += 1;
    }
    match found
        .map(str::to_owned)
        .or(env.filter(|v| !v.trim().is_empty()))
    {
        Some(raw) => Ok(Some(ProxyConfig::parse(&raw)?)),
        None => Ok(None),
    }
}

/// Install the process-wide proxy; must run before the first [`apply`] or [`connect`].
///
/// # Errors
/// Fails when called twice.
pub fn init(config: Option<ProxyConfig>) -> Result<()> {
    PROXY
        .set(config)
        .map_err(|_| anyhow::anyhow!("Proxy is already initialized"))
}

/// The configured proxy, if any.
pub fn current() -> Option<&'static ProxyConfig> {
    PROXY.get_or_init(|| None).as_ref()
}

/// Route a reqwest client through the configured proxy; without one, behaviour is unchanged.
///
/// # Errors
/// Returns an error when the proxy cannot be applied, so a client never silently bypasses it.
pub fn apply(builder: reqwest::ClientBuilder) -> Result<reqwest::ClientBuilder> {
    apply_config(builder, current())
}

fn apply_config(
    builder: reqwest::ClientBuilder,
    config: Option<&ProxyConfig>,
) -> Result<reqwest::ClientBuilder> {
    match config {
        Some(config) => Ok(builder.proxy(
            reqwest::Proxy::all(config.url.as_str())
                .map_err(|_| anyhow::anyhow!("Không tạo được cấu hình proxy"))?,
        )),
        None => Ok(builder),
    }
}

/// Open a TCP stream to `host:port`, tunnelled through `config`.
///
/// # Errors
/// Returns proxy handshake, authentication or transport failures without credentials.
pub async fn connect_via(config: &ProxyConfig, host: &str, port: u16) -> Result<TcpStream> {
    let proxy_addr = (config.host().trim_matches(['[', ']']), config.port());
    match config.url.scheme() {
        "http" => {
            let mut stream = TcpStream::connect(proxy_addr)
                .await
                .context("Không kết nối được tới proxy")?;
            http_connect(&mut stream, host, port, config.credentials()).await?;
            Ok(stream)
        }
        _ => {
            // socks5 resolves the target locally like reqwest does; socks5h lets the proxy resolve.
            let target: tokio_socks::TargetAddr<'_> = if config.url.scheme() == "socks5" {
                let addr = tokio::net::lookup_host((host, port))
                    .await?
                    .next()
                    .context("Không phân giải được máy chủ đích")?;
                tokio_socks::TargetAddr::Ip(addr)
            } else {
                tokio_socks::TargetAddr::Domain(host.into(), port)
            };
            let stream = match config.credentials() {
                Some((user, pass)) => {
                    tokio_socks::tcp::Socks5Stream::connect_with_password(
                        proxy_addr, target, &user, &pass,
                    )
                    .await
                }
                None => tokio_socks::tcp::Socks5Stream::connect(proxy_addr, target).await,
            }
            .map_err(|e| anyhow::anyhow!("Proxy SOCKS5 từ chối kết nối: {}", socks_reason(&e)))?;
            Ok(stream.into_inner())
        }
    }
}

fn socks_reason(e: &tokio_socks::Error) -> &'static str {
    match e {
        tokio_socks::Error::PasswordAuthFailure(_)
        | tokio_socks::Error::NoAcceptableAuthMethods => "sai thông tin đăng nhập proxy",
        tokio_socks::Error::Io(_) => "lỗi mạng",
        _ => "proxy trả lỗi",
    }
}

async fn http_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    credentials: Option<(String, String)>,
) -> Result<()> {
    anyhow::ensure!(
        !host.is_empty() && !host.contains(['\r', '\n', ' ', '/']),
        "Invalid tunnel host"
    );
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if let Some((user, pass)) = credentials {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        request.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    let mut total = reader.read_line(&mut status).await?;
    let code = status
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .context("Proxy trả phản hồi không hợp lệ")?;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        total += n;
        anyhow::ensure!(total <= MAX_CONNECT_RESPONSE, "Proxy trả phản hồi quá dài");
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }
    // A buffered byte past the header would belong to the tunnel; CONNECT replies carry none.
    anyhow::ensure!(
        reader.buffer().is_empty(),
        "Proxy gửi dữ liệu ngoài dự kiến"
    );
    match code {
        200..=299 => Ok(()),
        407 => anyhow::bail!("Proxy yêu cầu đăng nhập hoặc sai thông tin đăng nhập (407)"),
        code => anyhow::bail!("Proxy từ chối mở kết nối (HTTP {code})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_accepts_supported_schemes_and_defaults_bare_host_to_http() {
        assert_eq!(
            ProxyConfig::parse("127.0.0.1:8080").unwrap().redacted(),
            "http://127.0.0.1:8080/"
        );
        assert!(ProxyConfig::parse("socks5://127.0.0.1:1080").is_ok());
        assert!(ProxyConfig::parse("socks5h://proxy.example:1080").is_ok());
    }

    #[test]
    fn parse_rejects_unsupported_or_incomplete_urls_when_invalid() {
        for bad in [
            "",
            "ftp://h:1",
            "https://h:443",
            "socks5://h",
            "http://h:1/path",
            "http://:1",
        ] {
            assert!(ProxyConfig::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn redaction_hides_credentials_in_text_and_debug_output() {
        let config = ProxyConfig::parse("socks5://user:s3cret@10.0.0.1:1080").unwrap();
        assert_eq!(config.redacted(), "socks5://***:***@10.0.0.1:1080");
        assert!(!format!("{config:?}").contains("s3cret"));
        let error = ProxyConfig::parse("ftp://user:s3cret@h:1")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("s3cret"));
    }

    #[test]
    fn credentials_are_percent_decoded_when_url_encoded() {
        let config = ProxyConfig::parse("http://us%40er:p%3Ass@h:3128").unwrap();
        assert_eq!(config.credentials(), Some(("us@er".into(), "p:ss".into())));
    }

    #[test]
    fn args_take_flag_over_env_and_reject_repeats_when_parsed() {
        assert!(from_args(&args(&["--gui"]), None).unwrap().is_none());
        let env = Some("socks5://127.0.0.1:1080".to_string());
        assert_eq!(
            from_args(&args(&["--proxy", "127.0.0.1:3128"]), env.clone())
                .unwrap()
                .unwrap()
                .redacted(),
            "http://127.0.0.1:3128/"
        );
        assert!(from_args(&args(&[]), env).unwrap().is_some());
        assert!(from_args(&args(&["--proxy"]), None).is_err());
        assert!(from_args(&args(&["--proxy=a:1", "--proxy=b:2"]), None).is_err());
    }

    async fn fake_http_proxy(reply: &'static [u8]) -> (u16, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0; 1024];
            let n = socket.read(&mut buf).await.unwrap();
            socket.write_all(reply).await.unwrap();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        (port, server)
    }

    #[tokio::test]
    async fn http_connect_sends_basic_auth_and_opens_tunnel_when_proxy_accepts() {
        let (port, server) = fake_http_proxy(b"HTTP/1.1 200 Connection established\r\n\r\n").await;
        let config = ProxyConfig::parse(&format!("http://u:p@127.0.0.1:{port}")).unwrap();
        connect_via(&config, "ws.example", 443).await.unwrap();
        let request = server.await.unwrap();
        assert!(request.starts_with("CONNECT ws.example:443 HTTP/1.1\r\n"));
        assert!(request.contains("Proxy-Authorization: Basic dTpw\r\n"));
    }

    #[tokio::test]
    async fn http_client_routes_requests_through_proxy_when_configured() {
        let (port, server) =
            fake_http_proxy(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await;
        let config = ProxyConfig::parse(&format!("http://127.0.0.1:{port}")).unwrap();
        let client = apply_config(reqwest::Client::builder(), Some(&config))
            .unwrap()
            .build()
            .unwrap();
        let body = client
            .get("http://zalo.invalid/ping")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "ok");
        assert!(server
            .await
            .unwrap()
            .starts_with("GET http://zalo.invalid/ping HTTP/1.1"));
    }

    #[tokio::test]
    async fn http_connect_fails_closed_when_proxy_requires_auth() {
        let (port, server) =
            fake_http_proxy(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n").await;
        let config = ProxyConfig::parse(&format!("http://127.0.0.1:{port}")).unwrap();
        let error = connect_via(&config, "ws.example", 443)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("407"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn socks5_connect_requests_domain_target_when_socks5h() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut greeting = [0u8; 3];
            socket.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [5, 1, 0]);
            socket.write_all(&[5, 0]).await.unwrap();
            let mut head = [0u8; 5];
            socket.read_exact(&mut head).await.unwrap();
            assert_eq!(&head[..4], &[5, 1, 0, 3]);
            let mut domain = vec![0u8; head[4] as usize + 2];
            socket.read_exact(&mut domain).await.unwrap();
            socket
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
                .await
                .unwrap();
            String::from_utf8_lossy(&domain[..head[4] as usize]).into_owned()
        });
        let config = ProxyConfig::parse(&format!("socks5h://127.0.0.1:{port}")).unwrap();
        connect_via(&config, "ws.example", 443).await.unwrap();
        assert_eq!(server.await.unwrap(), "ws.example");
    }
}
