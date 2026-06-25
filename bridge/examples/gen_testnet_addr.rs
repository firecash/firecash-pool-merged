//! Generate a valid testnet-10 bech32 wallet address.
//!
//! Used by the Phase 1 acceptance smoke (and any future testnet-only
//! validation work) to obtain a `kaspatest:` address that passes the
//! bridge's `kaspa_addresses::Address::try_from` validation in
//! `handle_authorize`.
//!
//! The private key for this address is **not** retained — we don't
//! need to spend the testnet coinbase. The address only has to be a
//! syntactically-valid recipient for `getBlockTemplate` calls.
//!
//! Run with:
//!
//!     cargo run --release --example gen_testnet_addr -p kaspa-stratum-bridge

use std::fs::File;
use std::io::Read;

use kaspa_addresses::{Address, Prefix, Version};

fn main() {
    // 32 bytes of randomness, treated as a Schnorr x-only public key.
    // For Phase 1 smoke purposes any well-formed bech32 testnet address
    // is acceptable; we do not need a key that recovers spendable
    // outputs. Read from /dev/urandom directly to avoid pulling rand
    // into the bridge's dependency set just for this example.
    let mut payload = [0u8; 32];
    File::open("/dev/urandom").expect("open /dev/urandom").read_exact(&mut payload).expect("read /dev/urandom");
    let address = Address::new(Prefix::Testnet, Version::PubKey, &payload);
    println!("{address}");
}
