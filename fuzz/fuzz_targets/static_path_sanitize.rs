//! Fuzz target: static-file path sanitization (C2 traversal defense).
//!
//! Contract under attack: `handler::static_file::fuzz_sanitize` must never
//! panic on any input, and the sanitized path (when one is produced) must
//! never contain a `..` component — the assertion lives inside the
//! function itself so a violation fails the fuzz run immediately. Run:
//!
//!   cargo +nightly fuzz run static_path_sanitize

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = str::from_utf8(data) {
        unseen_servant::handler::static_file::fuzz_sanitize(s);
    }
});
