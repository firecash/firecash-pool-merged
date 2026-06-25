//! Treasury key-rotation auditor primitive (Phase 8 / Runbook 11).
//!
//! Verifies that a loaded treasury key actually controls the configured
//! treasury address — the continuous, read-only check that catches a botched
//! key rotation, a silent key swap, or a misconfigured address (the in-band
//! complement to ADR-0008's out-of-band custody). **No funds move and nothing
//! is signed for broadcast**: this only derives the schnorr P2PK address from
//! the key's public half and compares it to the expected address.

use kaspa_addresses::{Address, Prefix, Version};
use katpool_secrets::TreasurySecret;
use secp256k1::Keypair;

/// Derive the schnorr P2PK treasury address for `secret` under `prefix`.
///
/// This is the [`Version::PubKey`] address an observer would compute for the
/// key's public half (mainnet / testnet per `prefix`); the secret never leaves
/// this function.
///
/// # Errors
///
/// Returns [`secp256k1::Error`] if the key bytes are not a valid secret key.
pub fn treasury_address_from_secret(
    secret: &TreasurySecret,
    prefix: Prefix,
) -> Result<Address, secp256k1::Error> {
    let keypair = Keypair::from_seckey_slice(secp256k1::SECP256K1, secret.expose_secret())?;
    let xonly = keypair.x_only_public_key().0.serialize();
    Ok(Address::new(prefix, Version::PubKey, &xonly))
}

/// Whether `secret` controls `expected` (derives the same P2PK address).
///
/// A `false` result on a live pool means the running key cannot spend the
/// configured treasury — a page-worthy rotation / compromise / misconfiguration
/// signal.
#[must_use]
pub fn key_controls_address(secret: &TreasurySecret, expected: &Address) -> bool {
    treasury_address_from_secret(secret, expected.prefix).is_ok_and(|derived| &derived == expected)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // A valid, non-zero secp256k1 secret key (32 bytes of 0x11).
    const SAMPLE_KEY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    #[test]
    fn derived_address_is_controlled_by_its_own_key() {
        let secret = katpool_secrets::from_hex(SAMPLE_KEY_HEX).expect("valid key");
        let addr = treasury_address_from_secret(&secret, Prefix::Testnet).expect("derive");
        assert_eq!(addr.version, Version::PubKey);
        assert_eq!(addr.prefix, Prefix::Testnet);
        assert!(
            key_controls_address(&secret, &addr),
            "key must control its own address"
        );
    }

    #[test]
    fn rejects_a_different_pubkey_address() {
        let secret = katpool_secrets::from_hex(SAMPLE_KEY_HEX).expect("valid key");
        // A P2PK address for a different public key the secret does not control —
        // i.e. the configured treasury address no longer matches the loaded key.
        let other = Address::new(Prefix::Testnet, Version::PubKey, &[0x22u8; 32]);
        assert!(
            !key_controls_address(&secret, &other),
            "must reject an address it cannot spend"
        );
    }
}
