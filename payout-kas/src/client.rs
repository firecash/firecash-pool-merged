//! kaspad gRPC adapter behind a narrow, mockable trait.
//!
//! The executor depends only on [`KaspadClient`], so its logic is unit-tested
//! against an in-memory mock; [`GrpcKaspadClient`] is the production binding to
//! a real node and is exercised end-to-end on testnet-10 in M4.8.

use std::sync::Arc;

use async_trait::async_trait;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{Transaction, TransactionId, TransactionOutpoint, UtxoEntry};
use kaspa_grpc_client::GrpcClient;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::notify::mode::NotificationMode;
use kaspa_rpc_core::{RpcTransaction, RpcUtxoEntry};
use katpool_storagemass::TreasuryUtxo;

/// Errors crossing the kaspad boundary.
#[derive(Debug, thiserror::Error)]
pub enum KaspadError {
    /// Any error reported by the kaspad RPC layer (connect, request, decode).
    #[error("kaspad rpc error: {0}")]
    Rpc(String),
}

/// A treasury UTXO as returned by kaspad, with the chain metadata needed for
/// maturity decisions. Convertible into the planner's [`TreasuryUtxo`].
#[derive(Debug, Clone)]
pub struct TreasuryUtxoSnapshot {
    /// Outpoint of the coin.
    pub outpoint: TransactionOutpoint,
    /// Full UTXO entry (amount, script, `block_daa_score`, `is_coinbase`).
    pub entry: UtxoEntry,
}

impl TreasuryUtxoSnapshot {
    /// DAA score of the block that created this coin.
    #[must_use]
    pub const fn block_daa_score(&self) -> u64 {
        self.entry.block_daa_score
    }

    /// Convert into the planner input type.
    #[must_use]
    pub fn into_treasury_utxo(self) -> TreasuryUtxo {
        TreasuryUtxo {
            outpoint: self.outpoint,
            entry: self.entry,
        }
    }
}

/// The kaspad operations the KAS payout executor needs.
///
/// Deliberately minimal: fetch funding coins, read chain height, broadcast,
/// and probe the mempool. Confirmation *policy* lives in [`crate::confirm`],
/// driven by these reads, so it can be tested without a node.
#[async_trait]
pub trait KaspadClient: Send + Sync {
    /// Current virtual DAA score (chain height proxy for maturity math).
    async fn virtual_daa_score(&self) -> Result<u64, KaspadError>;

    /// All UTXOs currently held by `address`.
    async fn treasury_utxos(
        &self,
        address: &Address,
    ) -> Result<Vec<TreasuryUtxoSnapshot>, KaspadError>;

    /// Broadcast a signed transaction; returns the accepted txid.
    async fn submit_transaction(
        &self,
        tx: &Transaction,
        allow_orphan: bool,
    ) -> Result<TransactionId, KaspadError>;

    /// Current top-priority network fee-rate in `sompi/gram`.
    ///
    /// Returns the `get_fee_estimate` priority-bucket feerate (sub-second DAG
    /// inclusion). The planner multiplies it by transaction mass and floors at
    /// the mempool minimum relay fee to size each payout's fee, so a payout is
    /// never rejected for insufficient fee.
    async fn fee_estimate_sompi_per_gram(&self) -> Result<f64, KaspadError>;

    /// Whether `txid` is currently in the mempool (still pending inclusion).
    ///
    /// Implementations MUST treat transport errors as `false`: the executor
    /// only ever advances a payout on a *positive* acceptance signal (a change
    /// UTXO bearing the txid), so a false negative here merely defers the
    /// decision to the next reconcile pass — it never falsely confirms.
    async fn transaction_in_mempool(&self, txid: TransactionId) -> Result<bool, KaspadError>;
}

/// Production [`KaspadClient`] backed by `kaspa-grpc-client`.
pub struct GrpcKaspadClient {
    inner: GrpcClient,
}

impl GrpcKaspadClient {
    /// Connect to a kaspad gRPC endpoint, e.g. `grpc://127.0.0.1:16110`.
    ///
    /// The URL must carry the `grpc://` scheme. Reconnect is enabled so a brief
    /// node restart mid-cycle does not abort an in-flight payout run.
    pub async fn connect(url: impl Into<String>) -> Result<Self, KaspadError> {
        let inner = GrpcClient::connect_with_args(
            NotificationMode::Direct,
            url.into(),
            None,
            true,
            None,
            false,
            Some(500_000),
            Arc::default(),
        )
        .await
        .map_err(|e| KaspadError::Rpc(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Borrow the underlying gRPC client (for callers needing extra RPCs).
    #[must_use]
    pub const fn inner(&self) -> &GrpcClient {
        &self.inner
    }
}

fn rpc_err(e: impl std::fmt::Display) -> KaspadError {
    KaspadError::Rpc(e.to_string())
}

#[async_trait]
impl KaspadClient for GrpcKaspadClient {
    async fn virtual_daa_score(&self) -> Result<u64, KaspadError> {
        Ok(self
            .inner
            .get_block_dag_info()
            .await
            .map_err(rpc_err)?
            .virtual_daa_score)
    }

    async fn treasury_utxos(
        &self,
        address: &Address,
    ) -> Result<Vec<TreasuryUtxoSnapshot>, KaspadError> {
        let entries = self
            .inner
            .get_utxos_by_addresses(vec![address.clone()])
            .await
            .map_err(rpc_err)?;
        Ok(entries
            .into_iter()
            .map(|e| {
                let entry: RpcUtxoEntry = e.utxo_entry;
                TreasuryUtxoSnapshot {
                    outpoint: e.outpoint.into(),
                    entry: entry.into(),
                }
            })
            .collect())
    }

    async fn submit_transaction(
        &self,
        tx: &Transaction,
        allow_orphan: bool,
    ) -> Result<TransactionId, KaspadError> {
        self.inner
            .submit_transaction(RpcTransaction::from(tx), allow_orphan)
            .await
            .map_err(rpc_err)
    }

    async fn fee_estimate_sompi_per_gram(&self) -> Result<f64, KaspadError> {
        Ok(self
            .inner
            .get_fee_estimate()
            .await
            .map_err(rpc_err)?
            .priority_bucket
            .feerate)
    }

    async fn transaction_in_mempool(&self, txid: TransactionId) -> Result<bool, KaspadError> {
        // Ok(entry) ⇒ present. Any error (incl. "not found" and transport
        // faults) ⇒ treat as absent; see the trait contract above.
        Ok(self
            .inner
            .get_mempool_entry(txid, true, false)
            .await
            .is_ok())
    }
}
