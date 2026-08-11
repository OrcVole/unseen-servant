//! Keys and certificates (ADR 0003) — the **sole holder of private-key
//! material** in this crate (ADR 0002).
//!
//! Under TOFU the keypair *is* the capsule's identity: clients pin the
//! certificate fingerprint, and rotation is a user-visible trust event. So:
//!
//! - **Auto-generate on first run**: ECDSA P-256, self-signed, notAfter
//!   4096-01-01 (Agate's convention — expiry churn adds nothing under TOFU),
//!   key file mode 0600, PEM on disk.
//! - **Per-hostname subdirectories**: `<certs>/<hostname>/{cert,key}.pem`.
//!   DER slots (`cert.der`/`key.der`) are accepted on read for Agate
//!   migrants.
//! - **Never silently regenerate**: generation happens only when a
//!   hostname's slot is empty. Corrupt or half-present material is a loud
//!   startup error, never a regeneration.
//! - **Hostname-change detection**: a `generated-by-usv` marker is written
//!   beside every keypair usv mints. When a configured hostname has no slot
//!   but marked slots for *other* hostnames exist (the Cloudron clone/move
//!   signature), usv logs exactly what it concluded, mints a fresh keypair
//!   for the new name, and touches nothing else.
//!
//! Key material enters [`rustls::sign::CertifiedKey`] here and is exposed
//! only through the SNI resolver ([`IdentityStore`]); no other module can
//! name a private-key type. That module boundary is ADR 0002's replacement
//! for gmid's crypto process.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls::crypto::ring::sign::any_supported_type;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

/// Why identity setup failed. Every variant is a startup error with an
/// actionable message; none of them is ever answered by regenerating.
#[derive(Debug)]
pub enum IdentityError {
    /// Filesystem trouble (creating the certs dir, reading/writing a slot).
    Io(PathBuf, std::io::Error),
    /// A slot has one half of a keypair but not the other. Regenerating
    /// would silently change identity; the operator must decide.
    HalfPresent {
        /// The file that exists.
        present: PathBuf,
        /// The file that is missing.
        missing: PathBuf,
    },
    /// Certificate or key material exists but cannot be parsed/used.
    Corrupt(PathBuf, String),
    /// Certificate generation itself failed (rcgen).
    Generation(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentityError::Io(p, e) => write!(f, "identity: {}: {e}", p.display()),
            IdentityError::HalfPresent { present, missing } => write!(
                f,
                "identity: {} exists but {} is missing. usv never regenerates key material \
                 (ADR 0003): restore the missing file from backup, or move the orphan aside \
                 to mint a fresh identity for this hostname",
                present.display(),
                missing.display()
            ),
            IdentityError::Corrupt(p, why) => write!(
                f,
                "identity: {} is unreadable as certificate/key material ({why}). usv never \
                 regenerates over existing material (ADR 0003): restore it from backup, or \
                 move it aside to mint a fresh identity",
                p.display()
            ),
            IdentityError::Generation(e) => {
                write!(f, "identity: certificate generation failed: {e}")
            }
        }
    }
}

impl std::error::Error for IdentityError {}

/// Marker file name written beside keypairs usv generated itself. Its
/// absence marks user-supplied material (never touched, never reasoned
/// about); its presence powers hostname-change detection.
const MINTED_MARKER: &str = "generated-by-usv";

/// One hostname's loaded identity.
struct HostIdentity {
    /// Lowercase hostname this identity answers SNI for.
    name: String,
    key: Arc<CertifiedKey>,
}

/// The SNI resolver handed to rustls: hostname → certificate, exact match,
/// first configured host as the no-SNI/unknown-SNI default (recon guidance
/// §4: "if a ClientHello has no SNI, serve the configured default host or
/// refuse, but never crash").
pub struct IdentityStore {
    hosts: Vec<HostIdentity>,
}

