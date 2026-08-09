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

use sha2::Digest;

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
        // C2: a real docroot with an index.gmi, so the wire suite exercises
        // actual static-file serving, not an absent-docroot 51.
        std::fs::create_dir_all(dir.join("content")).expect("mkdir content");
        std::fs::write(
            dir.join("content/index.gmi"),
            b"# Unseen Servant\n\ntest capsule\n",
        )
        .expect("write index.gmi");
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

/// A multi-host server, config-file driven (not the single-hostname
/// `USV_HOSTNAME` env override `start()` uses) — for SNI vhost-selection
/// tests, where the whole point is more than one configured host.
fn start_multi_host(name: &str, hosts: &[&str]) -> TestServer {
    let dir = std::env::temp_dir().join(format!(
        "usv-wire-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir state dir");

    let mut toml = String::from("[server]\nlisten = [\"127.0.0.1:0\"]\n\n");
    for h in hosts {
        let content_dir = dir.join(format!("content-{h}"));
        std::fs::create_dir_all(&content_dir).expect("mkdir content");
        std::fs::write(
            content_dir.join("index.gmi"),
            format!("# {h}\n\nhello from {h}\n"),
        )
        .expect("write index.gmi");
        toml.push_str(&format!(
            "[[host]]\nname = \"{h}\"\ndocroot = \"content-{h}\"\n\n"
        ));
    }
    std::fs::write(dir.join("usv.toml"), toml).expect("write usv.toml");

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_usv"))
        .env("USV_STATE_DIR", &dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("usv spawns");

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
    std::thread::spawn(move || {
        let mut sink = String::new();
        let _ = reader.read_to_string(&mut sink);
    });
    TestServer { child, port, dir }
}

/// A single-host server with a certificate zone at `/private/` for the 6x
/// client-cert-flow tests. `allowed_fingerprints` empty means "any valid
/// cert accepted"; non-empty means only those exact fingerprints pass 61.
fn start_with_cert_zone(name: &str, allowed_fingerprints: &[&str]) -> TestServer {
    let dir = std::env::temp_dir().join(format!(
        "usv-wire-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content/private")).expect("mkdir");
    std::fs::write(dir.join("content/index.gmi"), b"# home\n").expect("write");
    std::fs::write(dir.join("content/private/secret.gmi"), b"# secret\n").expect("write");

    let fingerprints_toml = allowed_fingerprints
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let toml = format!(
        "[server]\nlisten = [\"127.0.0.1:0\"]\n\n\
         [[host]]\nname = \"localhost\"\n\n\
         [[host.cert_zone]]\npath_prefix = \"/private/\"\nfingerprints = [{fingerprints_toml}]\n"
    );
    std::fs::write(dir.join("usv.toml"), toml).expect("write usv.toml");

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_usv"))
        .env("USV_STATE_DIR", &dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("usv spawns");

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
    std::thread::spawn(move || {
        let mut sink = String::new();
        let _ = reader.read_to_string(&mut sink);
    });
    TestServer { child, port, dir }
}

/// A self-signed client certificate for the 6x flow tests, with its
/// SHA-256 fingerprint precomputed (lowercase hex, matching what the
/// server computes over the presented DER — see `server.rs`'s
/// `extract_client_cert`).
struct GeneratedClientCert {
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    fingerprint_hex: String,
}

/// `not_before`/`not_after` as days offset from now (negative = past).
fn generate_client_cert(not_before_days: i64, not_after_days: i64) -> GeneratedClientCert {
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("params");
    params.not_before = time::OffsetDateTime::now_utc() + time::Duration::days(not_before_days);
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(not_after_days);
    let key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).expect("keypair");
    let cert = params.self_signed(&key_pair).expect("self-sign");
    let cert_der = cert.der().to_vec();
    let key_der = key_pair.serialize_der();
    let fingerprint_hex = sha2::Sha256::digest(&cert_der)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    GeneratedClientCert {
        cert_der,
        key_der,
        fingerprint_hex,
    }
}

/// Like [`exchange`] but presenting a client certificate — for the 6x
/// status-flow tests (60/61/62). `cert` is `None` for the no-cert case.
fn exchange_with_client_cert(port: u16, raw: &[u8], cert: Option<&GeneratedClientCert>) -> Vec<u8> {
    let provider = rustls::crypto::ring::default_provider();
    let builder = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)));
    let config = match cert {
        Some(c) => builder
            .with_client_auth_cert(
                vec![rustls_pki_types::CertificateDer::from(c.cert_der.clone())],
                rustls_pki_types::PrivateKeyDer::try_from(c.key_der.clone()).expect("key"),
            )
            .expect("client auth cert"),
        None => builder.with_no_client_auth(),
    };
    let server_name = rustls_pki_types::ServerName::try_from("localhost").expect("name");
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name).expect("conn");
    let tcp = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .expect("timeout");
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    tls.write_all(raw).expect("request written");
    tls.flush().expect("flushed");
    let mut response = Vec::new();
    let _ = tls.read_to_end(&mut response);
    response
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

