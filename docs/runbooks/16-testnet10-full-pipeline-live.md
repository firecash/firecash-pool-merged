# Runbook 16 — testnet-10 full-pipeline live exercise

The Phase 3 M3d acceptance test. Validates the full mine-and-
allocate pipeline end-to-end on testnet-10:

1. **ASIC connects** to the bridge's stratum port.
2. **Bridge accepts shares**, emits `PoolEvent::ShareCredited`
   into the shared broadcast channel.
3. **Accountant event consumer** writes `share` rows.
4. **A block is found** (or, statistically, hashed-into-existence
   over the run window). Bridge emits `BlockFound` then
   `BlockAccepted`.
5. **Accountant event consumer** writes the `block` row in
   `submitted_to_node` status.
6. **Maturity tracker** observes kaspad confirming the block
   blue, then matured (100 blue blocks later).
7. **AllocationEngine** pro-rates the coinbase reward across the
   miners that contributed shares in the DAA window.
8. **`share_allocation` rows** land in the DB with correct
   audit-trail (`applied_topline_bps`, `applied_rebate_bps`,
   `applied_tier`).

What this exercises that runbook 15 (M3c) did not:

- Real share ingestion from real mining hardware.
- The shared `tokio::sync::broadcast` between bridge and
  accountant.
- `EventConsumer::run` against real-rate event flow.
- `WindowAggregator::close_window` against real shares.
- `AllocationEngine::allocate_coinbase_reward` against real
  miners + a real matured coinbase UTXO.

## Preconditions

- `kaspad-tn10` running and gRPC-reachable
  (runbook 13).
- Docker available for the throwaway Postgres.
- `psql`, `jq`, `sha256sum`, `cargo`, `ss` on PATH.
- A **testnet-10 ASIC** (or equivalent class-of-hash) pointed at
  the host running this script. The CPU miner is not sufficient
  at the bridge's minimum difficulty — see the empirical-limit
  block in `docs/phase-1-acceptance.md` for why.

## Command

```bash
export KASPAD_GRPC_URL='grpc://127.0.0.1:16210'
export KATPOOL_POOL_ADDRESS='kaspatest:...'     # the address(es) the ASIC mines to
export KATPOOL_RUN_FOR_SECS='900'               # 15 minutes; tune for desired share volume
export KATPOOL_STRATUM_PORT='15555'             # bridge listen port

./scripts/testnet10-full-pipeline-live.sh
```

The script:

1. Compiles the `katpool` runtime binary in release mode.
2. Spins up a throwaway Docker Postgres.
3. Applies every `crates/katpool-db/migrations/*.sql`.
4. Starts the runtime, which listens on
   `KATPOOL_STRATUM_PORT` and connects to kaspad.
5. **Pauses** while the operator points their ASIC at
   `stratum+tcp://<host>:<KATPOOL_STRATUM_PORT>` and authorizes
   as `KATPOOL_POOL_ADDRESS` (optionally `address.workername`).
6. Runs for `KATPOOL_RUN_FOR_SECS`.
7. Sends SIGTERM, captures evidence, tears down Postgres.

## ASIC configuration

The pool runs in **custodial PROP-pool mode**: every block's
coinbase pays the **pool's** address (`KATPOOL_POOL_ADDRESS`),
and the accountant later pro-rates the matured reward across the
miners who contributed shares (Phase 4 payout engine sends KAS to
each miner's authorized address).

So the address the miner authorizes with on `mining.authorize`
is the **miner's own** address (the one they want their share of
the payout sent to in Phase 4), **NOT** the pool's address.

Most testnet ASICs accept stratum URLs of the form:

```
URL:           stratum+tcp://<host>:<port>
Worker name:   <miner-kaspatest-address>.<rig-id>
Password:      x
```

Example for an IceRiver KS0 on a LAN with the host at
`192.168.1.10`, with the miner's wallet
`kaspatest:qy.....abc`:

```
URL:           stratum+tcp://192.168.1.10:15555
Worker name:   kaspatest:qy.....abc.ks0-rig01     ← MINER's address.worker
Password:      x
```

The pool address is the separate `KATPOOL_POOL_ADDRESS` env var
on the VPS side. The two addresses are **expected to differ** —
that's how the custodial model works.

The bridge auto-detects miner-vendor extranonce conventions on
`mining.subscribe` — no per-vendor flag needed.

## What success looks like

`manifest.json` shows:

```json
{
  "observed": {
    "shares": 12345,
    "blocks": 1,
    "allocations": 1
  },
  "disposition": "shares_and_blocks_observed",
  "runner_exit_code": 0,
  ...
}
```

Even partial success (shares but no block in the window) returns
exit code `0` and `disposition="shares_observed_no_blocks"` —
that's a successful share-ingestion proof, just statistically
unlucky on the block-find side.

`db-final.txt` shows:

- Per-wallet share counts and aggregate weights (the
  `share-summary` section).
- Block rows in `submitted_to_node` / `confirmed_blue` /
  `matured` status (the `block-rows` section). The deeper the
  blue-score depth at exit, the further the lifecycle advanced.
- `share_allocation` rows with full audit-trail columns if any
  block matured during the window (the `allocation-rows`
  section).

`katpool.log` should show:

```
INFO katpool runtime starting ...
INFO accountant consumer starting ...
INFO tracker sweep done ... still_waiting=N
INFO ks_valid_share_counter ...
INFO block found ...
INFO block confirmed blue ...
INFO block matured + allocated ...
```

## Exit codes

| Code | Meaning | Operator action |
|---|---|---|
| `0` (disposition `shares_and_blocks_observed`) | Full pipeline validated | Paste manifest + allocation-rows into Phase 3 acceptance ticket |
| `0` (disposition `shares_observed_no_blocks`) | Ingestion validated; block luck didn't hit | Increase run window OR accept as partial validation |
| `2` | Runtime exited unexpectedly | Read `katpool.log`; file `bug:bridge` or `bug:accountant` |
| `3` | No activity at all | Likely the ASIC didn't connect; check stratum URL, firewall, kaspad reachability |

## Cleanup

Automatic via `trap cleanup EXIT`. If you `kill -9` mid-run the
Docker container may persist; clean up with:

```bash
docker ps -a --filter "name=katpool-pipeline-pg-" -q | xargs -r docker rm -f
```

## What goes into the acceptance ticket

Paste into the Phase 3 acceptance issue (or this PR's comments):

- `manifest.json` (full file)
- Top-of-`katpool.log` showing the three subsystems starting
- The state transitions in `katpool.log` (search for `confirmed blue`, `matured + allocated`)
- The `share-summary` and `allocation-rows` sections of `db-final.txt`

## What this still doesn't prove

- Full Phase 3 close-out also requires the **24h replay-determinism harness**
  (Phase 3 M4): `cargo test -p accountant --test replay_harness_scale` plus
  operator-captured NDJSON under `replay-evidence/` (executed at cutover).
- **Payouts** are Phase 4 (KAS) + Phase 5 (NACHO) — separate ticket.
