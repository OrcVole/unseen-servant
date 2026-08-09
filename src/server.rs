//! The Gemini listener: accept loop, per-connection tasks, timeouts,
//! connection cap, graceful drain (ADR 0002).
//!
//! Connection lifecycle, in order, each phase under its own deadline:
//!
//! 1. **Accept** — gated on a semaphore permit ([`crate::config::Config::max_connections`]);
//!    the permit rides with the connection task, so the drain logic can
//!    count live connections by counting permits.
//! 2. **TLS handshake** — failure (plaintext probes, version mismatch) is a
//!    silent drop: there is no TLS channel to answer on.
//! 3. **Read request line** — at most [`MAX_REQUEST_BYTES`] bytes; framing
//!    (layer 1) then URI validation (layer 2) then the authority check
//!    (layer 3).
//! 4. **Respond** — via the C1 hello handler (the real handler trait
//!    arrives in C2), or the mapped rejection status.
//! 5. **close_notify, always** — every path that completed the handshake
//!    ends with a TLS shutdown; TOFU clients treat its absence as
//!    truncation (diagnostics check 6, the one twins fails).
//!
//! One deliberate deviation, documented per recon §6: a bare-LF request
//! line is answered by **closing without a response**, not with status 59.
//! The spec never authorizes accepting bare LF; closing satisfies both the
//! strict reading and gemini-diagnostics' RequestMissingCR expectation
//! (which fails a server that answers).

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::time::{Duration, timeout};
use tokio_rustls::TlsAcceptor;

use crate::config::Config;
use crate::protocol::request::{FramingError, MAX_REQUEST_BYTES, frame_request_line};
use crate::protocol::response::{Header, Status, stock};
use crate::protocol::uri::validate_uri;
use crate::protocol::{GEMINI_DEFAULT_PORT, check_authority};

/// Everything a connection needs, swapped atomically on SIGHUP reload.
#[derive(Clone)]
pub struct Shared {
    /// The validated configuration this state was built from.
    pub config: Arc<Config>,
    /// The rustls configuration (SNI resolver, version policy) to accept
    /// connections with.
    pub tls: Arc<rustls::ServerConfig>,
}

/// Bind one listener socket with explicit options: `SO_REUSEADDR`, and
/// `IPV6_V6ONLY` on IPv6 addresses so the default `["0.0.0.0:1965",
/// "[::]:1965"]` pair coexists deterministically on every OS instead of
/// depending on the host's `bindv6only` sysctl.
pub fn bind(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let domain = match addr {
        SocketAddr::V4(_) => socket2::Domain::IPV4,
        SocketAddr::V6(_) => socket2::Domain::IPV6,
    };
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    if addr.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    TcpListener::from_std(socket.into())
}

/// Run the accept loop on `listener` until `shutdown` flips to `true`.
/// Returns once the loop has stopped accepting (drain is the caller's job,
/// via the shared semaphore).
pub async fn accept_loop(
    listener: TcpListener,
    mut state_rx: watch::Receiver<Shared>,
    permits: Arc<Semaphore>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        // A closed semaphore means shutdown was requested while we waited.
        let permit = tokio::select! {
            p = permits.clone().acquire_owned() => match p {
                Ok(p) => p,
                Err(_) => return,
            },
            _ = shutdown.changed() => return,
        };
        let (tcp, peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    // Transient accept errors (EMFILE under pressure, RSTs)
                    // must not kill the listener.
                    tracing::warn!(error = %e, "accept failed");
                    continue;
                }
            },
            _ = shutdown.changed() => return,
        };
        let state = state_rx.borrow_and_update().clone();
        tokio::spawn(async move {
            handle_connection(tcp, peer, state).await;
            drop(permit);
        });
    }
}

/// Serve one connection start to finish. Never panics; never leaves a
/// completed handshake without a close_notify attempt.
async fn handle_connection(tcp: TcpStream, peer: SocketAddr, state: Shared) {
    let request_deadline = Duration::from_secs(state.config.request_timeout_secs);
    let response_deadline = Duration::from_secs(state.config.response_timeout_secs);

    let acceptor = TlsAcceptor::from(state.tls.clone());
    let mut stream = match timeout(request_deadline, acceptor.accept(tcp)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            // Plaintext probe, version mismatch, or handshake garbage: no
            // TLS channel exists to answer on. Dropping IS the answer
            // (diagnostics TLSRequired).
            tracing::debug!(%peer, error = %e, "TLS handshake failed");
            return;
        }
        Err(_) => {
            tracing::debug!(%peer, "TLS handshake timed out");
            return;
        }
    };

    let outcome = match timeout(request_deadline, read_request(&mut stream)).await {
        Ok(Ok(buf)) => respond(&buf, &state),
        Ok(Err(e)) => {
            tracing::debug!(%peer, error = %e, "request read failed");
            Outcome::CloseSilently
        }
        Err(_) => {
            tracing::debug!(%peer, "request read timed out (slow client)");
            Outcome::CloseSilently
        }
    };

    let write_fut = async {
        match &outcome {
            Outcome::Respond { header, body, log } => {
                stream.write_all(&header.to_wire()).await?;
                if let Some(body) = body {
                    stream.write_all(body).await?;
                }
                tracing::info!(%peer, status = header.status() as u8, "{log}");
            }
            Outcome::CloseSilently => {}
        }
        // close_notify on every path that reached this point.
        stream.shutdown().await
    };
    match timeout(response_deadline, write_fut).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::debug!(%peer, error = %e, "response write failed"),
        Err(_) => tracing::debug!(%peer, "response write timed out"),
    }
}