/// Like [`exchange`] but with the SNI hostname parameterized, for
/// vhost-selection tests where the whole point is connecting under
/// different names against the same port.
fn exchange_sni(port: u16, sni: &str, raw: &[u8]) -> Vec<u8> {
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("client config versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
        .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::try_from(sni.to_string()).expect("name");
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name).expect("client conn");
    let tcp = TcpStream::connect(("127.0.0.1", port)).expect("tcp connect");
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .expect("timeout");
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    tls.write_all(raw).expect("request written");
    tls.flush().expect("flushed");
    let mut response = Vec::new();
    let _ = tls.read_to_end(&mut response);
    response
}

/// The certificate rustls actually selected for this connection's SNI —
/// lets a test prove *which* host's identity answered, not just that
/// *some* response came back.
fn served_cert_der(port: u16, sni: &str) -> Vec<u8> {
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
        .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::try_from(sni.to_string()).expect("name");
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name).expect("conn");
    let tcp = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    // A byte must cross the wire to force the handshake to complete before
    // peer_certificates() is populated.
    let _ = tls.write_all(format!("gemini://{sni}/\r\n").as_bytes());
    let mut buf = [0u8; 1];
    let _ = tls.read(&mut buf);
    tls.conn.peer_certificates().expect("server sent a cert")[0]
        .as_ref()
        .to_vec()
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

