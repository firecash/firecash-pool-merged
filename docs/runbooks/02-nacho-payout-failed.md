# Runbook 02 — NACHO payout failed

## Symptom

A NACHO rebate payout cycle terminated with one or more recipients
unpaid. Alert `NachoPayoutFailed` fires when
`rate(payout_krc20_failures_total[1h]) > 0`. Treat as SEV-2; SEV-1
if every recipient in a cycle failed.

This is the failure mode that hit production on 2026-05-01 with 14
of 15 recipients failing on storage-mass-exceeds-maximum.

## Confirm

```bash
# Recent failure count
ssh prod-vps journalctl -u katpool --since '1 hour ago' | \
  grep -E 'KRC20Transfer.*failed' | head -20

# Pending rows in DB
ssh prod-vps psql -U katpool -d katpool -c "
SELECT db_entry_status, COUNT(*)
FROM pending_krc20_transfers
WHERE timestamp > NOW() - INTERVAL '3 hours'
GROUP BY db_entry_status;"

# Treasury NACHO balance
curl -s 'https://api.kasplex.org/v1/krc20/address/<treasury>/token/NACHO'
```

## Diagnose

- [ ] **`Storage mass exceeds maximum` in logs?** → see
      [07](07-storage-mass-rejection-burst.md). The new pool's
      mass-aware batcher should prevent this; a hit here is a bug.
- [ ] **`Couldn't deserialize u64` in logs?** → WASM-class failure
      that the new pool should not have. Ticket it as a P0 regression.
- [ ] **Treasury NACHO balance below required amount?** The cycle
      tried to spend more NACHO than the treasury holds. Check the
      floor-price cache for a stale or wildly-off value.
- [ ] **External-API outage (kasplex)?** Circuit breaker should
      have opened. See observability stack for breaker state. If
      open: cycle skipped correctly with an alert; not a P0.
- [ ] **Treasury UTXO set fragmented?** Many small UTXOs = high
      storage mass even for valid plans. Run the consolidation
      script (see [07](07-storage-mass-rejection-burst.md)).

## Remediate

1. If due to UTXO fragmentation: run consolidation, then re-trigger
   the failed cycle.
2. If due to bug: revert offending change; re-trigger after fix.
3. If due to external API outage and a cycle was correctly skipped:
   confirm via dashboard, no action needed; next cycle will pick up.
4. **Manual replay** of failed recipients (after root-causing):

   ```bash
   ssh prod-vps katpool-cli replay-krc20 --cycle <cycle-id> \
     --addresses <comma-separated>
   ```

   (Tool lands in Phase 5.)

## Verify

- `pending_krc20_transfers` with `db_entry_status='PENDING'` from
  the affected cycle drops to zero
- `nacho_payments` table has corresponding new rows
- On-chain reveal tx confirmations visible via `api.kasplex.org`

## Post-incident

- File a postmortem
- If the cause was a class of mass-rejection not covered by current
  tests: add a regression test to `katpool-storagemass` and to
  `payout-krc20`
