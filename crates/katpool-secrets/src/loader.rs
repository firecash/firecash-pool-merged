//! Loading the treasury key from the systemd credentials tmpfs.
//!
//! At boot, systemd `LoadCredentialEncrypted=` decrypts the sops/age file
//! (`docs/custody.md` §3.2, ADR-0003) and exposes the plaintext key under
//! `$CREDENTIALS_DIRECTORY/<name>`, readable only by the katpool process.
//! This crate never runs sops or age itself — it reads the already-decrypted
//! bytes from that tmpfs and immediately pins + wraps them.
//!
//! The credential payload is the private key as 64 hex characters (matching
//! the legacy `TREASURY_PRIVATE_KEY`), optionally with trailing whitespace.

use std::path::Path;

use zeroize::Zeroize;

use crate::error::SecretError;
use crate::secret::{KEY_LEN, TreasurySecret};

/// Default systemd credential id for the treasury key, matching
/// `LoadCredentialEncrypted=treasury-key:...` in the hardening unit.
pub const TREASURY_CREDENTIAL_ID: &str = "treasury-key";

/// Load the treasury key from `$CREDENTIALS_DIRECTORY/<name>`.
///
/// This is the production path: the process must run under a systemd unit
/// with `LoadCredentialEncrypted=`. Returns [`SecretError::CredentialsDirUnset`]
/// when the environment variable is absent.
pub fn load_from_systemd_credential(name: &str) -> Result<TreasurySecret, SecretError> {
    let dir = std::env::var_os("CREDENTIALS_DIRECTORY").ok_or(SecretError::CredentialsDirUnset)?;
    load_from_path(Path::new(&dir).join(name))
}

/// Load the treasury key from an explicit file path (the credential file, or
/// a developer/test fixture). The file content is zeroized after parsing.
pub fn load_from_path(path: impl AsRef<Path>) -> Result<TreasurySecret, SecretError> {
    let path = path.as_ref();
    let mut raw = std::fs::read_to_string(path).map_err(|source| SecretError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let result = from_hex(&raw);
    raw.zeroize();
    result
}

/// Parse a 64-character hex string into a locked [`TreasurySecret`].
///
/// Surrounding whitespace (e.g. a trailing newline) is ignored.
pub fn from_hex(hex_str: &str) -> Result<TreasurySecret, SecretError> {
    let mut bytes = [0_u8; KEY_LEN];
    // `decode_to_slice` requires `hex_str.len() == 2 * KEY_LEN`, so this also
    // enforces the exact length without leaking the material into the error.
    hex::decode_to_slice(hex_str.trim(), &mut bytes)
        .map_err(|_| SecretError::InvalidKeyMaterial)?;
    let result = TreasurySecret::from_bytes(bytes);
    bytes.zeroize();
    result
}
