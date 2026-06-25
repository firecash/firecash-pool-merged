# Runbook 22 — Mainnet cutover execution

> **Executed (2026-06).** Cutover complete; reconcile green; legacy pool
> shut down. Kept as a historical record. Importer/replay tools retired
> from the repo; evidence under `cutover-evidence/` and `replay-evidence/`.

The "hands on keyboard" sequence to move miners from the legacy pool to the new
Rust pool. Simple and **foolproof**: every step protects one invariant —
no miner loses an unpaid balance, the treasury is never spent by two pools at
once, and the whole thing is reversible by flipping DNS back.

**Downtime is minimized by pre-importing.** The full legacy import + reconcile
runs *ahead of time* against a clean production DB while legacy keeps serving,
so the only thing inside the dark window is a fast **idempotent delta** import,
the promote, and the DNS flip — a single ~30–60 s reconnect per miner (bounded
by the 60 s TTL), not a multi-minute freeze.

The handoff is **DNS-driven**: miners reconnect to the new pool on their own as
the 60 s-TTL record propagates, so there is no "connection freeze" step — when
legacy stops, sessions drop and reconnect to the new edge.

## The invariants (why each step exists)

1. **No lost balances** → legacy pays out every pending KAS balance *before* it
   stops (the importer carries history + NACHO rebates, **not** pending KAS).
2. **No double-spend / contamination** → only one pool spends the treasury at a
   time: legacy stops *before* the new pool goes live on the treasury address,
   and miners only reach the new pool *after* it is promoted.
3. **Correct key** → the treasury key controls the treasury address
   (`katpool treasury audit`, **verified ✓**).
4. **Bounded blast radius** → per-cycle spend caps set before payouts go live.
5. **Reversible** → legacy containers are stopped, never deleted; rollback is a
   DNS flip back + `docker compose start`.
6. **Clean reconcile** → the import target holds *only* imported legacy data at
   reconcile time. The new pool's canary soak data lives in a separate/cleared
   DB, so `legacy == new + allowance` is meaningful (the only tolerated gap is
   the importer's documented rejects — see PR #120's reject-aware reconcile).

## Pre-cutover gates (all must be true)

- [x] Canary soaked clean on the new pool (shares accepted end-to-end through
      the fly edge; observability green).
- [x] **Treasury key audit green** — `katpool treasury audit` confirms the key
      controls `kaspa:qz4j8mu…jxnp`.
- [x] fly anycast edge live + reachable (`kas.katpool.com:5555` TCP OK), origin
      nftables allowlist applied, `KATPOOL_STRATUM_PROXY_PROTOCOL=true`.
- [x] **Pre-import done + reconcile green** — full importer run into the clean
      production DB (canary-free, Invariant 6) exited 0 with
      `reconcile_all_passed == true`.
- [ ] **Spend caps set** in `ops/env/mainnet.env`:
      `KATPOOL_PAYOUT_MAX_SOMPI_PER_CYCLE`, `KATPOOL_KRC20_MAX_NACHO_PER_CYCLE`.
- [ ] **Rollback dry-checked**: legacy containers intact; confirm the legacy
      stratum IP (`152.53.37.182`) is recorded to flip DNS back to.
- [ ] DNS TTL on the `.xyz` stratum records lowered to **60 s ≥ a few hours
      ahead** so the flip propagates fast.

## Pre-import (ahead of the window, legacy still serving)

Run the importer once into the **production DB** (executed pre-cutover;
evidence archived):

```
# (retired) LEGACY_DATABASE_URL=…  KATPOOL_DATABASE_URL=<prod-db>  \
#   ./scripts/legacy-importer-rehearsal.sh --no-dry-run
```

The importer re-scans every legacy row (it is **idempotent**, not incremental),
so a full run is ~15–30 min — dominated by `block_details` (~567k rows). Against
a *live* legacy DB the reconcile will show small residuals on the append/mutate
tables (`blocks` grows; `nacho_rebate` moves); those go to **zero at T-0** once
legacy is stopped. What the pre-import proves is that the documented
reject/dedup **allowances are exact** (the money checks go green).

