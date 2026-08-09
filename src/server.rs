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
//! 4. **Respond** — dispatch order per matched host (C2): redirects, then
//!    certificate zones, then static file serving; or the mapped
//!    rejection status from an earlier layer.
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

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::time::{Duration, timeout};
use tokio_rustls::TlsAcceptor;

use crate::config::Config;
use crate::handler::{Body, ClientCertInfo, cert_zone, redirect, static_file};
use crate::protocol::request::{FramingError, MAX_REQUEST_BYTES, frame_request_line};
use crate::protocol::response::{Header, stock};
use crate::protocol::titan;
use crate::protocol::uri::validate_uri;
use crate::protocol::{GEMINI_DEFAULT_PORT, authority_is_ours, check_authority};

/// Everything a connection needs, swapped atomically on SIGHUP reload.
#[derive(Clone)]
pub struct Shared {
    /// The validated configuration this state was built from.
    pub config: Arc<Config>,
    /// The rustls configuration (SNI resolver, version policy) to accept
    /// connections with.
    pub tls: Arc<rustls::ServerConfig>,
}

/// A TLS record's first byte is its content type; a ClientHello always
/// starts a record of type 22 (handshake). Anything else on the wire is not
/// TLS at all.
const TLS_HANDSHAKE_RECORD_TYPE: u8 = 22;

