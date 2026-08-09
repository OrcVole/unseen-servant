//! Fuzz target: URI validation (protocol layer 2).
//!
//! Contract under attack: `validate_uri` must never panic, and every
//! rejection must be a typed error — arbitrary bytes, torn percent-
//! encodings, scheme confusion, IPv6 bracket abuse. Run:
//!
//!   cargo +nightly fuzz run validate_uri

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = unseen_servant::protocol::uri::validate_uri(data);
});