## Cutover (split import → dark window ≈ a few minutes)

The slow `blocks` transform is **display-only** (not used by the treasury scan
or payouts), so it is deferred out of the dark window. Only the money-critical
tables — `balances`/`payments`/`nacho_payments`/`krc20` (seconds) — are imported
and gated before promote; `blocks` is backfilled afterwards.

1. **Snapshot (rollback safety).** `pg_dump` the legacy DB →
   `cutover-evidence/…pre_cutover_<ts>.sql.gz`; record treasury KAS/NACHO
   balances + the legacy block count/last hash.
2. **Flush legacy.** Trigger a final legacy KAS payout of all pending balances;
   confirm pending → 0. *(Invariant 1.)*
3. **Stop legacy.** `docker compose stop katpool-app go-app katpool-payment
   katpool-monitor katpool-backup` — **do not remove** (rollback). *(Invariant 2.)*
   The dark window starts here.
4. **Fast money import + reconcile gate.** Re-run the importer with
   `--skip blocks` (balances/payments/nacho/krc20 only; ~2–3 min). With legacy
   stopped this is exact. **Gate:** exit 0 **and** `reconcile_all_passed == true`
   (blocks checks are omitted by `--skip`), else abort. This also refreshes the
   carried `nacho_rebate` balances to their final post-flush values *before* the
   pool can accrue forward (avoids a rebate write race).
5. **Promote the new pool.** In `ops/env/mainnet.env`: point at the prod DB, set
   `KATPOOL_POOL_ADDRESS` → the treasury address, `KATPOOL_TREASURY_CREDENTIAL`
   (key cred), `KATPOOL_COINBASE_MIN_DAA_SCORE` → the current treasury DAA, and
   the spend caps; keep `*_PAYOUT_DRY_RUN=true` for now. Deploy
   (`scripts/deploy.sh --network mainnet`) and confirm `/ready`.
6. **Flip DNS** — point every `.xyz` stratum record (and `kas.katpool.com`) at
   the fly anycast IP **`137.66.3.144`** / **`2a09:8280:1::129:8e82:0`**. Miners
   reconnect once over ~30–60 s. *(Invariant 5 is the reverse of this.)* The dark
   window ends here.
7. **Backfill blocks (background, legacy still stopped).** Run the importer with
   `--skip balances,payments,nacho_payments,krc20` (blocks only — historical
   display; legacy hashes never collide with the live pool's new blocks). When it
   finishes, a full `--skip` (none) reconcile should be all-green.
8. **Verify, then go live.** Shares accepted; coinbases land on the treasury;
   the public API + MiningPoolStats feed (`/api/pool/miningPoolStats`) serve
   from the new pool. Let one payout **dry-run** cycle log a clean plan, then
   flip `KATPOOL_PAYOUT_DRY_RUN=false` + `KATPOOL_KRC20_PAYOUT_DRY_RUN=false`
   and confirm the first live cycle settles on-chain.

## Rollback (any gate fails)

1. Flip the `.xyz` stratum DNS back to `152.53.37.182`.
2. `docker compose start` the stopped legacy containers (intact from step 3).
3. The legacy importer is append/idempotent — safe to re-run later. Record the
   abort + cause in `cutover-evidence/`.

## Note: treasury coinbase re-discovery (benign, bounded by the DAA floor)

When the new pool adopts the treasury address (step 5) its maturity tracker
scans the treasury's coinbase UTXOs. Setting `KATPOOL_COINBASE_MIN_DAA_SCORE` to
the treasury's current DAA makes it **skip every pre-cutover coinbase** — they
are neither recorded nor allocated, so there is no log/DB noise and no risk of
re-crediting blocks legacy already paid. Any coinbase below the floor is ignored
by both the recorder and the allocator.

---

See [`docs/cutover-plan.md`](../cutover-plan.md) for the full rationale, the
comms templates, and the optional dress rehearsal.
