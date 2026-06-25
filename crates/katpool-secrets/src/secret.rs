//! The in-memory treasury secret: `mlock`ed, zeroized on drop, opaque.

use std::ffi::c_void;

use secrecy::{ExposeSecret, SecretBox};
use zeroize::Zeroize;

use crate::error::SecretError;

/// Length of a Kaspa (secp256k1) private key in bytes.
pub const KEY_LEN: usize = 32;

/// A 32-byte treasury private key held under custody discipline
/// (`docs/custody.md` §3.3, ADR-0008):
///
/// - backed by [`secrecy::SecretBox`] — zeroized on drop, **no** `Debug`,
///   `Display`, `Clone`, or `Serialize` impl, so the bytes cannot leak into
///   logs, error messages, or wire formats;
/// - the backing heap page is `mlock(2)`ed at construction so the key is
///   never written to swap, and `munlock(2)`ed after the final zeroize.
///
/// Access the bytes only at the signing boundary via [`Self::expose_secret`].
pub struct TreasurySecret {
    // Field order matters for Drop: `inner` is dropped first (zeroizing the
    // bytes while the page is still locked), then the struct's own Drop has
    // already run munlock — see the `Drop` impl below.
    inner: SecretBox<[u8; KEY_LEN]>,
    /// Address of the locked region, kept as `usize` so the type stays
    /// `Send + Sync` (no raw pointer field).
    locked_addr: usize,
}

impl TreasurySecret {
    /// Construct from raw key bytes, locking the page and rejecting the
    /// all-zero key. The caller's `bytes` are zeroized before returning.
    #[allow(unsafe_code)]
    pub(crate) fn from_bytes(mut bytes: [u8; KEY_LEN]) -> Result<Self, SecretError> {
        if bytes.iter().all(|&b| b == 0) {
            bytes.zeroize();
            return Err(SecretError::AllZeroKey);
        }

        let inner = SecretBox::new(Box::new(bytes));
        bytes.zeroize();

        let locked_addr = inner.expose_secret().as_ptr() as usize;
        // SAFETY: `locked_addr` and `KEY_LEN` describe the live, boxed
        // `[u8; KEY_LEN]` owned by `inner`; locking pins the containing page
        // so the secret cannot be paged to swap. The address is stable for
        // the box's lifetime (moving `inner` moves only the box pointer).
        let rc = unsafe { libc::mlock(locked_addr as *const c_void, KEY_LEN) };
        if rc != 0 {
            // `inner` drops here, zeroizing the bytes; nothing is locked.
            return Err(SecretError::Mlock(std::io::Error::last_os_error()));
        }

        tracing::debug!(bytes = KEY_LEN, "treasury secret loaded, mlocked");
        Ok(Self { inner, locked_addr })
    }

    /// Borrow the raw key bytes for signing. This is the only accessor;
    /// callers must not copy, log, or persist the returned slice.
    #[must_use]
    pub fn expose_secret(&self) -> &[u8; KEY_LEN] {
        self.inner.expose_secret()
    }
}

impl Drop for TreasurySecret {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // Wipe while the page is still locked, then unlock. `inner`'s own
        // Drop will run after this body and zeroize again (a harmless no-op),
        // and only then is the box freed — so the address stays valid here.
        self.inner.zeroize();
        // SAFETY: `locked_addr`/`KEY_LEN` match the region locked in
        // `from_bytes`, and `inner`'s box is not freed until after this body.
        unsafe {
            libc::munlock(self.locked_addr as *const c_void, KEY_LEN);
        }
    }
}
