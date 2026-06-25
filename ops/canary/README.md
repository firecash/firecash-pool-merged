# Canary miner (end-to-end "are we actually paying?" probe)

ADR-0004 ground truth: a real miner, **running outside the pool VPS**, that
submits real shares and confirms it gets credited. If internal metrics look
healthy but this stops being credited, the real accept → validate → account →
credit path is broken. It backs the `CanaryMinerNotPaid` alert.

This is a **local tool** — run it on a MacBook or any Linux box, not on the pool
host and not as a Railway service. Two pieces:

1. **A CPU miner** (off-the-shelf) submitting shares to the pool stratum with a
   dedicated canary wallet.
2. **`katpool-canary.py`** — a dependency-free watcher that polls the pool API
   for the canary wallet's `last_seen_at` and publishes
   `canary_last_credited_timestamp_seconds` to VictoriaMetrics via vmauth.

## One-time setup

- **Canary wallet**: generate a dedicated Kaspa address for the canary (do *not*
  reuse the treasury or a real miner). On testnet a `kaspatest:` address; on
  mainnet a `kaspa:` address. It needs no funds — it only earns pool credit.
- **vmauth creds**: the same write user the origin `vmagent` uses (the canary
  pushes one gauge through the existing `/api/v1/import` route).

## 1. Run a CPU miner → pool stratum

Any Kaspa stratum miner works; point it at the pool's stratum endpoint with the
canary wallet as the mining address and a **low** difficulty (the canary only
needs an occasional accepted share, not hashrate). For tn10 the pool stratum is
`stratum+tcp://152.53.37.182:15555`; for mainnet use the advertised edge host.

```sh
# Example shape (substitute your miner binary + flags):
<kaspa-cpu-miner> \
  --stratum stratum+tcp://152.53.37.182:15555 \
  --wallet  kaspatest:<your-canary-address> \
  --worker  canary
```

A single low-end core is plenty: at tn10 difficulty one accepted share every few
minutes keeps `last_seen_at` fresh, well inside the 1h alert window.

## 2. Run the watcher

```sh
export KATPOOL_CANARY_API_BASE=https://api-tn10.katpool.com
export KATPOOL_CANARY_ADDRESS=kaspatest:<your-canary-address>
export KATPOOL_CANARY_VMAUTH_URL=https://vmauth-stage-testnet-10.up.railway.app
export KATPOOL_CANARY_VMAUTH_USER=katpool-tn10-write
export KATPOOL_CANARY_VMAUTH_PASSWORD=...        # same as the vmagent's
export KATPOOL_CANARY_NETWORK=testnet-10
python3 ops/canary/katpool-canary.py
```

It logs each cycle (`published canary_last_credited_timestamp_seconds=… credited
Ns ago`) and retries through transient network errors. Keep it running with
`launchd` (macOS), a `systemd --user` unit (Linux), `tmux`, or a container.

## How the alert closes the loop

`canary_last_credited_timestamp_seconds` is the canary wallet's last credited
share time. `CanaryMinerNotPaid` (in `victoria-metrics/rules/`) pages when
`time() - max(canary_last_credited_timestamp_seconds) > 3600` — i.e. the canary
has not been credited in over an hour, so either the miner stopped or the
accept → credit path is broken. Run miner + watcher from a network path a real
miner would use (ideally a different ISP/region than the VPS) so the probe also
exercises the edge.