impl std::fmt::Debug for IdentityStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No key material in Debug output, ever.
        f.debug_struct("IdentityStore")
            .field(
                "hosts",
                &self.hosts.iter().map(|h| &h.name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl IdentityStore {
    /// Load or mint identities for every configured hostname.
    ///
    /// `certs_dir` is `${state_dir}/certs` (ADR 0003). Hostnames must be
    /// the validated, lowercased names from config.
    pub fn open(certs_dir: &Path, hostnames: &[String]) -> Result<IdentityStore, IdentityError> {
        std::fs::create_dir_all(certs_dir)
            .map_err(|e| IdentityError::Io(certs_dir.to_path_buf(), e))?;
        let mut hosts = Vec::with_capacity(hostnames.len());
        for name in hostnames {
            hosts.push(HostIdentity {
                name: name.clone(),
                key: load_or_mint(certs_dir, name)?,
            });
        }
        Ok(IdentityStore { hosts })
    }

    /// The hostnames this store can answer for (logging/tests).
    pub fn hostnames(&self) -> impl Iterator<Item = &str> {
        self.hosts.iter().map(|h| h.name.as_str())
    }

    /// The SHA-256 fingerprint (lowercase hex) of `hostname`'s leaf
    /// certificate, `None` if `hostname` isn't one this store loaded.
    ///
    /// Same hash, same encoding as [`crate::server`]'s client-certificate
    /// fingerprint (one convention for "what a fingerprint looks like" on
    /// both sides of a TOFU pin) — this is the *server's own* identity, the
    /// value an operator publishes out-of-band for a client to verify on
    /// first connection, or that `usv fingerprint` (C5) prints for them.
    pub fn fingerprint(&self, hostname: &str) -> Option<String> {
        self.hosts
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(hostname))
            .and_then(|h| h.key.cert.first())
            .map(|leaf| hex_sha256(leaf.as_ref()))
    }

    /// Every configured hostname paired with its fingerprint, in load
    /// order — for `usv fingerprint` to print without one lookup per host.
    pub fn fingerprints(&self) -> impl Iterator<Item = (&str, String)> {
        self.hosts.iter().filter_map(|h| {
            h.key
                .cert
                .first()
                .map(|leaf| (h.name.as_str(), hex_sha256(leaf.as_ref())))
        })
    }
}

/// Lowercase-hex SHA-256, the one fingerprint format this crate uses
/// anywhere a certificate is identified by digest.
fn hex_sha256(der: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl ResolvesServerCert for IdentityStore {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let wanted = client_hello.server_name();
        match wanted {
            Some(name) => self
                .hosts
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .or(self.hosts.first())
                .map(|h| h.key.clone()),
            // No SNI (e.g. connection by literal IP): default host. The
            // request's own authority check still applies afterwards.
            None => self.hosts.first().map(|h| h.key.clone()),
        }
    }
}

/// Load a hostname's keypair, or mint one if — and only if — its slot is
/// entirely empty.
fn load_or_mint(certs_dir: &Path, hostname: &str) -> Result<Arc<CertifiedKey>, IdentityError> {
    let dir = certs_dir.join(hostname);
    let pem_cert = dir.join("cert.pem");
    let pem_key = dir.join("key.pem");
    let der_cert = dir.join("cert.der");
    let der_key = dir.join("key.der");

    let cert_path = [&pem_cert, &der_cert].into_iter().find(|p| p.exists());
    let key_path = [&pem_key, &der_key].into_iter().find(|p| p.exists());

    match (cert_path, key_path) {
        (Some(cert), Some(key)) => {
            tracing::info!(hostname, cert = %cert.display(), "identity loaded from disk");
            load_keypair(cert, key)
        }
        (Some(cert), None) => Err(IdentityError::HalfPresent {
            present: cert.clone(),
            missing: pem_key,
        }),
        (None, Some(key)) => Err(IdentityError::HalfPresent {
            present: key.clone(),
            missing: pem_cert,
        }),
        (None, None) => {
            detect_hostname_change(certs_dir, hostname);
            mint(&dir, hostname)?;
            load_keypair(&pem_cert, &pem_key)
        }
    }
}

/// The Cloudron clone/move signature (ADR 0003): the configured hostname has
/// no identity, but usv-minted identities for other hostnames exist. Purely
/// informational — the response is always "mint fresh for the new name,
/// touch nothing else" — but the operator deserves the loud, exact story.
fn detect_hostname_change(certs_dir: &Path, hostname: &str) {
    let Ok(entries) = std::fs::read_dir(certs_dir) else {
        return;
    };
    let minted_others: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join(MINTED_MARKER).exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.eq_ignore_ascii_case(hostname))
        .collect();
    if !minted_others.is_empty() {
        tracing::warn!(
            new_hostname = hostname,
            previous = ?minted_others,
            "hostname change detected: this looks like a clone/move (previously minted \
             identities exist for other hostnames). Minting a FRESH keypair for the new \
             hostname; the old keypairs are left untouched on disk and are still valid \
             for their own hostnames (ADR 0003 — clients pin per-host, keys are never \
             reused across hostnames and never deleted)"
        );
    }
}

