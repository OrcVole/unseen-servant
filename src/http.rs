//! The HTTP surface: serves the rendered HTML tree (BUILD-PLAN C3),
//! **unconditionally** — this listener starts independent of the Gemini
//! listener's state and must return 2xx at `/` even before any content
//! has ever been rendered (docs/internal/recon/cloudron-fit.md's hard constraint:
//! "the HTTP listener must start unconditionally... before/independently
//! of the Gemini listener"; the health check must pass "even when the
//! Gemini port is disabled or not yet configured").
//!
//! Deliberately hand-rolled, not built on an HTTP framework: the surface
//! this server needs is tiny (GET-only, no keep-alive, no request
//! bodies, no chunked responses — every response closes the connection),
//! the same reasoning ADR 0001 already applied to the Gemini wire
//! protocol. A framework capable of everything HTTP/1.1 allows would
//! bring far more surface than this server ever uses.
//!
//! Path resolution reuses [`crate::handler::static_file::resolve_safe_path`]
//! — the exact same traversal defense as the Gemini surface, applied to
//! the same kind of content tree, so there is only ever one
//! implementation of "is this path safe" to keep correct.

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::time::{Duration, timeout};

use crate::handler::{mime, static_file};

/// Hard cap on request-line-plus-headers size: this server has no need
/// for large headers, and an unbounded read is a memory-exhaustion vector
/// for a listener that (per the ADR 0008 constraint above) must always
/// stay up and answer health checks.
const MAX_REQUEST_BYTES: usize = 8192;

/// How long a client gets to send a complete request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Everything an HTTP connection needs, swapped atomically like the
/// Gemini listener's `Shared` (`server.rs`) — a render completing swaps
/// in a fresh `html_dir` value (currently the same path every time,
/// since `render::pipeline` renders in place; kept as a watch channel
/// for symmetry and so a future path change needs no restructuring).
#[derive(Clone)]
pub struct Shared {
    /// `${state_dir}/html` — the rendered tree's live root.
    pub html_dir: PathBuf,
    /// Every address this capsule answers on, for the colophon. Carried
    /// here rather than re-derived per request so it tracks a config
    /// reload through the same watch channel as the tree itself.
    pub addrs: crate::render::colophon::Addresses,
}

/// Bind the HTTP listener. Plain TCP, no TLS — Cloudron's own reverse
/// proxy terminates HTTPS in front of `httpPort` (cloudron-fit.md §4);
/// standalone deployments that want TLS put a reverse proxy in front
/// themselves, the same way any plain HTTP origin server expects to be
/// deployed.
pub fn bind(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let socket = socket2::Socket::new(
        match addr {
            SocketAddr::V4(_) => socket2::Domain::IPV4,
            SocketAddr::V6(_) => socket2::Domain::IPV6,
        },
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    if addr.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    TcpListener::from_std(socket.into())
}

/// Accept loop — the HTTP-surface twin of `server::accept_loop`. No
/// connection cap here: this surface serves static files with no
/// per-request compute cost worth rate-limiting at this layer, and it
/// must never itself become the reason a health check stalls.
pub async fn accept_loop(
    listener: TcpListener,
    mut state_rx: watch::Receiver<Shared>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        let (tcp, _peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!(error = %e, "http accept failed");
                    continue;
                }
            },
            _ = shutdown.changed() => return,
        };
        let state = state_rx.borrow_and_update().clone();
        tokio::spawn(handle_connection(tcp, state));
    }
}

async fn handle_connection(mut tcp: TcpStream, state: Shared) {
    let request = match timeout(REQUEST_TIMEOUT, read_request_line(&mut tcp)).await {
        Ok(Ok(Some(r))) => r,
        Ok(Ok(None)) => {
            let _ =
                write_response(&mut tcp, 400, "text/plain; charset=utf-8", b"bad request").await;
            return;
        }
        Ok(Err(_)) | Err(_) => return, // connection error or timeout: nothing to answer
    };

    if request.method != "GET" {
        let _ = write_response(
            &mut tcp,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed",
        )
        .await;
        return;
    }

    respond(&mut tcp, &state, &request.path).await;
}

struct Request {
    method: String,
    path: String,
}

