//! One listener kind for every cleartext smolnet protocol (ADR 0012 §1).
//!
//! Gopher, Spartan, Nex and Finger are the same shape on the wire: accept,
//! read one line under a cap and a deadline, write a body, close. None
//! keep the connection alive; none negotiate; none authenticate. What
//! differs between them is the grammar of that line and the framing of
//! that body — which is exactly what [`Service`] parameterises.
//!
//! Four bespoke listeners would mean fixing every slowloris-class bug
//! four times, so the machinery here is deliberately the same as
//! [`crate::server`]'s: a semaphore that caps concurrent connections and
//! rides with the task, per-phase deadlines, transient accept errors that
//! warn rather than kill the loop, and a shutdown watch.
//!
//! **These listeners are cleartext.** No confidentiality, no integrity,
//! no server authentication, and — the part that shapes the rest of the
//! design — no way to authenticate a *client* at all. Nothing gated may
//! be served from here; ADR 0012 §6 makes that a startup error rather
//! than a caution, enforced where the render targets are built rather
//! than in this module.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::time::{Duration, timeout};

use crate::config::Config;
use crate::server::PeerLabel;

/// What a protocol does with one request line.
///
/// Returns the complete body to write. Errors are the protocol's own
/// business — gopher has no status codes, so a "not found" is a perfectly
/// ordinary menu — which is why this cannot fail: every outcome is bytes.
///
/// Takes the [`Config`] rather than [`crate::server::Shared`]: a
/// cleartext listener has no business holding a TLS configuration, and
/// not passing one makes that structural rather than a matter of
/// discipline.
pub type Handler = Arc<
    dyn Fn(Vec<u8>, Arc<Config>) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send>>
        + Send
        + Sync
        + 'static,
>;

/// One cleartext protocol, as this listener needs to know it.
#[derive(Clone)]
pub struct Service {
    /// Protocol name, for logs. Appears in the startup disclaimer.
    pub name: &'static str,
    /// Longest request line accepted before the peer is refused.
    pub max_request_bytes: usize,
    /// Seconds the peer gets to deliver a complete line.
    pub request_timeout_secs: u64,
    /// Seconds the whole response write may take.
    pub response_timeout_secs: u64,
    /// Turns a request line into a body.
    pub handler: Handler,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("name", &self.name)
            .field("max_request_bytes", &self.max_request_bytes)
            .finish_non_exhaustive()
    }
}

/// Log the one-line trust disclaimer ADR 0012 §2 requires when a
/// cleartext listener comes up.
///
/// Not a warning to be silenced: a statement of what was just switched
/// on, at the moment it is switched on, naming the protocol so an
/// operator reading logs later can see which door was opened.
pub fn log_trust_disclaimer(name: &str, bound: SocketAddr) {
    tracing::info!(
        protocol = name,
        %bound,
        "cleartext listener enabled — no confidentiality, no integrity, and no client \
         authentication is possible on this protocol; serve only content whose \
         disclosure and alteration are acceptable"
    );
}

/// Accept connections until `shutdown` flips, handing each to `service`.
pub async fn accept_loop(
    listener: TcpListener,
    service: Service,
    mut config_rx: watch::Receiver<Arc<Config>>,
    permits: Arc<Semaphore>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
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
                    tracing::warn!(protocol = service.name, error = %e, "accept failed");
                    continue;
                }
            },
            _ = shutdown.changed() => return,
        };
        let config = config_rx.borrow_and_update().clone();
        let service = service.clone();
        tokio::spawn(async move {
            handle_connection(tcp, peer, service, config).await;
            drop(permit);
        });
    }
}

/// Serve one cleartext connection start to finish. Never panics.
async fn handle_connection(
    mut tcp: TcpStream,
    peer: SocketAddr,
    service: Service,
    config: Arc<Config>,
) {
    // Same discipline as the Gemini side: the address is turned into a
    // label immediately and the raw value shadowed, so no logging path
    // here can reach it (OQ-9).
    let peer = PeerLabel::new(&peer, config.log_peer);

    let read = timeout(
        Duration::from_secs(service.request_timeout_secs),
        read_request_line(&mut tcp, service.max_request_bytes),
    )
    .await;

    let line = match read {
        Ok(Ok(line)) => line,
        Ok(Err(TooLong)) => {
            // Refused, and refused silently: there is no error frame that
            // is meaningful across all four of these protocols, and a
            // peer flooding us is not owed an explanation.
            tracing::debug!(%peer, protocol = service.name, "request line exceeded the cap");
            return;
        }
        Err(_) => {
            tracing::debug!(%peer, protocol = service.name, "request read timed out (slow client)");
            return;
        }
    };

    let body = (service.handler)(line, config).await;

    match timeout(
        Duration::from_secs(service.response_timeout_secs),
        tcp.write_all(&body),
    )
    .await
    {
        Ok(Ok(())) => {
            tracing::info!(%peer, protocol = service.name, bytes = body.len(), "served");
        }
        Ok(Err(e)) => tracing::debug!(%peer, protocol = service.name, error = %e, "write failed"),
        Err(_) => tracing::debug!(%peer, protocol = service.name, "write timed out"),
    }
    // Closing *is* the end of the response in every one of these
    // protocols — there is no close_notify equivalent to get right.
    let _ = tcp.shutdown().await;
}

