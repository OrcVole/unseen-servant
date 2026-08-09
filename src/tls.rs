//! TLS server configuration (ADR 0001/0002; recon guidance §4–5).
//!
//! Policy, all decided in recon and ADRs — none of it is negotiable at this
//! layer:
//!
//! - **TLS 1.3 by default**, 1.2 only as explicit operator opt-in
//!   (`tls_min = "1.2"`).
//! - **SNI virtual hosting** via the [`IdentityStore`] resolver; no SNI
//!   serves the default host's certificate rather than crashing or
//!   refusing the handshake.
//! - **Client certificates are requested but never demanded** at the TLS
//!   layer, and *any* well-signed certificate is accepted here — including
//!   self-signed and expired ones. Protocol semantics (60/61/62, validity
//!   windows, fingerprint allowlists) are application decisions made per
//!   path scope (C2), not handshake decisions: a TLS-layer rejection would
//!   close the connection without a Gemini status line, which helps nobody.
//!   Possession of the private key IS verified (the CertificateVerify
//!   signature check stays on).
//! - **Session tickets and resumption off** (fingerprinting concern,
//!   contested issues #23/#39): no TLS 1.3 tickets, no 1.2 session cache.

use std::sync::Arc;

use rustls::crypto::CryptoProvider;
use rustls::crypto::ring as provider;
use rustls::server::NoServerSessionStorage;
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DistinguishedName, ServerConfig};
use rustls_pki_types::{CertificateDer, UnixTime};

use crate::config::TlsMinVersion;
use crate::identity::IdentityStore;

/// Build the rustls server configuration from validated config + identities.
pub fn server_config(
    tls_min: TlsMinVersion,
    identities: Arc<IdentityStore>,
) -> Result<Arc<ServerConfig>, rustls::Error> {
    let versions: &[&rustls::SupportedProtocolVersion] = match tls_min {
        TlsMinVersion::V1_3 => &[&rustls::version::TLS13],
        TlsMinVersion::V1_2 => &[&rustls::version::TLS12, &rustls::version::TLS13],
    };
    let provider = Arc::new(provider::default_provider());
    let verifier = Arc::new(CaptureAnyClientCert {
        provider: provider.clone(),
    });
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(versions)?
        .with_client_cert_verifier(verifier)
        .with_cert_resolver(identities);

    // TOFU-privacy posture: no tickets, no resumption, no 0-RTT.
    config.send_tls13_tickets = 0;
    config.session_storage = Arc::new(NoServerSessionStorage {});
    config.max_early_data_size = 0;
    Ok(Arc::new(config))
}

/// A client-certificate verifier that *captures* rather than *judges*.
///
/// Gemini's 6x semantics require the server to see invalid certificates and
/// answer them with a status line (62), which a strict TLS-layer verifier
/// makes impossible. So: request a certificate, accept whatever arrives,
/// verify only the handshake signature (proof of key possession — without
/// it, "the client presented cert X" means nothing), and let request
/// handling read the captured chain from the connection afterwards.
#[derive(Debug)]
struct CaptureAnyClientCert {
    provider: Arc<CryptoProvider>,
}

impl ClientCertVerifier for CaptureAnyClientCert {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        // Status 60 ("certificate required") is a per-path decision made by
        // handlers; the handshake itself never demands one.
        false
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // No CA hints: Gemini client certs are overwhelmingly self-signed
        // and clients pick identity by user choice, not by CA membership.
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        // Deliberately no chain building, no expiry check, no name check:
        // the protocol layer owns those judgments (statuses 61/62).
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(test: &str) -> Arc<IdentityStore> {
        let dir = std::env::temp_dir().join(format!("usv-tls-test-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store =
            IdentityStore::open(&dir, &["localhost".to_string()]).expect("mint test identity");
        Arc::new(store)
    }

    #[test]
    fn tls13_default_builds_without_tickets() {
        let cfg = server_config(TlsMinVersion::V1_3, store("v13")).expect("builds");
        assert_eq!(cfg.send_tls13_tickets, 0, "tickets are off by policy");
        assert_eq!(cfg.max_early_data_size, 0, "no 0-RTT");
    }

    #[test]
    fn tls12_opt_in_builds() {
        server_config(TlsMinVersion::V1_2, store("v12")).expect("builds with 1.2 floor");
    }
}
