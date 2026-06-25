# Runbook 01 — Blocks stopped being found

## Symptom

The pool is up, miners are connected and submitting shares, but no
new blocks are being accepted on-chain. The alert `BlocksNotFound`
fires when `time() - last_block_accepted_ts > 300` (5 minutes).

This is the failure mode that bit production on 2026-04-21 12:40 UTC
and stayed undetected for 32 hours. Now there is an alert. Treat it
as SEV-2 minimum and escalate to SEV-1 if it persists beyond 30 min.

## Confirm

```bash
# Time since last accepted block (replace with your dashboard URL)
curl -s 'http://obs.railway.app/api/datasources/proxy/1/api/v1/query?query=time()%20-%20pool_last_block_accepted_ts'

# kaspad sync state
ssh prod-vps 'docker logs --tail 20 kaspad 2>&1' | grep -E '(Accepted|sync)'

# Pool process active?
ssh prod-vps systemctl is-active katpool
```

If `kaspad` is healthy (accepting blocks via relay) but our pool
hasn't found one in > 5 min, something is broken between
share-received and block-submit. That's the case we are in.

## Diagnose

Walk this checklist in order:

- [ ] **Stratum is still receiving shares.** Check
      `pool_shares_credited_total` rate in Grafana. If zero, this is
      not "blocks stopped" — it's "shares stopped". Different runbook.
- [ ] **Bridge ↔ embedded kaspad is healthy.** Check
      `bridge_kaspad_rpc_calls_total` and the error counter. The
      legacy pool hit this exact failure mode when the WASM RPC
      client got stuck.
- [ ] **`submit_block` is being called.** Trace logs filtered to
      `event=submit_block`. If we're calling submit_block but they
      all fail, look at the response error class.
- [ ] **kaspad has peers.** Run `kaspad`'s status RPC; peer count
      below 5 triggers [03](03-kaspad-lost-peers.md). A node with
      no peers cannot propagate blocks even if we submit them.
- [ ] **Local clock skew.** `timedatectl status` on the VPS. A drift
      > 1 s can cause shares to be evaluated against the wrong
      template.

## Remediate

In ascending order of disruption:

1. **Restart the pool process** (`systemctl restart katpool`).
   On the legacy pool, the 32 h drought ended the moment we
   restarted katpool-app. Five minutes of stratum downtime is
   acceptable; ASIC miners auto-reconnect.
2. If restart doesn't help, **restart kaspad** as well.
3. If still failing, **check for a recent dependency or config
   change** that landed since the last accepted block. Roll back
   that change via the deploy script.
4. If all of the above are healthy and we still can't find blocks,
   open an issue on `kaspanet/rusty-kaspa` with the relevant trace
   logs. This would indicate a protocol-level or template-encoding
   issue; consult upstream.

## Verify

- `time() - pool_last_block_accepted_ts < 60` for a sustained 10-min
  window
- New rows appearing in `block_details` table
- Canary miner shares being credited normally

## Post-incident

- File a postmortem if the alert fires for > 30 min.
- If the cause was a regression, add a replay test that captures
  the failing scenario.
- Refresh this runbook with any newly-learned diagnostic step.
