//! Fuzz target: gemtext parsing (C3, `render::gemtext`).
//!
//! Contract under attack: `parse` must never panic on any UTF-8 input —
//! empty documents, unclosed preformat blocks, pathological heading/list/
//! quote marker runs, BOMs anywhere, huge single lines. Run:
//!
//!   cargo +nightly fuzz run parse_gemtext

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = str::from_utf8(data) {
        let _ = unseen_servant::render::gemtext::parse(s);
    }
});
