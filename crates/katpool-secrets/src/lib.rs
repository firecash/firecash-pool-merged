//! Treasury secret material handling.
//!
//! Implements the user-confirmed sops/age + OS-level isolation custody model
//! documented in ADR-008 and `docs/custody.md`:
//!
//! - keys at rest: sops-encrypted with age, mode 0600, owned by katpool uid
//! - keys in memory: [`TreasurySecret`] = `secrecy::SecretBox<[u8; 32]>`,
//!   zeroized on drop, `mlock`ed; no `Debug`/`Display`/`Clone`/`Serialize`
//! - keys at boot: decrypted by systemd `LoadCredentialEncrypted=` into a
//!   tmpfs credential, read via [`load_from_systemd_credential`]
//! - swap: disabled at OS level (defence in depth alongside `mlock`)
//!
//! This crate is the single sanctioned home for `unsafe` in the workspace
//! (ADR-0008): the `mlock(2)`/`munlock(2)` FFI calls behind [`TreasurySecret`].

#![cfg_attr(not(test), warn(missing_docs))]

mod error;
mod loader;
mod secret;

pub use error::SecretError;
pub use loader::{TREASURY_CREDENTIAL_ID, from_hex, load_from_path, load_from_systemd_credential};
pub use secret::{KEY_LEN, TreasurySecret};

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, unsafe_code)]

    use super::{
        KEY_LEN, SecretError, TreasurySecret, from_hex, load_from_path,
        load_from_systemd_credential,
    };
    use std::io::Write as _;

    const SAMPLE_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn sample_bytes() -> [u8; KEY_LEN] {
        [0x11_u8; KEY_LEN]
    }

    #[test]
    fn from_hex_round_trips_bytes() {
        let secret = from_hex(SAMPLE_HEX).expect("valid hex");
        assert_eq!(secret.expose_secret(), &sample_bytes());
    }

    #[test]
    fn from_hex_tolerates_trailing_newline() {
        let secret = from_hex(&format!("{SAMPLE_HEX}\n")).expect("trailing newline ok");
        assert_eq!(secret.expose_secret(), &sample_bytes());
    }

    #[test]
    fn from_hex_rejects_wrong_length() {
        let err = from_hex("1111").err().expect("too short");
        assert!(matches!(err, SecretError::InvalidKeyMaterial));
    }

    #[test]
    fn from_hex_rejects_non_hex() {
        let bad = "z".repeat(64);
        let err = from_hex(&bad).err().expect("non-hex");
        assert!(matches!(err, SecretError::InvalidKeyMaterial));
    }

    #[test]
    fn from_hex_rejects_all_zero_key() {
        let zero = "0".repeat(64);
        let err = from_hex(&zero).err().expect("all-zero invalid");
        assert!(matches!(err, SecretError::AllZeroKey));
    }

    #[test]
    fn load_from_path_reads_credential_file() {
        let mut file = tempfile::NamedTempFile::new().expect("temp file");
        writeln!(file, "{SAMPLE_HEX}").expect("write");
        let secret = load_from_path(file.path()).expect("load");
        assert_eq!(secret.expose_secret(), &sample_bytes());
    }

    #[test]
    fn load_from_path_missing_file_is_read_error() {
        let err = load_from_path("/nonexistent/treasury-key")
            .err()
            .expect("missing");
        assert!(matches!(err, SecretError::Read { .. }));
    }

    #[test]
    fn treasury_secret_is_send_and_sync() {
        const fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TreasurySecret>();
    }

    // Both `CREDENTIALS_DIRECTORY` cases live in one test: cargo runs tests in
    // parallel threads within a process, so this is the only place that mutates
    // the process-global variable, avoiding a data race with any other test.
    #[test]
    fn systemd_credential_loads_and_reports_unset_dir() {
        let prev = std::env::var_os("CREDENTIALS_DIRECTORY");

        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("treasury-key"), SAMPLE_HEX).expect("write credential");

        // SAFETY: only this test touches CREDENTIALS_DIRECTORY (see comment).
        unsafe { std::env::set_var("CREDENTIALS_DIRECTORY", dir.path()) };
        let secret = load_from_systemd_credential("treasury-key").expect("load credential");
        assert_eq!(secret.expose_secret(), &sample_bytes());

        // SAFETY: see above.
        unsafe { std::env::remove_var("CREDENTIALS_DIRECTORY") };
        let err = load_from_systemd_credential("treasury-key")
            .err()
            .expect("dir unset");
        assert!(matches!(err, SecretError::CredentialsDirUnset));

        // SAFETY: restore prior state for any later-running test.
        match prev {
            Some(value) => unsafe { std::env::set_var("CREDENTIALS_DIRECTORY", value) },
            None => unsafe { std::env::remove_var("CREDENTIALS_DIRECTORY") },
        }
    }
}
