//! Fuzz target: configuration parsing (ADR 0007).
//!
//! Contract under attack: the full TOML → validated-Config pipeline must
//! never panic on any input text — malformed TOML, hostile key names,
//! reserved sections, absurd values. Run:
//!
//!   cargo +nightly fuzz run config_parse

#![no_main]

use libfuzzer_sys::fuzz_target;
use unseen_servant::config::{Config, EnvOverrides};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = str::from_utf8(data) {
        let _ = Config::from_toml_str(text, &EnvOverrides::default());
    }
});