/// Peek (without consuming) the connection's first byte to decide whether it
/// looks like the start of a TLS handshake, without ever invoking the TLS
/// acceptor on data that plainly isn't. A short grace period covers clients
/// that are simply slow to write their ClientHello: if nothing has arrived
/// yet, we assume TLS and let the normal handshake path apply its own
/// request timeout.
async fn peek_looks_like_tls(tcp: &TcpStream) -> bool {
    let mut buf = [0u8; 1];
    match timeout(Duration::from_millis(200), tcp.peek(&mut buf)).await {
        Ok(Ok(n)) if n > 0 => buf[0] == TLS_HANDSHAKE_RECORD_TYPE,
        _ => true,
    }
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

    if !peek_looks_like_tls(&tcp).await {
        // A plaintext probe (diagnostics TLSRequired; port scanners): drop
        // outright, before the TLS acceptor ever runs. Letting rustls
        // attempt the handshake would have it write a TLS alert record to
        // the socket before failing — a real response, just not a Gemini
        // one, which is worse than silence: it confirms a TLS stack lives
        // on this port without ever getting to refuse the request properly.
        tracing::debug!(%peer, "non-TLS data on Gemini port; dropping without response");
        return;
    }

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

    let client_cert = extract_client_cert(&stream);

    let outcome = match timeout(request_deadline, read_request(&mut stream)).await {
        Ok(Ok(buf)) => respond(&buf, &state, client_cert.as_ref()).await,
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
        match outcome {
            Outcome::Respond {
                header,
                mut body,
                log,
                drain_bytes,
            } => {
                stream.write_all(&header.to_wire()).await?;
                match &mut body {
                    Body::None => {}
                    Body::Bytes(b) => stream.write_all(b).await?,
                    Body::File(f) => {
                        tokio::io::copy(f, &mut stream).await?;
                    }
                }
                // Single-line, query-redacted by construction: `log` is
                // built from the path only, never the query (recon §8 —
                // input status 10/11 lands in queries, treated as
                // sensitive by default).
                tracing::info!(%peer, status = header.status() as u8, "{log}");
                if drain_bytes > 0 {
                    // Bounded in bytes *and* in time — see the constants.
                    // A drain that times out is not an error: the client
                    // simply had nothing more to send.
                    let _ =
                        timeout(TITAN_DRAIN_TIMEOUT, drain_bounded(&mut stream, drain_bytes)).await;
                }
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
        body: Body,
        /// Pre-redacted log message: never contains the query (recon §8 —
        /// input lands in queries; queries are sensitive by default), and
        /// for Titan never the token (recon titan.md §5.2).
        log: String,
        /// Bytes of unread client payload to absorb *after* writing the
        /// response and before closing. Always 0 on the Gemini path (a
        /// Gemini request has no body); non-zero only when refusing a
        /// Titan upload whose payload may already be in flight. See
        /// [`drain_bounded`].
        drain_bytes: u64,
    },
    /// Close with close_notify but no response bytes (bare-LF policy,
    /// unreadable client).
    CloseSilently,
}

impl Outcome {
    /// The ordinary case: respond and close, nothing to drain.
    fn respond(header: Header, body: Body, log: impl Into<String>) -> Outcome {
        Outcome::Respond {
            header,
            body,
            log: log.into(),
            drain_bytes: 0,
        }
    }

    /// Refuse a Titan upload, absorbing up to `drain_bytes` of in-flight
    /// payload afterwards so the client can finish writing and still read
    /// the status (recon titan.md §5.5).
    fn refuse_upload(header: Header, log: impl Into<String>, drain_bytes: u64) -> Outcome {
        Outcome::Respond {
            header,
            body: Body::None,
            log: log.into(),
            drain_bytes,
        }
    }
}

/// Compute the requesting client's certificate identity, if one was
/// presented. Fingerprint is SHA-256 of the leaf certificate's DER bytes
/// (the identity Molly Brown's model and Gemini client-cert culture both
/// key on); validity comes from the certificate's own notBefore/notAfter,
/// independent of any CA chain (TLS layer already accepted the cert
/// unconditionally — see `tls.rs` — so this is the *only* place validity
/// is judged, and it maps directly to status 62).
fn extract_client_cert(
    stream: &tokio_rustls::server::TlsStream<TcpStream>,
) -> Option<ClientCertInfo> {
    let chain = stream.get_ref().1.peer_certificates()?;
    let leaf = chain.first()?;
    let fingerprint_sha256 = Sha256::digest(leaf.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let currently_valid = x509_parser::parse_x509_certificate(leaf.as_ref())
        .map(|(_, cert)| cert.validity().is_valid())
        .unwrap_or(false); // unparseable certificate: never valid, maps to 62
    Some(ClientCertInfo {
        fingerprint_sha256,
        currently_valid,
    })
}

/// The most in-flight Titan payload usv will absorb after refusing an
/// upload: 64 KiB (recon titan.md §5.5). Enough that a small write lands
/// and the client gets to read the status; bounded so a refused upload can
/// never be used to make the server read an attacker's whole payload.
const TITAN_DRAIN_LIMIT: u64 = 64 * 1024;

/// How long usv will wait while draining a refused upload.
///
/// The byte cap alone is not enough: a client may *declare* a size and
/// then send nothing, which would park the connection in the drain until
/// the response deadline — a slot-holding trick, and exactly the shape of
/// the slowloris behaviour the request path already guards against. The
/// drain is a courtesy to a client that is genuinely mid-write, so it gets
/// a short window and no more; whatever has not arrived by then was never
/// coming.
const TITAN_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// How much to absorb when refusing an upload of `declared_size` bytes:
/// never more than the client said it would send, never more than the cap.
fn drain_for(declared_size: u64) -> u64 {
    declared_size.min(TITAN_DRAIN_LIMIT)
}

/// Absorb and discard up to `limit` bytes of payload a rejected Titan
/// client may already be streaming.
///
/// The problem this solves (recon titan.md §5.5, flagged there as the top
/// interop risk): Titan lets a server refuse *before* the payload arrives,
/// but a client that has already begun writing sees the connection close
/// under it and reports a broken pipe instead of reading our status line.
/// Reading its bytes into the void lets the write complete, so the client
/// proceeds to read and reports the real reason it was refused.
///
/// Best-effort by design: every error and EOF simply ends the drain. There
/// is nothing to report — the response has already been written, and this
/// is politeness toward the peer, not part of the transaction.
///
/// Bounded in bytes here and in *time* by the caller
/// ([`TITAN_DRAIN_TIMEOUT`]); both bounds are required, since a client can
/// declare a size and then send nothing at all.
async fn drain_bounded(stream: &mut tokio_rustls::server::TlsStream<TcpStream>, limit: u64) {
    let mut scratch = [0u8; 8192];
    let mut remaining = limit;
    while remaining > 0 {
        let want = usize::try_from(remaining)
            .unwrap_or(scratch.len())
            .min(scratch.len());
        match stream.read(&mut scratch[..want]).await {
            Ok(0) | Err(_) => return,
            Ok(n) => remaining -= n as u64,
        }
    }
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

/// Layers 1–3 plus C2 host dispatch, mapped to a wire outcome.
async fn respond(raw: &[u8], state: &Shared, client_cert: Option<&ClientCertInfo>) -> Outcome {
    let uri = match frame_request_line(raw) {
        Ok(uri) => uri,
        Err(FramingError::BareLf) => {
            // Documented deviation: see module docs. Strict CRLF or nothing.
            return Outcome::CloseSilently;
        }
        Err(e) => {
            return Outcome::respond(
                stock::bad_request(&e),
                Body::None,
                format!("rejected at framing: {e}"),
            );
        }
    };

    // Same-listener scheme dispatch (ADR 0006; recon titan.md §5.4). Titan
    // shares the Gemini listener and its TLS — including the client
    // certificate, already extracted at handshake — and is distinguished
    // only by scheme. Everything after this point on the Gemini branch is
    // unchanged from C2/C3.
    if crate::protocol::is_titan_request(uri) {
        return respond_titan(uri, state, client_cert);
    }

    let target = match validate_uri(uri) {
        Ok(t) => t,
        Err(e) => {
            return Outcome::respond(
                stock::bad_request(&e),
                Body::None,
                format!("rejected at URI validation: {e}"),
            );
        }
    };
    let config = &state.config;
    let request = match check_authority(target, |h| config.serves_host(h), config.advertised_port) {
        Ok(r) => r,
        Err(crate::protocol::ForeignAuthority) => {
            return Outcome::respond(
                stock::proxy_refused(),
                Body::None,
                "refused non-local authority (53)",
            );
        }
    };

    // check_authority proved `request.host` matches a configured host
    // case-insensitively; find_host is infallible here by construction.
    let Some(host) = config.find_host(&request.host) else {
        // Unreachable given check_authority's own logic; fail closed.
        return Outcome::respond(
            stock::unavailable(),
            Body::None,
            "internal: authority check passed but host lookup failed",
        );
    };

    // Dispatch order (C2, this module's docs): redirects, then cert zones,
    // then static file serving.
    if let Some(resp) = redirect::try_match(&host.redirects, &request.path) {
        let log = format!("{} → redirect", request.path.len());
        return Outcome::respond(resp.header, resp.body, log);
    }
    if let Some(resp) = cert_zone::check(&host.cert_zones, &request.path, client_cert) {
        let status = resp.header.status() as u8;
        return Outcome::respond(resp.header, resp.body, format!("cert zone gate ({status})"));
    }
    let resp = static_file::serve(&host.docroot, &request.path, &config.lang).await;
    let status = resp.header.status() as u8;
    Outcome::respond(
        resp.header,
        resp.body,
        format!("{} for {} ({status})", request.host, request.path),
    )
}

/// The Titan branch of layer 2/3: parse the upload request line, check the
/// authority, and decide. No payload is read here — every path below
/// refuses *before* the body, which is the whole point of doing the auth
/// and size decisions at the request line (recon titan.md §5.4).
///
/// Writable zones, the certificate gate, and size caps arrive with the
/// `[titan]` configuration section; until a zone exists there is nothing an
/// upload could legitimately land in, so the honest answer is a flat
/// refusal. Each refusal drains a bounded amount of in-flight payload so
/// the client actually sees it (recon §5.5).
///
/// Never logs the token: it is a shared secret riding in a URL (recon
/// §5.2), and it must not reach the log the way a query never does.
fn respond_titan(uri: &[u8], state: &Shared, _client_cert: Option<&ClientCertInfo>) -> Outcome {
    let request = match titan::parse(uri) {
        Ok(r) => r,
        Err(e) => {
            // The declared size is unknown (that is what failed to parse),
            // so drain the standard bound rather than nothing: a client
            // that already began streaming still gets to read the 59.
            return Outcome::refuse_upload(
                stock::bad_request(&e),
                format!("titan rejected at parse: {e}"),
                TITAN_DRAIN_LIMIT,
            );
        }
    };

    let config = &state.config;
    if !authority_is_ours(
        &request.host,
        request.port,
        |h| config.serves_host(h),
        config.advertised_port,
    ) {
        return Outcome::refuse_upload(
            stock::proxy_refused(),
            "titan refused non-local authority (53)",
            drain_for(request.size),
        );
    }

    Outcome::refuse_upload(
        stock::uploads_not_accepted(),
        format!(
            "titan upload refused, no writable zone: {} bytes declared for {} (50)",
            request.size, request.path
        ),
        drain_for(request.size),
    )
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
