# Runbook 07 — Storage-mass rejection burst

## Symptom

Alert `StorageMassRejectionBurst` fires when
`rate(storage_mass_rejected_total[10m]) > 3`. Pool transactions are
being rejected by the network for exceeding mass limits despite
our mass-aware batcher's pre-flight checks.

If our `katpool-storagemass` calculator says a tx is valid but the
network says no, **the calculator has drifted from the consensus
rules**. That's a P0 bug; treat as SEV-2 immediately and SEV-1 if
it's blocking active payouts.

Reference: [`docs/kips.md`](../kips.md), KIP-9, KIP-13.

## Confirm

```bash
ssh prod-vps journalctl -u katpool --since '15 min ago' | \
  grep -E 'Storage mass exceeds maximum|mass_rejected' | head -20
```

```bash
ssh prod-vps psql -U katpool -d katpool -c "
SELECT first_txn_id, sompi_to_miner, nacho_amount, db_entry_status, timestamp
FROM pending_krc20_transfers
WHERE db_entry_status = 'FAILED'
  AND timestamp > NOW() - INTERVAL '1 hour'
ORDER BY timestamp DESC LIMIT 20;"
```

## Diagnose

- [ ] **Did the network just hardfork?** Cross-reference recent
      rusty-kaspa releases and KIPs. Crescendo-style mass-formula
      changes happen at coordinated upgrades; if we missed one,
      this is the symptom.
- [ ] **Is the treasury UTXO set heavily fragmented?** Many small
      UTXOs raise mass per output. Check:

      ```bash
      ssh prod-vps katpool-cli utxo-stats   # tool lands in Phase 4
      ```

      If we have hundreds of < 0.5 KAS UTXOs, consolidation is the
      fix.

- [ ] **Did our calculator output disagree with what the network
      accepted?** Pull one failed tx, recompute mass via
      `katpool-storagemass`, compare to what rusty-kaspa's
      `consensus::core::mass` says. Any disagreement = calculator
      bug.

## Remediate

### Treasury UTXO consolidation (most common cause)

```bash
ssh prod-vps katpool-cli sweep-self \
  --max-inputs 100 \
  --target-output-count 5 \
  --policy-token <one-time-from-sops>
```

This sends a `N → M` transaction to the treasury itself,
consolidating many small UTXOs into a handful of larger ones.
Wait for confirmation; retry the failed payouts.

### Calculator bug

1. Stop payout cycles: `systemctl stop katpool-payout-kas
   katpool-payout-krc20`
2. Reproduce locally against the failed tx; add a regression test
3. Patch + PR + deploy
4. Re-trigger the failed payouts

### Network hardfork missed

1. Upgrade rusty-kaspa version per the release notes
2. Re-test mass formula against the new reference
3. Deploy + verify on testnet-10 before re-enabling mainnet
   payouts

## Verify

- No `StorageMassRejectionBurst` alerts in the last 30 min
- Re-triggered payouts succeed
- `pending_krc20_transfers` from the failed batch transitions to
  COMPLETED

## Post-incident

- Postmortem required (this class of failure has happened in
  production before — the new pool exists in part to prevent it)
- Add regression to the storage-mass property tests
- If consolidation was needed, add a scheduled UTXO-health job
  that proactively runs sweep-self before the UTXO count crosses
  the danger threshold