/// The C2 exit gate's traversal corpus (docs/BUILD-PLAN.md): percent-
/// encoded, double-encoded, and literal `..` escapes, over the real wire —
/// not just the unit-level `static_file` tests, since the whole pipeline
/// (URI validation → authority → static_file::serve) is what must hold.
#[test]
fn traversal_corpus_never_escapes_docroot() {
    let server = TestServer::start("traversal-corpus");
    let cases = [
        "/../../../etc/passwd",
        "/%2e%2e/%2e%2e/etc/passwd",
        "/%252e%252e/%252e%252e/etc/passwd",
        "/sub/../../../etc/passwd",
        "/..%2f..%2fetc/passwd",
        "/%2e%2e%2f%2e%2e%2fetc%2fpasswd",
    ];
    for path in cases {
        let request = format!("gemini://localhost:{}{path}\r\n", server.port);
        let response = exchange(server.port, request.as_bytes());
        let status = status_line(&response);
        assert!(
            status.starts_with('5'),
            "traversal case {path:?} must get a 5x permanent failure, got {status:?}"
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

// ------------------------------------------------------------- C2 exit gate

/// SNI must select both the right certificate AND the right docroot: two
/// hosts on one listener, distinguished only by the ClientHello's SNI name.
#[test]
fn sni_selects_the_matching_host_cert_and_content() {
    let server = start_multi_host("sni-vhost", &["alpha.example", "beta.example"]);

    let cert_alpha = served_cert_der(server.port, "alpha.example");
    let cert_beta = served_cert_der(server.port, "beta.example");
    assert_ne!(
        cert_alpha, cert_beta,
        "each host must present its own certificate (ADR 0003 per-hostname identity)"
    );

    let req_alpha = format!("gemini://alpha.example:{}/\r\n", server.port);
    let resp_alpha = exchange_sni(server.port, "alpha.example", req_alpha.as_bytes());
    assert!(status_line(&resp_alpha).starts_with("20 "));
    assert!(
        resp_alpha
            .windows(b"alpha.example".len())
            .any(|w| w == b"alpha.example"),
        "alpha's own content must be served under its own SNI"
    );

    let req_beta = format!("gemini://beta.example:{}/\r\n", server.port);
    let resp_beta = exchange_sni(server.port, "beta.example", req_beta.as_bytes());
    assert!(status_line(&resp_beta).starts_with("20 "));
    assert!(
        resp_beta
            .windows(b"beta.example".len())
            .any(|w| w == b"beta.example"),
        "beta's own content must be served under its own SNI"
    );
}

/// The authority check (layer 3) is keyed on the *request's own* hostname,
/// independent of which certificate SNI happened to select for this TLS
/// connection — requesting `beta.example`'s resource still serves beta's
/// content even when `alpha.example`'s cert answered the handshake.
/// Documented deliberately: usv has one authority list per server, not a
/// per-connection SNI==requested-host binding, since Gemini's one-request-
/// per-connection model gives no protocol reason to couple them and doing
/// so would just be surprising behavior for multi-hostname capsules.
#[test]
fn authority_check_is_independent_of_which_sni_cert_answered() {
    let server = start_multi_host("sni-authority", &["alpha.example", "beta.example"]);
    let request = format!("gemini://beta.example:{}/\r\n", server.port);
    let response = exchange_sni(server.port, "alpha.example", request.as_bytes());
    assert!(
        status_line(&response).starts_with("20 "),
        "beta.example is a configured host and must still be servable, \
         got {:?}",
        status_line(&response)
    );
    assert!(
        response
            .windows(b"beta.example".len())
            .any(|w| w == b"beta.example"),
        "must serve beta's own content, not alpha's"
    );
}

/// A client that opens the TCP+TLS connection but then sends its request
/// line at a trickle (or never finishes it) must be dropped once the
/// server's request timeout elapses, not held open indefinitely
/// (slowloris). Default `request_timeout_secs` is 10s (config default);
/// this asserts the connection is gone within a bounded margin past that.
#[test]
fn slow_client_is_dropped_after_request_timeout() {
    let server = TestServer::start("slowloris");
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
        .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::try_from("localhost").expect("name");
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name).expect("conn");
    let tcp = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
    let mut tls = rustls::StreamOwned::new(conn, tcp);

    // Complete the TLS handshake (a partial byte write forces it) but send
    // only half a request line — no CRLF ever arrives.
    tls.write_all(b"gemini://localhost/")
        .expect("partial write");
    tls.flush().expect("flush");

    // The server's default request_timeout_secs is 10; poll read_to_end
    // with a deadline safely past that. A hung server never returns here
    // and the test times out instead of passing — that failure mode is
    // exactly what this test exists to catch.
    tls.get_mut()
        .set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .expect("read timeout");
    let start = std::time::Instant::now();
    let mut buf = Vec::new();
    let _ = tls.read_to_end(&mut buf);
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "server must drop a slow client at its own request_timeout_secs (~10s), \
         not hold the connection until the test's 20s read timeout; elapsed {elapsed:?}"
    );
    assert!(
        elapsed >= std::time::Duration::from_secs(9),
        "server dropped the connection suspiciously early ({elapsed:?}); the \
         timeout should be close to request_timeout_secs (10s default), not instant"
    );
}

/// 60: a cert-zone-gated path with no client certificate presented.
#[test]
fn cert_zone_missing_cert_is_60() {
    let server = start_with_cert_zone("cert-60", &[]);
    let request = format!("gemini://localhost:{}/private/secret.gmi\r\n", server.port);
    let response = exchange_with_client_cert(server.port, request.as_bytes(), None);
    assert!(status_line(&response).starts_with("60"));
}

/// 62: a client certificate outside its validity window (expired here;
/// not-yet-valid would map to the same status).
#[test]
fn cert_zone_expired_cert_is_62() {
    let server = start_with_cert_zone("cert-62", &[]);
    let expired = generate_client_cert(-30, -1); // valid 30..1 days ago
    let request = format!("gemini://localhost:{}/private/secret.gmi\r\n", server.port);
    let response = exchange_with_client_cert(server.port, request.as_bytes(), Some(&expired));
    assert!(
        status_line(&response).starts_with("62"),
        "expired cert must get 62, got {:?}",
        status_line(&response)
    );
}

/// 61: a currently-valid certificate that simply isn't on the zone's
/// allowlist.
#[test]
fn cert_zone_unauthorized_cert_is_61() {
    let authorized = generate_client_cert(-1, 365);
    let server = start_with_cert_zone("cert-61", &[&authorized.fingerprint_hex]);
    let stranger = generate_client_cert(-1, 365);
    let request = format!("gemini://localhost:{}/private/secret.gmi\r\n", server.port);
    let response = exchange_with_client_cert(server.port, request.as_bytes(), Some(&stranger));
    assert!(
        status_line(&response).starts_with("61"),
        "a valid but non-allowlisted cert must get 61, got {:?}",
        status_line(&response)
    );
}

/// The positive case: a valid, allowlisted certificate reaches the
/// protected content (20), proving the whole 60/61/62 gate isn't just
/// rejecting everything.
#[test]
fn cert_zone_authorized_cert_reaches_content() {
    let authorized = generate_client_cert(-1, 365);
    let server = start_with_cert_zone("cert-authorized", &[&authorized.fingerprint_hex]);
    let request = format!("gemini://localhost:{}/private/secret.gmi\r\n", server.port);
    let response = exchange_with_client_cert(server.port, request.as_bytes(), Some(&authorized));
    assert!(
        status_line(&response).starts_with("20 "),
        "an authorized cert must reach the content, got {:?}",
        status_line(&response)
    );
}

/// Wire-level redirect regress: a config-file-driven redirect rule fires
/// over the real dispatch path (not just the `handler::redirect` unit
/// tests), and a non-matching path still falls through to static serving
/// on the same host.
#[test]
fn redirect_rule_fires_over_the_wire_and_non_matches_fall_through() {
    let dir = std::env::temp_dir().join(format!(
        "usv-wire-redirect-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content")).expect("mkdir");
    std::fs::write(dir.join("content/index.gmi"), b"# home\n").expect("write");
    std::fs::write(
        dir.join("usv.toml"),
        "[server]\nlisten = [\"127.0.0.1:0\"]\n\n\
         [[host]]\nname = \"localhost\"\n\n\
         [[host.redirect]]\npattern = \"^/old$\"\ntarget = \"/new\"\npermanent = true\n",
    )
    .expect("write usv.toml");

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_usv"))
        .env("USV_STATE_DIR", &dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("usv spawns");
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
    std::thread::spawn(move || {
        let mut sink = String::new();
        let _ = reader.read_to_string(&mut sink);
    });
    let server = TestServer { child, port, dir };

    let redirect_req = format!("gemini://localhost:{}/old\r\n", server.port);
    let resp = exchange(server.port, redirect_req.as_bytes());
    assert_eq!(status_line(&resp), "31 /new");

    let home_req = format!("gemini://localhost:{}/\r\n", server.port);
    let resp2 = exchange(server.port, home_req.as_bytes());
    assert!(
        status_line(&resp2).starts_with("20 "),
        "a non-matching path must still fall through to static serving"
    );
}
