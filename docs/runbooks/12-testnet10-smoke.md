# Runbook 12 — testnet-10 smoke against the stratum bridge

Run this before promoting any bridge build to mainnet. The smoke
verifies the three Phase 1 acceptance criteria that cannot be checked
in CI:

1. Bridge boots in **< 5 s** in external (non-inprocess) mode against
   a real kaspad-testnet-10 endpoint.
2. The bridge accepts **≥ 100 valid shares** from a CPU miner over a
   60-second run.
3. **At least one block** is mined within that 60 s window.

The first criterion guards against startup regressions (slow gRPC
handshake, dependency-cascade builds, blocking I/O on the hot path).
The second and third confirm the end-to-end PoW pipeline still works
under the anti-abuse limits introduced in Phase 1 milestone 3.

## Symptom

This runbook is *proactive*. There is no incident to confirm — it is
executed once per release candidate.

## Confirm

Verify you have everything before starting:

```bash
# Bridge binary built from the candidate commit
which katpool-stratum-bridge
katpool-stratum-bridge --version

# Reachable kaspad-testnet-10. Replace with your endpoint.
nc -z -w 3 <kaspad-testnet10-host> 16210 && echo ok

# If pointing at the local katpool-kaspad-tn10, confirm it has
# completed IBD and is following the live tip. The discriminator is
# the "via relay" suffix in the journal — IBD blocks come through the
# headers-proof / sync-block paths and never have that suffix; only
# P2P-relayed blocks (= live tip) do.
journalctl -u katpool-kaspad-tn10 --no-pager -n 50 \
    | grep -c 'Accepted [0-9]\+ blocks .* via relay'
# Expect: > 0. If zero, kaspad is still in IBD (typically 30-60 min
# from cold start on this VPS; see runbook 13).

# CPU miner artifact (built from this repo)
ls bridge/examples/cpu_stratum_miner.rs

# Tools the smoke script consumes
which curl jq
```

The smoke script itself enforces the "kaspad at tip" precondition
when targeting `127.0.0.1:16210` — if you point at an external
operator-owned node you are responsible for confirming its sync state
yourself.

## Diagnose

Not applicable — this is an acceptance procedure, not an incident
diagnosis. If the smoke fails, stop the candidate from rolling out and
file an issue with the failure JSON and the bridge journal logs.

## Remediate

Run the canned smoke harness. All variables have safe defaults; the
only required override is the testnet wallet:

```bash
export KATPOOL_TESTNET10_WALLET=kaspatest:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9
export KASPAD_TESTNET10_GRPC=127.0.0.1:16210
export KATPOOL_BRIDGE_CONFIG=/etc/katpool/config.yaml
scripts/testnet10-smoke.sh | tee phase-1-acceptance-$(date -u +%FT%TZ).json
```

The script:

1. Cold-boots the bridge with `--node-mode external` and measures the
   wall time until the stratum port accepts a TCP connection.
2. Snapshots `ks_valid_share_counter` and `ks_blocks_mined` from
   `/metrics`.
3. Runs the CPU miner for `${KATPOOL_TESTNET10_DURATION:-60}` seconds.
4. Re-snapshots the counters and prints a single JSON line.

A successful run looks like:

```json
{
  "ts": "2026-05-25T22:30:00Z",
  "result": true,
  "boot": { "secs": 1.43, "budget": 5, "ok": true },
  "shares": { "observed": 412, "required": 100, "ok": true },
  "blocks": { "observed": 3, "required": 1, "ok": true }
}
```

A failed run reports `"result": false` and exits non-zero. Capture the
bridge journal alongside the JSON before tearing down the test.

## Verify

- The JSON's `result` field is `true`.
- All three nested `ok` fields are `true`.
- `journalctl -u katpool-bridge --since "30 min ago"` shows no
  `WARN`/`ERROR` outside expected steady-state messages
  (`stale share`, `vardiff adjust`, etc.).
- `curl -s :2114/metrics | grep -E "^ks_anti_abuse_"` confirms the
  anti-abuse counters were exercised (they should all be 0 in a clean
  test — non-zero suggests the miner is misconfigured or the limits
  are too tight for the test setup).

## Post-incident

Archive the JSON in `docs/phase-1-acceptance.md` under the "Run history"
table on the corresponding PR. Anyone reviewing the release later can
confirm the candidate passed by reading the table.
