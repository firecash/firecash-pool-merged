//! Typed errors for treasury secret loading.
//!
//! No variant ever carries key material — error messages are safe to log.

use std::path::PathBuf;

/// Failure modes when loading the treasury key.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// `CREDENTIALS_DIRECTORY` is unset — the process was not started
    /// under systemd `LoadCredentialEncrypted=` (see `docs/custody.md` §3.2).
    #[error(
        "CREDENTIALS_DIRECTORY is not set; the treasury key is delivered via \
         systemd LoadCredentialEncrypted="
    )]
    CredentialsDirUnset,

    /// The credential file could not be read.
    #[error("failed to read treasury credential at {path}")]
    Read {
        /// Path that failed to read (never the contents).
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The credential is not exactly 32 bytes of valid lowercase/uppercase
    /// hex. The offending material is deliberately omitted from the message.
    #[error("treasury key material is not 64 hex characters (32 bytes)")]
    InvalidKeyMaterial,

    /// The decoded key is all-zero — not a valid secp256k1 secret and a
    /// common sentinel for an empty/placeholder credential.
    #[error("treasury key is all-zero, which is not a valid secp256k1 secret")]
    AllZeroKey,

    /// `mlock(2)` failed, so the key page cannot be pinned out of swap.
    /// Treated as fatal: custody requires the key never reach swap.
    #[error("mlock(2) failed for the treasury key page")]
    Mlock(#[source] std::io::Error),
}
