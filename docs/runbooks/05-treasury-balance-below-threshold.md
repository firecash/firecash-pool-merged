# Runbook 05 — Treasury balance below threshold

## Symptom

Alert `TreasuryBalanceLow` fires when the on-chain KAS balance of
the treasury address falls below the configured threshold (default
`kasAlertThreshold = 250 KAS`), or when the on-chain NACHO balance
falls below `nachoAlertThreshold = 1000 NACHO`.

Treat as SEV-2 by default; SEV-1 if a payout cycle is imminent and
the balance is insufficient to cover it.

## Confirm

```bash
# On-chain balances
curl -s 'https://api.kaspa.org/addresses/<treasury>/balance' | jq
curl -s 'https://api.kasplex.org/v1/krc20/address/<treasury>/token/NACHO' | jq

# Expected payout amount for the next cycle (KAS)
ssh prod-vps psql -U katpool -d katpool -c "
SELECT SUM(balance) AS eligible_total
FROM miners_balance
WHERE balance >= 500000000;"
```

## Diagnose

- [ ] **Is mining still producing coinbases?** If yes, KAS will
      refill over time. The threshold may just be tight relative to
      a temporary low. Decide whether to raise the threshold or
      let it self-resolve.
- [ ] **Did a recent payout cycle drain more than expected?**
      Check the policy-guard logs — any guard violations should
      have been alerted separately.
- [ ] **Was there a withdrawal we didn't initiate?** Compare on-
      chain tx history (api.kaspa.org) to our `payments` table.
      Any tx from the treasury NOT in `payments` is a finding.
      Escalate to SEV-1 immediately if found.
- [ ] **NACHO balance**: the rebate cycle requires the treasury to
      hold NACHO. If it's empty, the cycle correctly skips and
      this is informational, not an outage.

## Remediate

- If self-resolving (mining will refill): silence the alert for
  the expected refill duration; document.
- If a payout cycle is imminent and insufficient KAS: pause the
  payout cron, then either:
  - Wait for additional coinbases to mature
  - Top up the treasury from cold storage (operator-only)
- If unexpected outbound tx: immediately
  1. Stop the payout services: `systemctl stop katpool-payout-kas
     katpool-payout-krc20` (or whichever unit covers them)
  2. Rotate the treasury key per [11](11-key-rotation.md)
  3. Open a SEV-1 incident and a forensic investigation

## Verify

- Treasury balance back above threshold
- Next payout cycle runs to completion without policy violations

## Post-incident

- Postmortem required if an unexpected outbound tx occurred
- Review threshold settings; widen or narrow based on real refill
  patterns observed over the last 30 days
