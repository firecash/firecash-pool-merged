//! Stratum JSON-RPC parser fuzz harness.
//!
//! Drives `kaspa_stratum_bridge::jsonrpc_event::unmarshal_event` over
//! arbitrary byte slices. The contract under test is the property
//! "parser must never panic, regardless of input". A panic is a
//! denial-of-service surface: an unprivileged TCP peer should never be
//! able to crash the bridge by sending crafted bytes.
//!
//! Acceptance target for Phase 1: 1M+ iterations with zero panics.
//!
//! Build/run:
//!
//! ```bash
//! cd bridge/fuzz
//! cargo +nightly fuzz run stratum_parser
//! ```
//!
//! With a fixed iteration budget (for CI / scheduled runs):
//!
//! ```bash
//! cd bridge/fuzz
//! cargo +nightly fuzz run stratum_parser -- -runs=1000000 -max_total_time=300
//! ```

#![no_main]

use kaspa_stratum_bridge::jsonrpc_event::unmarshal_event;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // libFuzzer feeds arbitrary bytes; unmarshal_event takes &str, so
    // we route invalid UTF-8 through the lossy path which is what the
    // real TCP read loop does upstream (see
    // `stratum_listener::spawn_client_listener`, which calls
    // `String::from_utf8_lossy(&data)`).
    let s = String::from_utf8_lossy(data);
    let _ = unmarshal_event(&s);
});
