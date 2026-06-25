//! Address redaction for logs and traces.
//!
//! Wallet addresses are pseudonymous but linkable. The threat model
//! (`docs/threat-model.md`) treats them as semi-sensitive and forbids
//! emitting full addresses into logs. Responses still return the full
//! address the caller already supplied — redaction applies only to the
//! telemetry side (span fields, error logs), never to the response body.
//!
//! The canonical implementation lives in [`katpool_domain::redact`] so the
//! API and the runtime binary emit identically-shaped tags; this module
//! re-exports it to keep call sites in `api` ergonomic.

pub use katpool_domain::redact::address;

#[cfg(test)]
mod tests {
    use super::address;

    #[test]
    fn keeps_prefix_and_last_four() {
        let r = address("kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp");
        assert_eq!(r, "kaspa:…jxnp");
    }

    #[test]
    fn never_contains_full_body() {
        let full = "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp";
        assert!(!address(full).contains("qz4j8mu269"));
    }
}
