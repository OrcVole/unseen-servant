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
//!   signature check stays on) — but that check is done against the
//!   certificate's bare `SubjectPublicKeyInfo`, not through webpki's full
//!   `EndEntityCert` parser. Found live, 2026-08-10, against a real
//!   Lagrange-generated client identity: `rustls::crypto::
//!   verify_tls13_signature`/`verify_tls12_signature` route the signature
//!   check through `webpki::EndEntityCert::try_from`, which hard-rejects
//!   any certificate whose ASN.1 version field is not explicitly `2`
//!   (X.509v3) — see `rustls-webpki`'s `cert::version3`. That field is
//!   `DEFAULT v1` per RFC 5280 §4.1.2.1 and is legitimately omitted by
//!   minimal certificate generators (Lagrange's identity certs among
//!   them), so a real, working Gemini client identity was being refused
//!   before its signature was ever checked — a version-parsing default
//!   rather than a security decision, and squarely webpki's business to
//!   have, not usv's to inherit for client-cert handshakes it deliberately
//!   never chain-validates in the first place. [`raw_spki_verify_tls12`] /
//!   [`raw_spki_verify_tls13`] below extract the SPKI directly
//!   (`x509-parser`, which is lenient about the version field) and verify
//!   against that with `webpki::RawPublicKeyEntity` (the RFC 7250 raw-key
//!   path, which never touches the version field at all), sidestepping the
//!   strict parser without weakening what is actually checked: the
//!   signature still must be valid for the presented key.
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
        raw_spki_verify_tls12(
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
        raw_spki_verify_tls13(
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

/// TLS 1.2 client-certificate signature verification against the bare
/// `SubjectPublicKeyInfo` (see module docs). Mirrors upstream
/// `rustls::crypto::verify_tls12_signature` exactly in algorithm
/// selection — a scheme may map to more than one candidate algorithm
/// family under TLS 1.2, and every candidate is tried in turn — differing
/// only in *what* the signature is checked against: [`webpki::
/// RawPublicKeyEntity`] (RFC 7250) rather than a full `EndEntityCert`,
/// so the certificate's X.509 version is never inspected.
fn raw_spki_verify_tls12(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
    supported_schemes: &rustls::crypto::WebPkiSupportedAlgorithms,
) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    let spki = extract_spki(cert)?;
    let entity = webpki::RawPublicKeyEntity::try_from(&spki).map_err(pki_error)?;
    let candidates = matching_algorithms(supported_schemes, dss.scheme)?;

    let mut last_err = None;
    for alg in candidates {
        match entity.verify_signature(*alg, message, dss.signature()) {
            Ok(()) => return Ok(rustls::client::danger::HandshakeSignatureValid::assertion()),
            // This specific variant means "wrong algorithm for this key
            // type" (e.g. tried RSA against an EC key) — expected when
            // trying several candidates, so keep trying the rest.
            Err(e @ webpki::Error::UnsupportedSignatureAlgorithmForPublicKeyContext(_)) => {
                last_err = Some(e);
            }
            Err(e) => return Err(pki_error(e)),
        }
    }
    match last_err {
        Some(e) => Err(pki_error(e)),
        // `candidates` was non-empty (`matching_algorithms` never returns
        // an empty slice) but every attempt failed via some other error
        // path already returned above — unreachable in practice, but a
        // named error beats a panic if a future algorithm mapping changes.
        None => Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::BadSignature,
        )),
    }
}

/// TLS 1.3 client-certificate signature verification. Mirrors upstream
/// `rustls::crypto::verify_tls13_signature`: rejects a scheme TLS 1.3
/// does not advertise, then tries exactly the first mapped algorithm (TLS
/// 1.3 schemes map 1:1, unlike TLS 1.2's ambiguity) — against the bare
/// SPKI rather than a full `EndEntityCert`, per the module docs.
fn raw_spki_verify_tls13(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
    supported_schemes: &rustls::crypto::WebPkiSupportedAlgorithms,
) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    if !scheme_permitted_in_tls13(dss.scheme) {
        return Err(rustls::PeerMisbehaved::SignedHandshakeWithUnadvertisedSigScheme.into());
    }
    let spki = extract_spki(cert)?;
    let entity = webpki::RawPublicKeyEntity::try_from(&spki).map_err(pki_error)?;
    let alg = matching_algorithms(supported_schemes, dss.scheme)?[0];

    entity
        .verify_signature(alg, message, dss.signature())
        .map_err(pki_error)
        .map(|()| rustls::client::danger::HandshakeSignatureValid::assertion())
}

