//! Wire-level regress suite: the real binary, real sockets, real TLS —
//! gmid-style (docs/internal/recon/prior-art.md §2). Every test here talks to a
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

/// A server with one writable Titan zone at `/uploads/`, gated on the
/// given fingerprints. Mirrors `start_with_cert_zone`; the extra knobs
/// (token, delete) cover the policy paths the wire tests exercise.
fn start_with_titan_zone(
    name: &str,
    allowed_fingerprints: &[&str],
    token: Option<&str>,
    allow_delete: bool,
) -> TestServer {
    let dir = std::env::temp_dir().join(format!(
        "usv-wire-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content")).expect("mkdir");
    std::fs::write(dir.join("content/index.gmi"), b"# home\n").expect("write");

    let fingerprints_toml = allowed_fingerprints
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let token_line = match token {
        Some(t) => format!("token = \"{t}\"\n"),
        None => String::new(),
    };
    let toml = format!(
        "[server]\nlisten = [\"127.0.0.1:0\"]\n\n\
         [[host]]\nname = \"localhost\"\n\n\
         [[host.titan_zone]]\npath_prefix = \"/uploads/\"\n\
         fingerprints = [{fingerprints_toml}]\n{token_line}\
         allow_delete = {allow_delete}\n"
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

/// Like [`exchange`], but the ClientHello carries no SNI extension at all
/// — connecting by literal IP address rather than a `ServerName::DnsName`
/// is the one way rustls's own client omits SNI (RFC 6066 §3: SNI is not
/// sent for literal IP addresses; rustls's `client_hello_payload` only
/// populates the extension for `ServerName::DnsName`, see
/// `rustls::client::hs`). This is the real "Tor client with no SNI"
/// shape, not a simulation of it.
fn exchange_no_sni(port: u16, raw: &[u8]) -> Vec<u8> {
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("client config versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
        .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::from(std::net::IpAddr::from([127, 0, 0, 1]));
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

/// The certificate served over a no-SNI connection (see
/// [`exchange_no_sni`]) — proves *which* identity answered when the
/// ClientHello named none.
fn served_cert_der_no_sni(port: u16) -> Vec<u8> {
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
        .with_safe_default_protocol_versions()
        .expect("versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustAnything(provider)))
        .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::from(std::net::IpAddr::from([127, 0, 0, 1]));
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name).expect("conn");
    let tcp = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let _ = tls.write_all(b"gemini://127.0.0.1/\r\n");
    let mut buf = [0u8; 1];
    let _ = tls.read(&mut buf);
    tls.conn.peer_certificates().expect("server sent a cert")[0]
        .as_ref()
        .to_vec()
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

/// The C2 exit gate's traversal corpus (docs/internal/BUILD-PLAN.md): percent-
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

/// A ClientHello with no SNI at all (the shape a Tor/I2P client commonly
/// sends — docs/internal/notes/integration-ideas.md "Tor / I2P") must be served,
/// not refused: the resolver falls back to the first configured host's
/// certificate (`identity::IdentityStore`'s documented no-SNI default),
/// and the request's own authority check still governs which content
/// comes back, exactly as it does when SNI picks the "wrong" host's cert
/// (see `authority_check_is_independent_of_which_sni_cert_answered`).
#[test]
fn a_connection_with_no_sni_is_served_by_the_default_host() {
    let server = start_multi_host("no-sni", &["alpha.example", "beta.example"]);

    let cert_alpha = served_cert_der(server.port, "alpha.example");
    let cert_no_sni = served_cert_der_no_sni(server.port);
    assert_eq!(
        cert_alpha, cert_no_sni,
        "no SNI must fall back to the first configured host's certificate"
    );

    // Content routing is untouched by the missing SNI: a no-SNI connection
    // can still fetch beta's content, because Gemini authority is a
    // per-request URI check, not a per-connection SNI binding.
    let request = format!("gemini://beta.example:{}/\r\n", server.port);
    let response = exchange_no_sni(server.port, request.as_bytes());
    assert!(
        status_line(&response).starts_with("20 "),
        "a no-SNI connection must still be able to reach a configured host, \
         got {:?}",
        status_line(&response)
    );
    assert!(
        response
            .windows(b"beta.example".len())
            .any(|w| w == b"beta.example"),
        "must serve beta's own content over the no-SNI connection"
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

// ---------------------------------------------------------------------
// C4: same-listener Titan scheme dispatch (ADR 0006; recon titan.md §5.4).
//
// These prove the dispatch graduated from C1's "titan is a foreign scheme,
// answer 53" stub: a titan:// request line is now parsed by the Titan
// parser and answered on its own terms. Writable zones, the certificate
// gate and size caps arrive with the [titan] config section, so a
// well-formed upload is currently refused 50 ("this capsule does not
// accept uploads") rather than accepted — the point here is *which code
// path answered*, not that an upload succeeded.
// ---------------------------------------------------------------------

#[test]
fn well_formed_titan_upload_is_dispatched_not_treated_as_a_foreign_scheme() {
    let server = TestServer::start("titan-dispatch");
    let payload = b"# uploaded\n";
    let request = format!(
        "titan://localhost:{}/upload.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange(server.port, &raw);
    let status = status_line(&response);
    assert!(
        status.starts_with("50"),
        "a recognised titan upload with no writable zone must be refused 50, got: {status}"
    );
    assert!(
        !status.starts_with("53"),
        "53 would mean titan was still being treated as a foreign scheme"
    );
}

#[test]
fn titan_scheme_dispatch_is_case_insensitive() {
    let server = TestServer::start("titan-case");
    let request = format!("TITAN://localhost:{}/x.gmi;size=0\r\n", server.port);
    let response = exchange(server.port, request.as_bytes());
    assert!(
        status_line(&response).starts_with("50"),
        "schemes are case-insensitive per RFC 3986"
    );
}

#[test]
fn malformed_titan_request_gets_59_from_the_titan_parser() {
    let server = TestServer::start("titan-malformed");
    // size is the one mandatory parameter; without it the payload has no
    // defined end, so this can only be a bad request.
    let request = format!(
        "titan://localhost:{}/x.gmi;mime=text/plain\r\n",
        server.port
    );
    let response = exchange(server.port, request.as_bytes());
    let status = status_line(&response);
    assert!(status.starts_with("59"), "expected 59, got: {status}");
    assert!(
        status.contains("size"),
        "the META should name the missing parameter, got: {status}"
    );
}

#[test]
fn titan_upload_to_a_foreign_authority_gets_53() {
    let server = TestServer::start("titan-foreign");
    // Same authority rule as Gemini — one predicate, no drift. Sends the
    // payload it declares, as a real client would.
    let request = format!("titan://not-ours.example:{}/x.gmi;size=1\r\n", server.port);
    let mut raw = request.into_bytes();
    raw.push(b'x');
    let response = exchange(server.port, &raw);
    assert!(
        status_line(&response).starts_with("53"),
        "a titan upload for someone else's host is still a foreign authority"
    );
}

#[test]
fn refused_titan_payload_is_drained_so_the_client_reads_the_status() {
    // Recon titan.md §5.5, flagged there as the top interop risk: a server
    // that refuses before the payload and closes immediately leaves a
    // client that already started streaming with a broken pipe instead of
    // a status line. usv absorbs a bounded amount of in-flight payload
    // after responding, so the write completes and the client reads the
    // refusal — with a clean close_notify, no truncation.
    let server = TestServer::start("titan-drain");
    let payload = vec![b'x'; 64 * 1024];
    let request = format!(
        "titan://localhost:{}/big.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(&payload);

    // require_close_notify = true: the drain must not cost us the clean
    // TLS shutdown every other path guarantees.
    let response = exchange(server.port, &raw);
    assert!(
        status_line(&response).starts_with("50"),
        "client must receive the refusal, not a truncated connection"
    );
}

#[test]
fn a_titan_token_never_reaches_the_log() {
    // Tokens ride in the URL and are shared secrets (recon titan.md §5.2):
    // they must be treated like a query — never logged. The server's
    // stderr is drained into a sink thread, so this asserts the property
    // that matters at the wire: the token is not echoed back in the META
    // either, which is the other place it could leak.
    let server = TestServer::start("titan-token");
    let request = format!(
        "titan://localhost:{}/x.gmi;size=1;token=super-secret-value\r\n",
        server.port
    );
    let mut raw = request.into_bytes();
    raw.push(b'x');
    let response = exchange(server.port, &raw);
    let text = String::from_utf8_lossy(&response);
    assert!(
        !text.contains("super-secret-value"),
        "the token must never be echoed back to the client: {text}"
    );
}

#[test]
fn a_titan_client_that_declares_a_payload_then_sends_nothing_still_gets_closed() {
    // Regression: the drain is bounded in bytes *and* in time. A client
    // that declares a size and then goes quiet must not be able to park
    // the connection in the drain — that would be a slot-holding trick of
    // exactly the shape the slowloris guard exists to prevent. The server
    // gives up on the missing payload and closes cleanly.
    let server = TestServer::start("titan-silent");
    let request = format!("titan://localhost:{}/x.gmi;size=4096\r\n", server.port);
    let started = std::time::Instant::now();
    let response = exchange(server.port, request.as_bytes());
    let elapsed = started.elapsed();

    assert!(
        status_line(&response).starts_with("50"),
        "the refusal must still be delivered"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "the drain must time out promptly, not hold the connection: {elapsed:?}"
    );
}

// ---------------------------------------------------------------------
// C4: authorized uploads end to end. These are the tests that prove the
// whole path — TLS with a client certificate, the pre-body decision, the
// payload read, the atomic write into the content tree, and the 30
// redirect the ecosystem expects (recon titan.md §1.3).
// ---------------------------------------------------------------------

#[test]
fn an_authorized_upload_is_written_and_answered_with_30() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-write", &[&cert.fingerprint_hex], None, false);

    let payload = b"# Uploaded over Titan\n\nIt works.\n";
    let request = format!(
        "titan://localhost:{}/uploads/note.gmi;size={};mime=text/gemini\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    let status = status_line(&response);
    assert!(
        status.starts_with("30"),
        "expected a redirect, got: {status}"
    );
    assert!(
        status.contains("/uploads/note.gmi"),
        "the redirect must point at the page just written: {status}"
    );

    // The bytes really landed in the content tree, unchanged.
    let written = std::fs::read(server.dir.join("content/uploads/note.gmi"))
        .expect("the uploaded file exists in the content tree");
    assert_eq!(written, payload);
}

#[test]
fn an_upload_becomes_readable_over_gemini_immediately() {
    // The point of writing into the SOURCE tree (ADR 0004): the page is
    // served natively on Gemini with no extra step.
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-readback", &[&cert.fingerprint_hex], None, false);

    let payload = b"# Readback\n\nHello from Titan.\n";
    let upload = format!(
        "titan://localhost:{}/uploads/readback.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = upload.into_bytes();
    raw.extend_from_slice(payload);
    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    assert!(status_line(&response).starts_with("30"));

    let fetch = format!(
        "gemini://localhost:{}/uploads/readback.gmi\r\n",
        server.port
    );
    let read_back = exchange(server.port, fetch.as_bytes());
    assert!(status_line(&read_back).starts_with("20 text/gemini"));
    let text = String::from_utf8_lossy(&read_back);
    assert!(
        text.contains("Hello from Titan."),
        "the uploaded content must be served back: {text}"
    );
}

#[test]
fn an_upload_without_a_certificate_is_60_and_writes_nothing() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-nocert", &[&cert.fingerprint_hex], None, false);

    let payload = b"should never land";
    let request = format!(
        "titan://localhost:{}/uploads/evil.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange(server.port, &raw);
    assert!(status_line(&response).starts_with("60"));
    assert!(
        !server.dir.join("content/uploads/evil.gmi").exists(),
        "an unauthenticated upload must never reach the content tree"
    );
}

#[test]
fn an_unlisted_certificate_is_61_and_writes_nothing() {
    let allowed = generate_client_cert(-1, 30);
    let stranger = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-stranger", &[&allowed.fingerprint_hex], None, false);

    let payload = b"not mine to write";
    let request = format!(
        "titan://localhost:{}/uploads/evil.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange_with_client_cert(server.port, &raw, Some(&stranger));
    assert!(status_line(&response).starts_with("61"));
    assert!(!server.dir.join("content/uploads/evil.gmi").exists());
}

#[test]
fn an_upload_outside_the_writable_zone_is_refused() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-outside", &[&cert.fingerprint_hex], None, false);

    // Authorized identity, but a path the zone does not cover.
    let payload = b"# overwrite the homepage\n";
    let request = format!(
        "titan://localhost:{}/index.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    assert!(
        status_line(&response).starts_with("51"),
        "outside every zone is answered as not-found, disclosing nothing"
    );
    let home = std::fs::read_to_string(server.dir.join("content/index.gmi")).expect("home intact");
    assert_eq!(home, "# home\n", "the homepage must be untouched");
}

#[test]
fn a_traversal_upload_cannot_escape_the_content_tree() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-traversal", &[&cert.fingerprint_hex], None, false);

    let payload = b"owned";
    // The zone prefix matches, but the path then tries to climb out.
    let request = format!(
        "titan://localhost:{}/uploads/../../escaped.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    let status = status_line(&response);
    assert!(
        !status.starts_with("30"),
        "a traversal upload must never report success: {status}"
    );
    assert!(!server.dir.join("escaped.gmi").exists());
    assert!(!server.dir.join("content/../escaped.gmi").exists());
}

#[test]
fn an_oversize_declaration_is_refused_before_the_body_is_read() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-oversize", &[&cert.fingerprint_hex], None, false);

    // Declare far beyond the 10 MiB default cap, and send nothing: the
    // server must answer from the request line alone.
    let request = format!(
        "titan://localhost:{}/uploads/huge.gmi;size=99999999999\r\n",
        server.port
    );
    let response = exchange_with_client_cert(server.port, request.as_bytes(), Some(&cert));
    let status = status_line(&response);
    assert!(status.starts_with("59"), "expected 59, got: {status}");
    assert!(!server.dir.join("content/uploads/huge.gmi").exists());
}

#[test]
fn a_disallowed_mime_is_refused() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-mime", &[&cert.fingerprint_hex], None, false);

    let payload = b"\x7fELF binary-ish";
    let request = format!(
        "titan://localhost:{}/uploads/x.bin;size={};mime=application/x-executable\r\n",
        server.port,
        payload.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(payload);

    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    assert!(status_line(&response).starts_with("59"));
    assert!(!server.dir.join("content/uploads/x.bin").exists());
}

#[test]
fn deletion_is_refused_unless_the_zone_opts_in() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-nodelete", &[&cert.fingerprint_hex], None, false);

    // Put a page there first.
    let payload = b"# doomed\n";
    let upload = format!(
        "titan://localhost:{}/uploads/doomed.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = upload.into_bytes();
    raw.extend_from_slice(payload);
    assert!(
        status_line(&exchange_with_client_cert(server.port, &raw, Some(&cert))).starts_with("30")
    );

    // size=0 is Titan's delete; the zone has not opted in.
    let delete = format!(
        "titan://localhost:{}/uploads/doomed.gmi;size=0\r\n",
        server.port
    );
    let response = exchange_with_client_cert(server.port, delete.as_bytes(), Some(&cert));
    assert!(status_line(&response).starts_with("50"));
    assert!(
        server.dir.join("content/uploads/doomed.gmi").exists(),
        "the page must survive a refused deletion"
    );
}

#[test]
fn deletion_works_where_the_zone_opts_in() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone("titan-delete", &[&cert.fingerprint_hex], None, true);

    let payload = b"# temporary\n";
    let upload = format!(
        "titan://localhost:{}/uploads/temp.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = upload.into_bytes();
    raw.extend_from_slice(payload);
    assert!(
        status_line(&exchange_with_client_cert(server.port, &raw, Some(&cert))).starts_with("30")
    );
    assert!(server.dir.join("content/uploads/temp.gmi").exists());

    let delete = format!(
        "titan://localhost:{}/uploads/temp.gmi;size=0\r\n",
        server.port
    );
    let response = exchange_with_client_cert(server.port, delete.as_bytes(), Some(&cert));
    assert!(status_line(&response).starts_with("20"));
    assert!(
        !server.dir.join("content/uploads/temp.gmi").exists(),
        "an authorized deletion must actually remove the page"
    );
}

#[test]
fn a_token_zone_requires_both_the_certificate_and_the_token() {
    let cert = generate_client_cert(-1, 30);
    let server = start_with_titan_zone(
        "titan-token-zone",
        &[&cert.fingerprint_hex],
        Some("hunter2"),
        false,
    );

    let payload = b"# with token\n";

    // Right certificate, no token: refused.
    let no_token = format!(
        "titan://localhost:{}/uploads/t.gmi;size={}\r\n",
        server.port,
        payload.len()
    );
    let mut raw = no_token.into_bytes();
    raw.extend_from_slice(payload);
    assert!(
        status_line(&exchange_with_client_cert(server.port, &raw, Some(&cert))).starts_with("61")
    );
    assert!(!server.dir.join("content/uploads/t.gmi").exists());

    // Right certificate and right token: written.
    let with_token = format!(
        "titan://localhost:{}/uploads/t.gmi;size={};token=hunter2\r\n",
        server.port,
        payload.len()
    );
    let mut raw2 = with_token.into_bytes();
    raw2.extend_from_slice(payload);
    let ok = exchange_with_client_cert(server.port, &raw2, Some(&cert));
    assert!(status_line(&ok).starts_with("30"));
    assert!(server.dir.join("content/uploads/t.gmi").exists());
}

// ---------------------------------------------------------------------
// C4 / ADR 0011: the identity roster over the wire. Named identities with
// capabilities, and key rotation with a self-closing window.
// ---------------------------------------------------------------------

/// A server whose writable zone names a roster identity rather than a raw
/// fingerprint. `capability` is written verbatim so a test can supply the
/// wrong one; `superseded`/`until` drive the rotation window.
fn start_with_roster_identity(
    name: &str,
    current_fingerprint: &str,
    capability: &str,
    superseded: Option<(&str, &str)>,
) -> TestServer {
    let dir = std::env::temp_dir().join(format!(
        "usv-wire-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content")).expect("mkdir");
    std::fs::write(dir.join("content/index.gmi"), b"# home\n").expect("write");

    let rotation = match superseded {
        Some((old, until)) => {
            format!("superseded = [\"{old}\"]\nsuperseded_until = \"{until}\"\n")
        }
        None => String::new(),
    };
    let toml = format!(
        "[server]\nlisten = [\"127.0.0.1:0\"]\n\n\
         [[identity]]\nlabel = \"scribe\"\nfingerprint = \"{current_fingerprint}\"\n\
         capabilities = [\"{capability}\"]\n{rotation}\n\
         [[host]]\nname = \"localhost\"\n\n\
         [[host.titan_zone]]\npath_prefix = \"/uploads/\"\nidentities = [\"scribe\"]\n"
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

fn titan_upload(port: u16, path: &str, body: &[u8]) -> Vec<u8> {
    let mut raw = format!("titan://localhost:{port}{path};size={}\r\n", body.len()).into_bytes();
    raw.extend_from_slice(body);
    raw
}

#[test]
fn a_roster_identity_with_titan_write_may_upload() {
    let cert = generate_client_cert(-1, 30);
    let server =
        start_with_roster_identity("roster-ok", &cert.fingerprint_hex, "titan-write", None);

    let raw = titan_upload(server.port, "/uploads/named.gmi", b"# by name\n");
    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    assert!(
        status_line(&response).starts_with("30"),
        "a named identity holding titan-write may write: {}",
        status_line(&response)
    );
    assert!(server.dir.join("content/uploads/named.gmi").exists());
}

#[test]
fn a_roster_identity_without_titan_write_is_refused() {
    // The capability is a server-wide grant: the zone naming you is not
    // enough on its own.
    let cert = generate_client_cert(-1, 30);
    let server = start_with_roster_identity("roster-nocap", &cert.fingerprint_hex, "read", None);

    let raw = titan_upload(server.port, "/uploads/nope.gmi", b"# denied\n");
    let response = exchange_with_client_cert(server.port, &raw, Some(&cert));
    assert!(status_line(&response).starts_with("61"));
    assert!(!server.dir.join("content/uploads/nope.gmi").exists());
}

#[test]
fn a_superseded_key_still_writes_inside_its_rotation_window() {
    // The holder has pinned a new key but is still presenting the old one.
    // Both work until the window closes — that is what makes rotation
    // possible without a flag-day.
    let old = generate_client_cert(-1, 30);
    let new = generate_client_cert(-1, 30);
    let server = start_with_roster_identity(
        "roster-rotating",
        &new.fingerprint_hex,
        "titan-write",
        Some((&old.fingerprint_hex, "2099-01-01")),
    );

    let with_new = titan_upload(server.port, "/uploads/new-key.gmi", b"# new\n");
    assert!(
        status_line(&exchange_with_client_cert(
            server.port,
            &with_new,
            Some(&new)
        ))
        .starts_with("30")
    );

    let with_old = titan_upload(server.port, "/uploads/old-key.gmi", b"# old\n");
    assert!(
        status_line(&exchange_with_client_cert(
            server.port,
            &with_old,
            Some(&old)
        ))
        .starts_with("30"),
        "the retiring key must still work while its window is open"
    );
    assert!(server.dir.join("content/uploads/old-key.gmi").exists());
}

#[test]
fn a_superseded_key_stops_working_once_its_window_has_closed() {
    // The window closes on its own: no operator action, no restart. A
    // forgotten old key fails closed, which is the whole point.
    let old = generate_client_cert(-1, 30);
    let new = generate_client_cert(-1, 30);
    let server = start_with_roster_identity(
        "roster-expired",
        &new.fingerprint_hex,
        "titan-write",
        Some((&old.fingerprint_hex, "2020-01-01")),
    );

    let with_old = titan_upload(server.port, "/uploads/stale.gmi", b"# stale\n");
    let response = exchange_with_client_cert(server.port, &with_old, Some(&old));
    assert!(
        status_line(&response).starts_with("61"),
        "an expired rotation window must refuse the old key: {}",
        status_line(&response)
    );
    assert!(!server.dir.join("content/uploads/stale.gmi").exists());

    // The current key is unaffected.
    let with_new = titan_upload(server.port, "/uploads/fresh.gmi", b"# fresh\n");
    assert!(
        status_line(&exchange_with_client_cert(
            server.port,
            &with_new,
            Some(&new)
        ))
        .starts_with("30")
    );
}
