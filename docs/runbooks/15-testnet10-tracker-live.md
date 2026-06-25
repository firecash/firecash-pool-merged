# Runbook 15 — testnet-10 maturity-tracker live exercise

The Phase 3 M3c acceptance test for the accountant's maturity
tracker. Validates real-kaspad-gRPC integration end-to-end on
testnet-10 (Toccata-aware) without yet requiring the full
bridge-plus-accountant unified runner (that's M3d).

What this exercises:

1. The `KaspadGrpcClient` connects to the operator's
   `kaspad-tn10` and authenticates correctly.
2. `get_sink_blue_score` returns the current virtual chain tip's
   blue score.
3. `get_block` parses real testnet-10 `RpcBlock` payloads
   (including Toccata-fork blocks) without panicking or drifting
   on the `verbose_data` shape.
4. Coinbase reward extraction reads real coinbase transaction
   outputs.
5. The maturity tracker's state machine drives a seeded block
   from `submitted_to_node` to `matured` in response to live DAG
   state.
6. The `run_loop` cleanly handles a `SIGTERM` from `timeout`.

What this does **not** exercise (deferred to M3d / Phase 7
acceptance):

- Stratum share ingestion (no miners connected to the bridge in
  this test).
- The accountant's full event-consumer pipeline.
- Real PROP allocation against contributing miners.
  (The seeded block is allocated via `NoContributingWallets`
  because no shares exist in the DAA window — that's expected.)
- KAS / NACHO payouts.

## Preconditions

Operator must have:

- `kaspad-tn10` running and gRPC-reachable (per
  [runbook 13](13-kaspad-tn10-bootstrap.md)).
- Docker available for the throwaway Postgres.
- `psql`, `jq`, `sha256sum`, and `cargo` on `PATH`.
- A known testnet-10 block whose blue-score depth ≥ 100 below
  the current sink (so it'll mature during the test). The
  operator can find one with:

  ```bash
  # Replace 127.0.0.1:16210 with your kaspad gRPC endpoint.
  grpcurl -plaintext \
    -d '{}' \
    127.0.0.1:16210 \
    protowire.RPC/GetBlockDagInfoRequest \
    | jq '.sinkHash, .virtualDaaScore'
  ```

  Then pick a block ~150-200 blue scores deep. The block's hash
  + DAA score are the values that go into the env vars below.

  Alternative: use the kaspad UI / explorer to find a recent
  testnet-10 block, then `grpcurl ... GetBlockRequest
  '{"hash":"<hash>", "includeTransactions":true}'` to confirm
  the block is in kaspad's store.

## Command

```bash
export KASPAD_GRPC_URL='grpc://127.0.0.1:16210'
export KATPOOL_POOL_ADDRESS='kaspatest:...'       # block's coinbase recipient
export KATPOOL_SEED_BLOCK_HASH='cc2b...138a'      # 64-hex
export KATPOOL_SEED_DAA_SCORE='467579632'         # u64 DAA
export KATPOOL_RUN_FOR_SECS='180'                 # optional; default 120

./scripts/testnet10-tracker-live.sh
```

The script:

1. Compiles `accountant-tracker-runner` in release mode.
2. Spins up a throwaway Docker Postgres on a free port.
3. Applies every `crates/katpool-db/migrations/*.sql`.
4. Seeds a `block` row in the `submitted_to_node` state for the
   block hash you provided.
5. Runs the tracker for `KATPOOL_RUN_FOR_SECS`.
6. Tears down Postgres.
7. Writes evidence to `tracker-evidence/<UTC-stamp>-tracker-live/`.

## What success looks like

`manifest.json` shows:

```json
{
  "final_block_status": "matured",
  "disposition": "matured",
  "runner_exit_code": 0,
  ...
}
```

`db-final.txt` shows the block row's `status = matured` and
`matured_at` populated; an `audit_log` entry with
`action = block.allocated_empty` (because no shares were seeded
into the window).

`tracker.log` shows the lifecycle progression:

```
INFO ... block confirmed blue
INFO ... block matured + allocated
```

## Exit codes

| Code | Meaning | Operator action |
|---|---|---|
| `0` | Block matured cleanly | Capture evidence; live test passes |
| `2` | Tracker exited with an error | Read `tracker.log`; file `bug:accountant` if it's a regression |
| `3` | Observation window elapsed without maturity | Either pick a deeper block or increase `KATPOOL_RUN_FOR_SECS` |

## Cleanup

The script handles cleanup automatically (`trap cleanup EXIT`).
If you `kill -9` mid-run, the Docker container may persist; clean
up with:

```bash
docker ps -a --filter "name=katpool-tracker-pg-" -q | xargs -r docker rm -f
```

## What goes into the acceptance ticket

Paste into the Phase 3 acceptance issue (or PR comment):

- `manifest.json` (the full file)
- The relevant slice of `tracker.log` showing the state transitions
- The relevant slice of `db-final.txt` showing the `block.allocated`
  audit entry and the final block row
