//! Wire-level regress suite: the real binary, real sockets, real TLS —
//! gmid-style (docs/recon/prior-art.md §2). Every test here talks to a
//! spawned `usv` process over an actual TLS connection.
//!
//! These tests cover the request/response contract the gemini-diagnostics
//! gate also checks, so regressions are caught by `cargo test` long before
//! the gate runs — plus the close_notify behavior that a Python client
//! can't assert as precisely.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

// ---------------------------------------------------------------- server

/// A spawned `usv` with an ephemeral port and throwaway state dir.
struct TestServer {
    child: std::process::Child,
    port: u16,
    dir: std::path::PathBuf,
}

impl TestServer {
    fn start(name: &str) -> TestServer {
        let dir = std::env::temp_dir().join(format!(
            "usv-wire-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .subsec_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_usv"))
            .env("USV_STATE_DIR", &dir)
            .env("USV_LISTEN", "127.0.0.1:0")
            .env("USV_HOSTNAME", "localhost")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("usv spawns");

        // The bound address is announced on stderr; parse the port out.
        let stderr = child.stderr.take().expect("piped stderr");
        let mut reader = std::io::BufReader::new(stderr);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut port = None;
        let mut line = String::new();
        use std::io::BufRead;
        while std::time::Instant::now() < deadline {
            line.clear();
            if reader.read_line(&mut line).expect("read stderr") == 0 {
                break;
            }
            if let Some(idx) = line.find("bound=127.0.0.1:") {
                let digits: String = line[idx + "bound=127.0.0.1:".len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                port = digits.parse().ok();
            }
            if port.is_some() && line.contains("serving") {
                break;
            }
        }
        let port = port.expect("server announced its bound port");
        // Keep draining stderr forever so the child never blocks on it.
        std::thread::spawn(move || {
            let mut sink = String::new();
            let _ = reader.read_to_string(&mut sink);
        });
        TestServer { child, port, dir }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// ---------------------------------------------------------------- client

/// Accept-anything server-cert verifier: the TOFU client stance, and the
/// only way to talk to a self-signed test server. Signature checks stay on.
#[derive(Debug)]
struct TrustAnything(rustls::crypto::CryptoProvider);

impl rustls::client::danger::ServerCertVerifier for TrustAnything {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Open a TLS connection (SNI `localhost`), send `raw` bytes, read to EOF.
///
/// Returns the raw response bytes. Panics on TLS-level truncation — i.e. a
/// missing close_notify — so every test doubles as the close_notify check
/// (diagnostics check 6).
fn exchange(port: u16, raw: &[u8]) -> Vec<u8> {
    exchange_expect(port, raw, true)
}

/// Like [`exchange`] but with the close_notify assertion switchable, for
/// the paths where the *absence of data* is the assertion.
fn exchange_expect(port: u16, raw: &[u8], require_close_notify: bool) -> Vec<u8> {
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("client config versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
        .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::try_from("localhost").expect("name");
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name).expect("client conn");
    let tcp = TcpStream::connect(("127.0.0.1", port)).expect("tcp connect");
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .expect("timeout");
    let mut tls = rustls::StreamOwned::new(conn, tcp);

    tls.write_all(raw).expect("request written");
    tls.flush().expect("flushed");

    let mut response = Vec::new();
    match tls.read_to_end(&mut response) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            assert!(
                !require_close_notify,
                "server closed without TLS close_notify (truncation): {e}"
            );
        }
        Err(e) => panic!("read failed: {e}"),
    }
    response
}

fn status_line(response: &[u8]) -> String {
    let end = response
        .windows(2)
        .position(|w| w == b"\r\n")
        .expect("response has a CRLF-terminated header");
    String::from_utf8_lossy(&response[..end]).into_owned()
}

// ---------------------------------------------------------------- tests

#[test]
fn homepage_serves_20_with_gemtext_and_body() {
    let server = TestServer::start("home");
    let request = format!("gemini://localhost:{}/\r\n", server.port);
    let response = exchange(server.port, request.as_bytes());
    let header = status_line(&response);
    assert!(
        header.starts_with("20 text/gemini"),
        "expected 20 text/gemini, got {header:?}"
    );
    assert_eq!(&header[2..3], " ", "exactly one SP after the status");
    let body_start = response
        .windows(2)
        .position(|w| w == b"\r\n")
        .expect("crlf")
        + 2;
    assert!(!response[body_start..].is_empty(), "non-empty body");
}

#[test]
fn empty_path_is_the_homepage_without_redirect() {
    let server = TestServer::start("emptypath");
    let request = format!("gemini://localhost:{}\r\n", server.port);
    let response = exchange(server.port, request.as_bytes());
    assert!(
        status_line(&response).starts_with("20 "),
        "empty path must serve, not redirect (spec MUST)"
    );
}

#[test]
fn explicit_port_in_url_is_accepted() {
    let server = TestServer::start("port");
    let request = format!("gemini://localhost:{}/\r\n", server.port);
    let response = exchange(server.port, request.as_bytes());
    assert!(status_line(&response).starts_with("20 "));
}

#[test]
fn unknown_path_is_51_not_found() {
    let server = TestServer::start("notfound");
    let request = format!("gemini://localhost:{}/does-not-exist\r\n", server.port);
    let response = exchange(server.port, request.as_bytes());
    assert!(status_line(&response).starts_with("51"));
}

#[test]
fn foreign_schemes_get_53() {
    let server = TestServer::start("schemes");
    for scheme in ["http", "https", "gopher"] {
        let request = format!("{scheme}://localhost/\r\n");
        let response = exchange(server.port, request.as_bytes());
        assert!(
            status_line(&response).starts_with("53"),
            "{scheme} should be refused with 53"
        );
    }
}

#[test]
fn wrong_host_and_wrong_port_get_53() {
    let server = TestServer::start("authority");
    let response = exchange(server.port, b"gemini://other.example/\r\n");
    assert!(status_line(&response).starts_with("53"));
    let request = format!("gemini://localhost:{}/\r\n", server.port.wrapping_add(1));
    let response = exchange(server.port, request.as_bytes());
    assert!(status_line(&response).starts_with("53"));
}

#[test]
fn bad_requests_get_59() {
    let server = TestServer::start("bad59");
    let cases: Vec<Vec<u8>> = vec![
        b"\r\n".to_vec(),                               // empty request
        b"/relative/path\r\n".to_vec(),                 // no scheme
        b"gemini://user@localhost/\r\n".to_vec(),       // userinfo
        b"gemini://localhost/page#frag\r\n".to_vec(),   // fragment
        b"gemini://localhost/caf\xc3\xa9\r\n".to_vec(), // raw non-ASCII
        b"gemini://localhost/%zz\r\n".to_vec(),         // bad pct-encoding
    ];
    for raw in cases {
        let response = exchange(server.port, &raw);
        assert!(
            status_line(&response).starts_with("59"),
            "expected 59 for {:?}, got {:?}",
            String::from_utf8_lossy(&raw),
            status_line(&response)
        );
    }
}

#[test]
fn oversize_uri_gets_59() {
    let server = TestServer::start("oversize");
    let mut raw = b"gemini://localhost/".to_vec();
    raw.extend(std::iter::repeat_n(b'a', 1024));
    raw.extend_from_slice(b"\r\n");
    let response = exchange(server.port, &raw);
    assert!(status_line(&response).starts_with("59"));
}

#[test]
fn exactly_1024_byte_uri_is_handled_not_rejected() {
    let server = TestServer::start("max1024");
    // Build a URI of exactly 1024 bytes: prefix + padding path segment.
    let prefix = format!("gemini://localhost:{}/", server.port);
    let mut uri = prefix.clone();
    uri.push_str(&"a".repeat(1024 - prefix.len()));
    assert_eq!(uri.len(), 1024);
    let response = exchange(server.port, format!("{uri}\r\n").as_bytes());
    assert!(
        status_line(&response).starts_with("51"),
        "1024-byte URI parses fine and the path is simply not found"
    );
}

#[test]
fn bare_lf_request_is_closed_without_response() {
    let server = TestServer::start("barelf");
    let response = exchange_expect(server.port, b"gemini://localhost/\n", false);
    assert!(
        response.is_empty(),
        "bare-LF requests get no response bytes (documented deviation; \
         diagnostics RequestMissingCR), got {:?}",
        String::from_utf8_lossy(&response)
    );
}

#[test]
fn two_concurrent_connections_are_served() {
    let server = TestServer::start("concurrent");
    let t1 = {
        let raw = format!("gemini://localhost:{}/\r\n", server.port).into_bytes();
        let port = server.port;
        std::thread::spawn(move || exchange(port, &raw))
    };
    let request = format!("gemini://localhost:{}/\r\n", server.port);
    let r2 = exchange(server.port, request.as_bytes());
    let r1 = t1.join().expect("thread joins");
    assert!(status_line(&r1).starts_with("20 "));
    assert!(status_line(&r2).starts_with("20 "));
}

#[test]
fn identity_is_stable_across_restarts() {
    // The TOFU promise at wire level: same state dir → same certificate.
    let fingerprint = |server: &TestServer| -> Vec<u8> {
        let provider = rustls::crypto::ring::default_provider();
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
            .with_no_client_auth();
        let name = rustls_pki_types::ServerName::try_from("localhost").expect("name");
        let conn = rustls::ClientConnection::new(Arc::new(config), name).expect("conn");
        let tcp = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        tls.write_all(format!("gemini://localhost:{}/\r\n", server.port).as_bytes())
            .expect("write");
        let mut buf = Vec::new();
        let _ = tls.read_to_end(&mut buf);
        tls.conn
            .peer_certificates()
            .expect("server sent a certificate")[0]
            .as_ref()
            .to_vec()
    };

    let cert_a;
    {
        let mut server = TestServer::start("tofu-a");
        // Redirect this server at the shared state dir by restarting it
        // there: simplest is to just use its own dir for the first read.
        cert_a = fingerprint(&server);
        let _ = server.child.kill();
        let _ = server.child.wait();
        // Re-spawn against the SAME state dir.
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_usv"))
            .env("USV_STATE_DIR", &server.dir)
            .env("USV_LISTEN", "127.0.0.1:0")
            .env("USV_HOSTNAME", "localhost")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("respawn");
        let stderr = child.stderr.take().expect("stderr");
        let mut reader = std::io::BufReader::new(stderr);
        use std::io::BufRead;
        let mut port = None;
        let mut line = String::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            line.clear();
            if reader.read_line(&mut line).expect("read") == 0 {
                break;
            }
            if let Some(idx) = line.find("bound=127.0.0.1:") {
                let digits: String = line[idx + "bound=127.0.0.1:".len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                port = digits.parse().ok();
            }
            if port.is_some() && line.contains("serving") {
                break;
            }
        }
        std::thread::spawn(move || {
            let mut sink = String::new();
            let _ = reader.read_to_string(&mut sink);
        });
        let second = TestServer {
            child,
            port: port.expect("second server announced port"),
            dir: server.dir.clone(),
        };
        server.dir = std::path::PathBuf::from("/nonexistent-usv-already-cleaned");
        let cert_b = fingerprint(&second);
        assert_eq!(
            cert_a, cert_b,
            "identity (certificate) must be byte-identical across restarts (ADR 0003)"
        );
    }
}