/// What the connection should do after reading (or failing to read) a
/// request line.
enum Outcome {
    /// Write this header (and body), log the given line, close_notify.
    Respond {
        header: Header,
        body: Option<Vec<u8>>,
        /// Pre-redacted log message: never contains the query (recon §8 —
        /// input lands in queries; queries are sensitive by default).
        log: String,
    },
    /// Close with close_notify but no response bytes (bare-LF policy,
    /// unreadable client).
    CloseSilently,
}

/// Read until a CRLF is inside the buffer, the [`MAX_REQUEST_BYTES`] budget
/// is exhausted, or EOF. The framing layer judges whatever this returns —
/// including short or empty buffers.
async fn read_request(
    stream: &mut tokio_rustls::server::TlsStream<TcpStream>,
) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; MAX_REQUEST_BYTES];
    let mut filled = 0;
    loop {
        let n = stream.read(&mut buf[filled..]).await?;
        if n == 0 {
            break; // EOF — judge what we have.
        }
        filled += n;
        if buf[..filled].windows(2).any(|w| w == b"\r\n") || filled == buf.len() {
            break;
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

/// Layers 1–3 plus the C1 hello handler, mapped to a wire outcome.
fn respond(raw: &[u8], state: &Shared) -> Outcome {
    let uri = match frame_request_line(raw) {
        Ok(uri) => uri,
        Err(FramingError::BareLf) => {
            // Documented deviation: see module docs. Strict CRLF or nothing.
            return Outcome::CloseSilently;
        }
        Err(e) => {
            return Outcome::Respond {
                header: stock::bad_request(&e),
                body: None,
                log: format!("rejected at framing: {e}"),
            };
        }
    };
    let target = match validate_uri(uri) {
        Ok(t) => t,
        Err(e) => {
            return Outcome::Respond {
                header: stock::bad_request(&e),
                body: None,
                log: format!("rejected at URI validation: {e}"),
            };
        }
    };
    let config = &state.config;
    let request = match check_authority(target, |h| config.serves_host(h), config.advertised_port) {
        Ok(r) => r,
        Err(crate::protocol::ForeignAuthority) => {
            return Outcome::Respond {
                header: stock::proxy_refused(),
                body: None,
                log: "refused non-local authority (53)".to_string(),
            };
        }
    };

    // ---- C1 hello handler. Replaced by the Handler trait in C2. ----
    // Empty path and "/" are the same resource, served without redirecting
    // (spec MUST; diagnostics HomepageNoRedirect).
    if request.path.is_empty() || request.path == "/" {
        let body = format!(
            "# Unseen Servant\n\nThis capsule is served by usv {} (phase C1: wire core).\n\
             Content arrives in phase C2.\n",
            env!("CARGO_PKG_VERSION")
        );
        match Header::new(Status::Success, Some("text/gemini; charset=utf-8")) {
            Ok(header) => Outcome::Respond {
                header,
                body: Some(body.into_bytes()),
                log: format!("served hello page for {}", request.host),
            },
            Err(e) => {
                // Unreachable by construction; if it ever happens, fail
                // closed rather than panic.
                tracing::error!(error = %e, "hello header construction failed");
                Outcome::Respond {
                    header: stock::unavailable(),
                    body: None,
                    log: "internal header error (41)".to_string(),
                }
            }
        }
    } else {
        Outcome::Respond {
            header: stock::not_found(),
            body: None,
            log: format!("51 for path ({} bytes)", request.path.len()),
        }
    }
}

/// Advertised-port sanity warning at startup: a Gemini capsule off :1965 is
/// reachable only through explicit-port URLs, which breaks discovery
/// (docs/recon/cloudron-fit.md §1).
pub fn warn_if_nonstandard(config: &Config) {
    if config.advertised_port != GEMINI_DEFAULT_PORT {
        tracing::warn!(
            advertised_port = config.advertised_port,
            "Gemini's default port is 1965; clients will only find this capsule through \
             explicit gemini://host:{}/ URLs",
            config.advertised_port
        );
    }
}
