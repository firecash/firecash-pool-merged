//! Address redaction for logs and traces.
//!
//! Wallet and treasury addresses are pseudonymous but linkable. The threat
//! model (`docs/threat-model.md`) treats them as semi-sensitive and forbids
//! emitting full addresses into logs or traces; the Phase 7 requirement
//! extends this to never emitting treasury address material into telemetry.
//!
//! This is the single canonical redactor for the workspace: the API layer
//! (`api::redact`) and the runtime binary both route through it so every
//! emitted tag has identical, low-information shape. Response bodies still
//! return the full address the caller already supplied — redaction applies
//! only to the telemetry side (span fields, log lines), never to data the
//! caller is entitled to.

/// Redact an address to a stable, low-information telemetry tag.
///
/// Keeps the network prefix (up to and including the first `:`) plus the last
/// four characters, e.g. `kaspa:…s9jx`. Short or prefixless inputs degrade
/// gracefully. The output never contains enough of the body to reconstruct
/// the address.
#[must_use]
pub fn address(addr: &str) -> String {
    let tail: String = addr
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    match addr.split_once(':') {
        Some((prefix, _)) => format!("{prefix}:…{tail}"),
        None => format!("…{tail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::address;

    #[test]
    fn keeps_prefix_and_last_four() {
        let r = address("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp");
        assert_eq!(r, "kaspa:…jxnp");
    }

    #[test]
    fn exact_tail_is_four_chars() {
        let r = address("kaspa:abcdef1234");
        assert_eq!(r, "kaspa:…1234");
    }

    #[test]
    fn prefixless_input_degrades() {
        assert_eq!(address("abcdef"), "…cdef");
    }

    #[test]
    fn testnet_prefix_is_preserved() {
        let r = address("kaspatest:qr6mqnvwf2e2m6hlxkzje5tqczn67rx2ht3v32t352a82qzs6qrjjleqdsnfl");
        assert_eq!(r, "kaspatest:…snfl");
    }

    #[test]
    fn never_contains_full_body() {
        let full = "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp";
        let r = address(full);
        assert!(!r.contains("qz4j8mu269"));
    }

    #[test]
    fn shorter_than_tail_does_not_panic() {
        assert_eq!(address("ab"), "…ab");
        assert_eq!(address(""), "…");
    }
}
