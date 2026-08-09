//! Fuzz target: Titan request-line parsing (protocol layer 2, C4).
//!
//! Contract under attack: `titan::parse` must never panic, and every
//! rejection must be a typed [`TitanError`] — torn percent-encodings in
//! parameter values, scheme confusion, semicolon/equals abuse in the
//! parameter block, oversized/negative/non-decimal `size`, IPv6 bracket
//! abuse in the authority (shared with the URI parser). Run:
//!
//!   cargo +nightly fuzz run parse_titan

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = unseen_servant::protocol::titan::parse(data);
});