/// Mint a fresh self-signed identity into `dir`. Only ever called on an
/// empty slot.
fn mint(dir: &Path, hostname: &str) -> Result<(), IdentityError> {
    std::fs::create_dir_all(dir).map_err(|e| IdentityError::Io(dir.to_path_buf(), e))?;

    let mut params = rcgen::CertificateParams::new(vec![hostname.to_string()])
        .map_err(|e| IdentityError::Generation(e.to_string()))?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, hostname);
    // Backdated a day so a freshly minted cert is valid even against peers
    // with modest clock skew; far-future expiry is the TOFU convention
    // (docs/internal/recon/prior-art.md §1 — Agate).
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    #[expect(
        clippy::unwrap_used,
        reason = "4096-01-01 is a valid date; const-evaluable"
    )]
    let not_after = time::Date::from_calendar_date(4096, time::Month::January, 1)
        .unwrap()
        .midnight()
        .assume_utc();
    params.not_after = not_after;

    let key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
        .map_err(|e| IdentityError::Generation(e.to_string()))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| IdentityError::Generation(e.to_string()))?;

    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    write_new(&cert_path, cert.pem().as_bytes(), 0o644)?;
    write_new(&key_path, key_pair.serialize_pem().as_bytes(), 0o600)?;
    let marker = format!(
        "minted by usv {} — do not edit; presence of this file marks the keypair as \
         auto-generated (ADR 0003 hostname-change detection)\n",
        env!("CARGO_PKG_VERSION")
    );
    write_new(&dir.join(MINTED_MARKER), marker.as_bytes(), 0o644)?;

    tracing::info!(
        hostname,
        cert = %cert_path.display(),
        "minted new self-signed identity (ECDSA P-256, expires 4096-01-01). Clients \
         will pin this certificate on first use; keep the certs directory in backups"
    );
    Ok(())
}

/// Create-new + write + chmod, refusing to overwrite anything.
fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), IdentityError> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let mut f = opts
        .open(path)
        .map_err(|e| IdentityError::Io(path.to_path_buf(), e))?;
    f.write_all(bytes)
        .map_err(|e| IdentityError::Io(path.to_path_buf(), e))?;
    f.sync_all()
        .map_err(|e| IdentityError::Io(path.to_path_buf(), e))?;
    Ok(())
}

/// Parse cert + key files (PEM preferred, raw DER accepted) into a rustls
/// [`CertifiedKey`].
fn load_keypair(cert_path: &Path, key_path: &Path) -> Result<Arc<CertifiedKey>, IdentityError> {
    let cert_bytes =
        std::fs::read(cert_path).map_err(|e| IdentityError::Io(cert_path.to_path_buf(), e))?;
    let key_bytes =
        std::fs::read(key_path).map_err(|e| IdentityError::Io(key_path.to_path_buf(), e))?;

    let certs: Vec<CertificateDer<'static>> = if looks_pem(&cert_bytes) {
        let parsed: Result<Vec<_>, _> = CertificateDer::pem_slice_iter(&cert_bytes).collect();
        parsed.map_err(|e| IdentityError::Corrupt(cert_path.to_path_buf(), format!("{e:?}")))?
    } else {
        vec![CertificateDer::from(cert_bytes).into_owned()]
    };
    if certs.is_empty() {
        return Err(IdentityError::Corrupt(
            cert_path.to_path_buf(),
            "no certificates found in file".into(),
        ));
    }

    let key: PrivateKeyDer<'static> = if looks_pem(&key_bytes) {
        PrivateKeyDer::from_pem_slice(&key_bytes)
            .map_err(|e| IdentityError::Corrupt(key_path.to_path_buf(), format!("{e:?}")))?
    } else {
        PrivateKeyDer::try_from(key_bytes)
            .map_err(|e| IdentityError::Corrupt(key_path.to_path_buf(), e.to_string()))?
            .clone_key()
    };

    let signing_key = any_supported_type(&key)
        .map_err(|e| IdentityError::Corrupt(key_path.to_path_buf(), e.to_string()))?;
    Ok(Arc::new(CertifiedKey::new(certs, signing_key)))
}