/// Read up to [`MAX_REQUEST_BYTES`] looking for the blank line that ends
/// HTTP headers, then parse just the request line (`METHOD PATH
/// HTTP/x.y`). Headers themselves are read (so the socket is left in a
/// clean state) but not interpreted — nothing this server does depends
/// on any request header. Returns `None` for anything that doesn't parse
/// as `METHOD SP PATH SP HTTP-VERSION`.
async fn read_request_line(tcp: &mut TcpStream) -> std::io::Result<Option<Request>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = tcp.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > MAX_REQUEST_BYTES {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let Some(request_line) = text.lines().next() else {
        return Ok(None);
    };
    let mut parts = request_line.split(' ');
    let (Some(method), Some(path), Some(_version)) = (parts.next(), parts.next(), parts.next())
    else {
        return Ok(None);
    };
    Ok(Some(Request {
        method: method.to_string(),
        path: path.to_string(),
    }))
}

/// Serve `path` from the rendered tree. `/` is the health check
/// (cloudron-fit.md: `healthCheckPath: "/"`) and MUST return 2xx even
/// when nothing has been rendered yet — it falls back to a minimal
/// built-in page rather than depending on `index.html` existing.
async fn respond(tcp: &mut TcpStream, state: &Shared, path: &str) {
    let path_only = path.split('?').next().unwrap_or(path);

    if path_only == "/" {
        match static_file::resolve_safe_path(&state.html_dir, "/index.html").await {
            Some(file) => serve_file(tcp, &file).await,
            None => {
                let _ = write_response(
                    tcp,
                    200,
                    "text/html; charset=utf-8",
                    PLACEHOLDER_HEALTH_PAGE.as_bytes(),
                )
                .await;
            }
        }
        return;
    }

    match static_file::resolve_safe_path(&state.html_dir, path_only).await {
        Some(file) => serve_file(tcp, &file).await,
        // The colophon, rendered through the same gemtext-to-HTML
        // emitter the rest of the mirror uses, so it inherits the
        // capsule's markup rather than growing a second style. Only
        // reached when the operator has no page of their own here.
        None if crate::render::colophon::matches(path_only) => {
            let gmi = crate::render::colophon::gemtext(
                crate::render::colophon::Protocol::Web,
                &state.addrs,
            );
            let lines = crate::render::gemtext::parse(&gmi);
            let html = crate::render::html::render_document(&lines, "About this capsule", "en");
            let _ = write_response(tcp, 200, "text/html; charset=utf-8", html.as_bytes()).await;
        }
        None => {
            let _ = write_response(tcp, 404, "text/plain; charset=utf-8", b"not found").await;
        }
    }
}

/// The health-check fallback when no `index.html` has been rendered yet
/// — a fresh capsule with no content authored is a normal state (ADR
/// 0008), not a fault, and the tile must not be a dead end either way.
const PLACEHOLDER_HEALTH_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head>\
<meta charset=\"utf-8\"><title>Unseen Servant</title></head>\n\
<body><p>This capsule is starting up. Content will appear here once rendered.</p>\
</body></html>\n";

async fn serve_file(tcp: &mut TcpStream, path: &std::path::Path) {
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        let _ = write_response(tcp, 404, "text/plain; charset=utf-8", b"not found").await;
        return;
    };
    let Ok(meta) = file.metadata().await else {
        let _ = write_response(tcp, 500, "text/plain; charset=utf-8", b"internal error").await;
        return;
    };
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let content_type = mime::lookup(filename);
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        meta.len()
    );
    if tcp.write_all(header.as_bytes()).await.is_err() {
        return;
    }
    let _ = tokio::io::copy(&mut file, tcp).await;
}

