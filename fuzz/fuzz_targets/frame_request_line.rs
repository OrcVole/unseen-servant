//! Property-fuzz for the framing layer (`protocol::request`).
//!
//! Beyond "never panics", this target asserts the layer's full contract on
//! every accepted input, so the fuzzer is also a property checker:
//! an Ok(uri) must be non-empty, within budget, CR/LF-free, and must be
//! exactly the bytes preceding a CRLF at the start of the buffer.

#![no_main]

use libfuzzer_sys::fuzz_target;
use unseen_servant::protocol::request::{frame_request_line, MAX_URI_BYTES};

fuzz_target!(|data: &[u8]| {
    if let Ok(uri) = frame_request_line(data) {
        assert!(!uri.is_empty(), "framing must reject empty URIs");
        assert!(uri.len() <= MAX_URI_BYTES, "framing must enforce the budget");
        assert!(
            !uri.contains(&b'\r') && !uri.contains(&b'\n'),
            "framed URI must be CR/LF-free"
        );
        assert!(data.starts_with(uri), "URI must be a prefix of the buffer");
        assert_eq!(
            &data[uri.len()..uri.len() + 2],
            b"\r\n",
            "URI must be followed by the CRLF terminator"
        );
    }
});