/// The request line was longer than the protocol's cap.
struct TooLong;

/// Read one LF-terminated line, refusing anything over `cap` bytes.
///
/// The cap is enforced against what has been *buffered*, not against a
/// length the peer claims, so a client that never sends a terminator is
/// bounded by memory as well as by the caller's deadline.
async fn read_request_line(tcp: &mut TcpStream, cap: usize) -> Result<Vec<u8>, TooLong> {
    let mut buf = Vec::with_capacity(128);
    let mut chunk = [0u8; 256];
    loop {
        let n = match tcp.read(&mut chunk).await {
            Ok(0) => return Ok(buf), // peer closed; let the parser judge it
            Ok(n) => n,
            Err(_) => return Ok(buf),
        };
        buf.extend_from_slice(&chunk[..n]);
        if buf.contains(&b'\n') {
            return Ok(buf);
        }
        if buf.len() > cap {
            return Err(TooLong);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A service that echoes back whatever line it was given, so the
    /// tests exercise the listener rather than a protocol.
    fn echo_service() -> Service {
        Service {
            name: "test",
            max_request_bytes: 64,
            request_timeout_secs: 2,
            response_timeout_secs: 2,
            handler: Arc::new(|line, _config| {
                Box::pin(async move {
                    let mut out = b"got:".to_vec();
                    out.extend_from_slice(&line);
                    out
                })
            }),
        }
    }

    /// The watch *senders* must outlive the test: dropping either one
    /// makes `changed()` resolve immediately, the accept loop returns,
    /// and every connection is refused. That is correct shutdown
    /// behaviour and a trap for the test harness, so they are handed
    /// back rather than left to fall out of scope.
    struct Running {
        addr: SocketAddr,
        _config_tx: watch::Sender<Arc<Config>>,
        _shutdown_tx: watch::Sender<bool>,
    }

    async fn serve_one(service: Service) -> Running {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let config = Arc::new(Config::from_toml_str("", &Default::default()).expect("defaults"));
        let (config_tx, rx) = watch::channel(config);
        let (shutdown_tx, srx) = watch::channel(false);
        let permits = Arc::new(Semaphore::new(4));
        tokio::spawn(accept_loop(listener, service, rx, permits, srx));
        Running {
            addr,
            _config_tx: config_tx,
            _shutdown_tx: shutdown_tx,
        }
    }

    #[tokio::test]
    async fn a_complete_line_is_handled_and_the_connection_closes() {
        let s = serve_one(echo_service()).await;
        let mut c = TcpStream::connect(s.addr).await.expect("connect");
        c.write_all(b"/hello\r\n").await.expect("write");
        let mut out = Vec::new();
        // A read to EOF proves the server closed rather than hanging,
        // which is the whole response framing in these protocols.
        c.read_to_end(&mut out).await.expect("read");
        assert_eq!(out, b"got:/hello\r\n");
    }

    #[tokio::test]
    async fn a_bare_lf_line_is_handled_too() {
        let s = serve_one(echo_service()).await;
        let mut c = TcpStream::connect(s.addr).await.expect("connect");
        c.write_all(b"x\n").await.expect("write");
        let mut out = Vec::new();
        c.read_to_end(&mut out).await.expect("read");
        assert_eq!(out, b"got:x\n");
    }

    #[tokio::test]
    async fn an_unterminated_flood_is_dropped_without_a_response() {
        let s = serve_one(echo_service()).await;
        let mut c = TcpStream::connect(s.addr).await.expect("connect");
        // No newline, well past the 64-byte cap.
        let _ = c.write_all(&vec![b'a'; 4096]).await;
        let mut out = Vec::new();
        let _ = c.read_to_end(&mut out).await;
        assert!(out.is_empty(), "a flooding peer is owed nothing: {out:?}");
    }

    #[tokio::test]
    async fn a_silent_client_is_timed_out_rather_than_held() {
        let mut svc = echo_service();
        svc.request_timeout_secs = 1;
        let s = serve_one(svc).await;
        let mut c = TcpStream::connect(s.addr).await.expect("connect");
        // Never send anything at all.
        let mut out = Vec::new();
        let read = timeout(Duration::from_secs(5), c.read_to_end(&mut out)).await;
        assert!(read.is_ok(), "the server must drop us, not hold the socket");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn concurrent_connections_are_served() {
        let s = serve_one(echo_service()).await;
        let addr = s.addr;
        let mut handles = Vec::new();
        for i in 0..4 {
            handles.push(tokio::spawn(async move {
                let mut c = TcpStream::connect(addr).await.expect("connect");
                c.write_all(format!("/{i}\r\n").as_bytes())
                    .await
                    .expect("write");
                let mut out = Vec::new();
                c.read_to_end(&mut out).await.expect("read");
                out
            }));
        }
        for (i, h) in handles.into_iter().enumerate() {
            let out = h.await.expect("join");
            assert_eq!(out, format!("got:/{i}\r\n").as_bytes());
        }
    }
}
