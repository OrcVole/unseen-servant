//! Fuzz target: gopher selector-line parsing (v1.1, ADR 0012).
//!
//! Contract under attack: `gopher::parse_selector_line` must never
//! panic, and every rejection must be a typed [`RequestError`] — control
//! bytes and NULs (which a later path-mapping layer would treat
//! differently), invalid UTF-8, tab abuse in and around the search-term
//! separator, missing or doubled terminators, and lines that reach the
//! length cap without one.
//!
//! Also asserts the invariant the menu writer depends on: an accepted
//! selector never contains a tab, CR, or LF, because any of the three
//! would let a request forge menu structure once it is echoed back into
//! a menu line. Run:
//!
//!   cargo +nightly fuzz run parse_gopher_selector

#![no_main]

use libfuzzer_sys::fuzz_target;
use unseen_servant::protocol::gopher;

fuzz_target!(|data: &[u8]| {
    if let Ok((req, consumed)) = gopher::parse_selector_line(data) {
        assert!(consumed <= data.len(), "consumed past the buffer");
        for bad in ['\t', '\r', '\n'] {
            assert!(
                !req.selector.contains(bad),
                "accepted selector carries a structural byte: {:?}",
                req.selector
            );
        }
        // A round-trip through the menu writer must not grow the field
        // count: that is exactly the forgery this scrubbing prevents.
        let line = gopher::MenuLine {
            item: gopher::ItemType::Menu,
            display: req.selector.clone(),
            selector: req.selector.clone(),
            host: "example.org".to_string(),
            port: 70,
        };
        let wire = line.to_wire();
        assert_eq!(wire.matches('\t').count(), 3, "forged a menu field");
        assert_eq!(wire.matches("\r\n").count(), 1, "forged a menu line");
    }
});