/// Whether `scheme` is legal in a TLS 1.3 `CertificateVerify` (RFC 8446
/// §4.2.3): reimplemented rather than called, because `rustls::
/// SignatureScheme::supported_in_tls13` is `pub(crate)` to that crate.
/// Named by variant rather than by decomposing the scheme's raw hash/sig
/// byte pair (which upstream does and which stays correct only by
/// matching rustls's private bit-layout): SHA-1, the legacy-ECDSA
/// combination, and every plain RSA-PKCS1 scheme are excluded; ECDSA
/// (secp256/384/521), RSA-PSS, and EdDSA are the whole permitted set.
fn scheme_permitted_in_tls13(scheme: rustls::SignatureScheme) -> bool {
    use rustls::SignatureScheme::*;
    matches!(
        scheme,
        ECDSA_NISTP256_SHA256
            | ECDSA_NISTP384_SHA384
            | ECDSA_NISTP521_SHA512
            | RSA_PSS_SHA256
            | RSA_PSS_SHA384
            | RSA_PSS_SHA512
            | ED25519
            | ED448
    )
}

/// The webpki-supplied algorithms this build's crypto provider considers a
/// match for `scheme` — the same lookup `rustls::crypto::
/// WebPkiSupportedAlgorithms::convert_scheme` does internally, reimplemented
/// here only because that method is private to the `rustls` crate.
fn matching_algorithms(
    supported_schemes: &rustls::crypto::WebPkiSupportedAlgorithms,
    scheme: rustls::SignatureScheme,
) -> Result<&'static [&'static dyn rustls_pki_types::SignatureVerificationAlgorithm], rustls::Error>
{
    supported_schemes
        .mapping
        .iter()
        .find(|(s, _)| *s == scheme)
        .map(|(_, algs)| *algs)
        .ok_or_else(|| rustls::PeerMisbehaved::SignedHandshakeWithUnadvertisedSigScheme.into())
}

/// Extract a certificate's `SubjectPublicKeyInfo` as raw DER bytes (RFC
/// 5280 §4.1), via `x509-parser` rather than webpki: `x509-parser`'s
/// version field is a bare integer it never validates against, so it
/// parses the same minimal client certs webpki's `EndEntityCert` refuses.
fn extract_spki(
    cert: &CertificateDer<'_>,
) -> Result<rustls_pki_types::SubjectPublicKeyInfoDer<'static>, rustls::Error> {
    let (_, parsed) = x509_parser::parse_x509_certificate(cert.as_ref())
        .map_err(|_| rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding))?;
    let raw = parsed.tbs_certificate.subject_pki.raw.to_vec();
    Ok(rustls_pki_types::SubjectPublicKeyInfoDer::from(raw))
}

/// A minimal `webpki::Error` → `rustls::Error` mapping. Deliberately not
/// exhaustive like rustls's own private `pki_error`: [`webpki::
/// RawPublicKeyEntity::verify_signature`] does no chain-building, expiry,
/// or revocation checking — it can only fail with a signature or
/// DER-encoding problem — so those are the only cases handled; anything
/// else collapses to the generic bad-signature error rather than silently
/// mis-reporting a class of failure this path cannot actually produce.
fn pki_error(error: webpki::Error) -> rustls::Error {
    match error {
        webpki::Error::BadDer | webpki::Error::BadDerTime | webpki::Error::TrailingData(_) => {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding)
        }
        webpki::Error::InvalidSignatureForPublicKey => {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature)
        }
        #[allow(
            deprecated,
            reason = "matching webpki's own naming; both variants are real"
        )]
        webpki::Error::UnsupportedSignatureAlgorithm
        | webpki::Error::UnsupportedSignatureAlgorithmForPublicKey
        | webpki::Error::UnsupportedSignatureAlgorithmForPublicKeyContext(_) => {
            rustls::Error::InvalidCertificate(
                rustls::CertificateError::UnsupportedSignatureAlgorithm,
            )
        }
        _ => rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature),
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
