//! Wallet-tier classification.
//!
//! At block-maturity time the accountant decides what fee tier
//! each contributing wallet sat in. The decision is based on the
//! wallet's on-chain holdings *at the moment of evaluation*:
//!
//! - **Elite**: owns at least one `NACHO` KRC-721 token, **or**
//!   holds ≥ 100M NACHO (`100 * 10^14` = `10^16` base units at the
//!   token's 8-decimal precision).
//! - **Standard**: everything else.
//!
//! ## Why this is a trait
//!
//! The real implementation talks to `krc721.kat.foundation` (Phase
//! 3 M3 lands the HTTP-based `KasplexTierClassifier`). Tests and
//! per-transform unit work want a deterministic stub. M1 ships
//! the trait + the two synchronous stubs only; M3 wires the
//! cached-HTTP impl.
//!
//! ## Caching strategy (deferred to M3)
//!
//! In production the classifier will sit behind a small in-process
//! TTL cache (~5 minutes) so we don't hammer the kasplex endpoint
//! on every block maturity. The trait shape is async-aware
//! specifically so the cached impl can return immediately from the
//! cache without spawning a network task per call.

use async_trait::async_trait;

use katpool_domain::WalletAddress;

use crate::config::WalletTier;

/// Asynchronous wallet-tier lookup.
#[async_trait]
pub trait TierClassifier: Send + Sync + 'static {
    /// Resolve the wallet's tier as of "now". Errors surface to
    /// the caller as `Other` so the consumer can metric them; on
    /// any classifier error the safe fallback is to treat the
    /// wallet as `Standard` (the lower-rebate tier) so the pool
    /// never accidentally over-rebates due to a transient
    /// upstream failure.
    async fn classify(&self, wallet: &WalletAddress) -> Result<WalletTier, ClassifierError>;
}

/// Errors a [`TierClassifier`] can surface. All variants are
/// recoverable from the caller's perspective: on any of them the
/// allocation engine MUST default to `Standard`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClassifierError {
    /// Upstream service (kasplex) unreachable or returned 5xx.
    #[error("upstream unreachable: {0}")]
    Upstream(String),

    /// Upstream responded successfully but the payload didn't
    /// parse as expected.
    #[error("upstream payload malformed: {0}")]
    Malformed(String),
}

/// Test/stub classifier that always returns the same answer.
/// Lives in non-test code so other crates can use it as a
/// deterministic stand-in until the cached HTTP classifier lands.
#[derive(Debug, Clone, Copy)]
pub struct StaticTierClassifier {
    fixed: WalletTier,
}

impl StaticTierClassifier {
    /// Construct a classifier that always returns `tier`.
    #[must_use]
    pub const fn new(tier: WalletTier) -> Self {
        Self { fixed: tier }
    }

    /// Convenience: classifier that returns `Standard`. Use this
    /// as the safe default outside of allocation testing.
    #[must_use]
    pub const fn standard() -> Self {
        Self::new(WalletTier::Standard)
    }
}

#[async_trait]
impl TierClassifier for StaticTierClassifier {
    async fn classify(&self, _wallet: &WalletAddress) -> Result<WalletTier, ClassifierError> {
        Ok(self.fixed)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn sample_wallet() -> WalletAddress {
        WalletAddress::new(
            "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq".to_owned(),
        )
        .expect("valid")
    }

    #[tokio::test]
    async fn static_classifier_returns_configured_tier() {
        let c = StaticTierClassifier::new(WalletTier::Elite);
        assert_eq!(
            c.classify(&sample_wallet()).await.unwrap(),
            WalletTier::Elite
        );

        let s = StaticTierClassifier::standard();
        assert_eq!(
            s.classify(&sample_wallet()).await.unwrap(),
            WalletTier::Standard
        );
    }
}