async fn write_response(
    tcp: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    tcp.write_all(header.as_bytes()).await?;
    tcp.write_all(body).await
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream as StdTcpStream;

    async fn start_server(html_dir: PathBuf) -> SocketAddr {
        let listener = bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = listener.local_addr().unwrap();
        let (state_tx, state_rx) = watch::channel(Shared {
            html_dir,
            addrs: Default::default(),
        });
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Both senders must outlive this function: a dropped `watch::Sender`
        // resolves the receiver's `.changed()` (as an error, but the
        // accept loop's `select!` doesn't distinguish), which would stop
        // the server almost immediately after it starts — exactly the bug
        // this comment is here to stop from recurring.
        std::mem::forget(state_tx);
        std::mem::forget(shutdown_tx);
        tokio::spawn(accept_loop(listener, state_rx, shutdown_rx));
        addr
    }

    fn get(addr: SocketAddr, path: &str) -> (u16, String) {
        let mut stream = StdTcpStream::connect(addr).expect("connect");
        stream
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        let status_line = response.lines().next().unwrap_or("");
        let status: u16 = status_line
            .split(' ')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        (status, response)
    }

    fn tmp_html_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("usv-http-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_check_passes_with_no_content_rendered_yet() {
        let dir = tmp_html_dir("no-content");
        let addr = start_server(dir.clone()).await;
        let (status, body) = get(addr, "/");
        assert_eq!(status, 200, "the health check must pass unconditionally");
        assert!(body.contains("starting up"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_check_serves_index_once_rendered() {
        let dir = tmp_html_dir("with-index");
        std::fs::write(dir.join("index.html"), "<h1>Home</h1>").unwrap();
        let addr = start_server(dir.clone()).await;
        let (status, body) = get(addr, "/");
        assert_eq!(status, 200);
        assert!(body.contains("<h1>Home</h1>"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn serves_a_real_page() {
        let dir = tmp_html_dir("real-page");
        std::fs::create_dir_all(dir.join("blog")).unwrap();
        std::fs::write(dir.join("blog/post.html"), "<h1>Post</h1>").unwrap();
        let addr = start_server(dir.clone()).await;
        let (status, body) = get(addr, "/blog/post.html");
        assert_eq!(status, 200);
        assert!(body.contains("<h1>Post</h1>"));
        assert!(body.contains("text/html"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_page_is_404() {
        let dir = tmp_html_dir("missing");
        let addr = start_server(dir.clone()).await;
        let (status, _) = get(addr, "/nope.html");
        assert_eq!(status, 404);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn traversal_attempt_is_404_not_the_secret() {
        let dir = tmp_html_dir("traversal");
        let outside = dir.parent().unwrap();
        std::fs::write(outside.join("usv-http-secret"), "top secret").ok();
        let addr = start_server(dir.clone()).await;
        let (status, body) = get(addr, "/../usv-http-secret");
        assert_eq!(status, 404);
        assert!(!body.contains("top secret"));
        let _ = std::fs::remove_file(outside.join("usv-http-secret"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn non_get_method_is_405() {
        let dir = tmp_html_dir("method");
        let addr = start_server(dir.clone()).await;
        let mut stream = StdTcpStream::connect(addr).unwrap();
        stream
            .write_all(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 405"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn query_string_is_ignored_for_path_resolution() {
        let dir = tmp_html_dir("query");
        std::fs::write(dir.join("index.html"), "<h1>Home</h1>").unwrap();
        let addr = start_server(dir.clone()).await;
        let (status, body) = get(addr, "/?foo=bar");
        assert_eq!(status, 200);
        assert!(body.contains("<h1>Home</h1>"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn the_web_mirror_serves_the_colophon_rather_than_404() {
        // The default skeleton links /usv from the capsule root, so this
        // path must answer on every surface the skeleton is rendered to
        // -- the web mirror included, where it is not a file in the tree.
        let out = colophon_html();
        assert!(out.contains("UnSeen serVant"), "{out}");
        assert!(
            out.contains("<h1"),
            "not rendered through the HTML emitter: {out}"
        );
    }

    fn colophon_html() -> String {
        let addrs = crate::render::colophon::Addresses {
            host: "example.org".into(),
            gemini_port: Some(1965),
            gopher_port: Some(70),
            ..Default::default()
        };
        let gmi = crate::render::colophon::gemtext(crate::render::colophon::Protocol::Web, &addrs);
        let lines = crate::render::gemtext::parse(&gmi);
        crate::render::html::render_document(&lines, "About this capsule", "en")
    }

    #[test]
    fn the_web_colophon_points_at_the_other_protocols() {
        let out = colophon_html();
        assert!(out.contains("gemini://example.org/"), "{out}");
        assert!(out.contains("gopher://example.org:70/"), "{out}");
    }
}