/// PEM files are ASCII armored; DER is binary ASN.1 starting 0x30.
fn looks_pem(bytes: &[u8]) -> bool {
    bytes.starts_with(b"-----")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("usv-identity-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn first_run_mints_and_second_run_loads_identically() {
        let dir = tmpdir("mint");
        let hosts = vec!["localhost".to_string()];
        let store = IdentityStore::open(&dir, &hosts).expect("first run mints");
        assert_eq!(store.hostnames().collect::<Vec<_>>(), vec!["localhost"]);

        let cert_before =
            std::fs::read(dir.join("localhost/cert.pem")).expect("cert exists after mint");
        assert!(dir.join("localhost/key.pem").exists());
        assert!(dir.join("localhost").join(MINTED_MARKER).exists());

        let _again = IdentityStore::open(&dir, &hosts).expect("second run loads");
        let cert_after = std::fs::read(dir.join("localhost/cert.pem")).expect("still there");
        assert_eq!(
            cert_before, cert_after,
            "loading must never rewrite identity"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprint_is_64_lowercase_hex_and_stable_across_loads() {
        let dir = tmpdir("fingerprint");
        let hosts = vec!["localhost".to_string()];
        let first = IdentityStore::open(&dir, &hosts).expect("mints");
        let fp = first.fingerprint("localhost").expect("host is configured");
        assert_eq!(fp.len(), 64, "SHA-256 hex is 64 chars");
        assert!(
            fp.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );

        let second = IdentityStore::open(&dir, &hosts).expect("loads the same identity");
        assert_eq!(
            second.fingerprint("localhost"),
            Some(fp),
            "loading must report the same fingerprint as minting"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprint_of_an_unconfigured_host_is_none() {
        let dir = tmpdir("fingerprint-unknown");
        let store = IdentityStore::open(&dir, &["localhost".to_string()]).expect("mints");
        assert_eq!(store.fingerprint("nope.example"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprint_lookup_is_case_insensitive() {
        let dir = tmpdir("fingerprint-case");
        let store = IdentityStore::open(&dir, &["localhost".to_string()]).expect("mints");
        assert_eq!(
            store.fingerprint("localhost"),
            store.fingerprint("LOCALHOST")
        );
    }

    #[test]
    fn fingerprints_lists_every_host_in_order() {
        let dir = tmpdir("fingerprints-all");
        let hosts = vec!["a.example".to_string(), "b.example".to_string()];
        let store = IdentityStore::open(&dir, &hosts).expect("mints both");
        let listed: Vec<&str> = store.fingerprints().map(|(name, _)| name).collect();
        assert_eq!(listed, vec!["a.example", "b.example"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_different_hosts_have_different_fingerprints() {
        let dir = tmpdir("fingerprint-distinct");
        let hosts = vec!["a.example".to_string(), "b.example".to_string()];
        let store = IdentityStore::open(&dir, &hosts).expect("mints both");
        assert_ne!(
            store.fingerprint("a.example"),
            store.fingerprint("b.example")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn minted_key_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmpdir("mode");
        IdentityStore::open(&dir, &["localhost".to_string()]).expect("mints");
        let mode = std::fs::metadata(dir.join("localhost/key.pem"))
            .expect("key exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "private key must be 0600");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn half_present_slot_is_a_loud_error_not_a_regeneration() {
        let dir = tmpdir("half");
        let host_dir = dir.join("capsule.example");
        std::fs::create_dir_all(&host_dir).expect("mkdir");
        std::fs::write(host_dir.join("cert.pem"), "-----BEGIN CERTIFICATE-----\n")
            .expect("write orphan cert");
        let err = IdentityStore::open(&dir, &["capsule.example".to_string()]).unwrap_err();
        assert!(matches!(err, IdentityError::HalfPresent { .. }), "{err}");
        assert!(err.to_string().contains("never regenerates"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_material_is_a_loud_error_not_a_regeneration() {
        let dir = tmpdir("corrupt");
        let host_dir = dir.join("capsule.example");
        std::fs::create_dir_all(&host_dir).expect("mkdir");
        std::fs::write(
            host_dir.join("cert.pem"),
            "-----BEGIN CERTIFICATE-----\ngarbage",
        )
        .expect("write");
        std::fs::write(
            host_dir.join("key.pem"),
            "-----BEGIN PRIVATE KEY-----\ngarbage",
        )
        .expect("write");
        let before = std::fs::read(host_dir.join("cert.pem")).expect("read");
        let err = IdentityStore::open(&dir, &["capsule.example".to_string()]).unwrap_err();
        assert!(matches!(err, IdentityError::Corrupt(..)), "{err}");
        let after = std::fs::read(host_dir.join("cert.pem")).expect("read");
        assert_eq!(before, after, "corrupt material must be left untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_hostname_mints_without_touching_the_old_one() {
        let dir = tmpdir("clone");
        IdentityStore::open(&dir, &["old.example".to_string()]).expect("mint old");
        let old_cert = std::fs::read(dir.join("old.example/cert.pem")).expect("read");

        // The clone/move signature: config now says new.example only.
        let store = IdentityStore::open(&dir, &["new.example".to_string()]).expect("mint new");
        assert_eq!(store.hostnames().collect::<Vec<_>>(), vec!["new.example"]);
        assert!(dir.join("new.example/cert.pem").exists());
        let old_after = std::fs::read(dir.join("old.example/cert.pem")).expect("still there");
        assert_eq!(old_cert, old_after, "old identity must survive untouched");
        assert_ne!(
            std::fs::read(dir.join("new.example/key.pem")).expect("new key"),
            std::fs::read(dir.join("old.example/key.pem")).expect("old key"),
            "keys are never reused across hostnames"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multi_host_store_loads_every_hostname() {
        let dir = tmpdir("multi");
        let hosts = vec!["a.example".to_string(), "b.example".to_string()];
        let store = IdentityStore::open(&dir, &hosts).expect("mints both");
        assert_eq!(store.hostnames().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
