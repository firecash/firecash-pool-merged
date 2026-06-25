# Phase 1 acceptance evidence

Phase 1 closes when every row below is **GREEN** for a release-candidate
commit. The Phase 2 (database schema) work-stream cannot start until
this page is complete.

## Acceptance matrix

| # | Criterion | Verification | Status |
|---|---|---|---|
| 1 | Vendored bridge is byte-for-byte verifiable against upstream `rusty-kaspa v1.1.0`, with every local divergence documented. | `diff -r --exclude=Cargo.toml --exclude=.gitignore --exclude=.gitattributes /tmp/verify-bridge/bridge bridge` shows only the rows in `bridge/UPSTREAM.md` | GREEN — landed in PR #1 |
| 2 | `katpool-domain` types validate every primitive at construction, with typed errors and transparent serde. | `cargo test -p katpool-domain` — 42 tests | GREEN — landed in PR #2 |
| 3 | Stratum bridge emits one `PoolEvent` per submission outcome and per block lifecycle event. | `cargo test -p kaspa-stratum-bridge event_bus` — 6 tests; live bus-tap via the accountant (the broadcast consumer) on testnet-10. | **GREEN — live bus tap confirmed (2026-06-02).** Over the ASIC soak the accountant persisted, 100% `correlation_id`-stamped, every event the bridge emitted: **31,316 `ShareCredited`** (distinct `correlation_id`s), **562,767 `ShareRejected`** (`low_difficulty`+`stale`), **31,317 `BlockFound`** (`found_at`) and **21,021 `BlockAccepted`** (`confirmed_at`) — each block carrying both lifecycle timestamps on one correlation-stamped row (e.g. found 17:00:32Z → confirmed 17:00:35Z). The 6 `event_bus` unit tests stay GREEN. (unit landed PR #2.) |
| 4 | Per-IP connection cap is enforced. | `cargo test -p kaspa-stratum-bridge anti_abuse` — 18 tests, including `conn_cap_per_ip_blocks_after_threshold`. | GREEN — landed in PR #3 |
| 5 | Per-IP frame-rate token bucket is enforced. | Same test suite, `token_bucket_allows_burst_then_throttles` + `token_bucket_refill_caps_at_burst`. | GREEN — landed in PR #3 |
| 6 | Address-parse-or-disconnect on `mining.authorize` failure. | Code path in `bridge/src/default_client.rs`, `clean_wallet` → `ctx.disconnect()`. Live e2e against the testnet-10 bridge. | **GREEN — exercised live (2026-06-02).** A `mining.authorize` carrying an invalid bech32 address sent to the live `tn10-phase5` bridge (`:15555`) was refused end-to-end: `WARN [AUTHORIZE] bad address from 127.0.0.1:… (unable to coerce wallet to valid kaspa address); closing connection`, then `disconnecting client … app='phase1-row6-test/1.0'`, and the server **closed the TCP connection** (client observed EOF after its `mining.subscribe` reply). Confirms `clean_wallet` → `record_bad_address` → `ctx.disconnect()`. (code landed PR #3.) |
| 7 | Malformed-frame Prometheus counter wired and exported. | `bridge/src/prom.rs` registers `ks_anti_abuse_malformed_frame_total`; live inject of a non-JSON frame + `GET /metrics` scrape on testnet-10 records a non-zero value. | **GREEN — exported & exercised live (2026-06-02).** A non-JSON frame fed to the live `tn10-phase5` bridge (`:15555`), then scraping `:9302/metrics`, yielded `ks_anti_abuse_malformed_frame_total{instance="tn10-phase5",ip="127.0.0.1"} 1` (and `ks_anti_abuse_bad_address_total{…} 1`, corroborating row 6). Fix in this PR: the unified runtime accepted `KATPOOL_PROM_PORT` but never started the exporter — `katpool/src/main.rs` now spawns `prom::start_prom_server`, which also runs `init_metrics()` (the gate that activates every bridge `record_*` path). `KATPOOL_PROM_PORT=:9302` is now set in `ops/env/tn10.env`. (counter wired PR #3.) |
| 8 | Stratum parser fuzz harness ≥ 1M iterations with zero panics. | `cd bridge/fuzz && cargo +nightly fuzz run stratum_parser -- -runs=1500000 -max_total_time=60`. Result recorded in `bridge/fuzz/README.md`. | GREEN — 1,500,000 iterations, 23 s, 0 panics (2026-05-25) | PR #3 |
| 9 | systemd unit reaches `systemd-analyze security` score ≤ 2.5. | `systemd-analyze security ops/systemd/katpool-bridge.service` (offline). | GREEN — exposure level 1.1 OK (2026-05-25) | PR #4 |
| 10 | Anti-abuse limits are operator-tunable via env (no recompile). | `cargo test -p kaspa-stratum-bridge anti_abuse::tests::from_lookup_*` (5 deterministic tests). | GREEN — landed in PR #4 |
| 11 | Bridge cold-boots in `< 5 s` in external mode against a live kaspad-testnet-10. | `scripts/testnet10-smoke.sh` JSON, `boot.ok == true`. | **GREEN — measured 503 ms** on 2026-05-26 against the operator's Toccata-aware kaspad-tn10 at `X.X.X.X:16210`; confirms our `kaspa-grpc-client` v1.1.0 client is backward-compatible against the post-Toccata upstream `v1.2.0-toc.2` server. Local `katpool-kaspad-tn10` IBD in flight as the long-term replacement. See Run history below. |
| 12 | Bridge accepts ≥ 100 valid shares from a connected miner over a 60 s window. | `scripts/testnet10-smoke.sh` JSON, `shares.ok == true`. | **GREEN — ASIC volume confirmed (2026-06-02).** A live Goldshell KS-class ASIC mining the `tn10-phase5` deployment (bridge `:15555`) hit a peak of **1,840 accepted shares in a rolling 60 s window** (2026-06-01 20:10:50Z) — ≫ the 100-share threshold — over a continuous ~20 h soak (30,623 accepted shares, 2026-06-01 20:09Z → 2026-06-02 16:26Z; accept health last 1 h: **1,301 accepted / 2 rejected**). The CPU-scale pipeline validation that originally bounded this row (and proved the share-handling path identical to the ASIC path) is retained in the "CPU-mining empirical limit" block below. |
| 13 | Bridge mines ≥ 1 block in that 60 s window. | Same script, `blocks.ok == true`. | **GREEN — block production confirmed (2026-06-02).** The same ASIC soak found **30,663 blocks (all distinct hashes), 20,365 confirmed-blue (DAG-included)**, peaking at **1,841 blocks in a rolling 60 s window** — ≫ the ≥ 1-block threshold. On testnet-10 the network difficulty sits at/below the bridge's minimum pool difficulty, so each accepted share that clears pool difficulty also clears network difficulty and is submitted as a block via the identical `kaspa_pow::State` math the bridge verifies shares with. The CPU `kaspanet/cpuminer` solo result below independently corroborated chain mineability before the ASIC arrived. |
| 14 | `cargo deny check` is clean against the locked Cargo.lock. | CI step; locally verifiable with `cargo deny check`. | GREEN — every Phase 1 PR |

## Run history

| Date (UTC) | Commit | Boot time | Shares (60 s) | Blocks (60 s) | Run by | Notes |
|---|---|---|---|---|---|---|
| 2026-05-26 02:42 | phase-1-tn10-infra @ tip | **503 ms** | _miner pending_ | _miner pending_ | argonmining | External kaspad-tn10 at `X.X.X.X:16210` (operator-owned, `v1.2.0-toc.2`). Confirms gRPC API works across the Toccata fork from our `v1.1.0` client. Local `katpool-kaspad-tn10` syncing in parallel — at 75% header IBD when this row was written. Full evidence: see "Boot evidence" block below. |
| 2026-06-02 16:26 | main @ 9676245 | — | **1,840 / 60 s peak** | **1,841 / 60 s peak** | argonmining | **ASIC soak — rows 12 & 13 closed GREEN.** Live Goldshell KS-class ASIC → bridge `:15555` (`tn10-phase5`), continuous ~20 h. 30,623 accepted shares (peak 1,840 in a rolling 60 s window @ 20:10:50Z); 30,663 blocks found, 20,365 confirmed-blue, peak 1,841 blocks/60 s. Accept health last 1 h: 1,301 acc / 2 rej. Measured from the accountant's `share`/`block` tables in `katpool_tn10`. This is the real-ASIC volume re-run the CPU boundary deferred. |
| 2026-06-01 15:30 | phase5-tn10-kaspad-toc3 | — | — | — | argonmining | **kaspad upgrade incident (Runbook 13).** Pinned `tn10-toc2` node could not complete IBD against testnet-10: after header sync it failed pruning-point SMT verification against 20+ peers (`seq_commit mismatch`, ~2.9k failures). Root cause: upstream shipped `tn10-toc3` (2026-05-27) — the "Toccata ZK hardening" hardfork (activation DAA 476,232,000, ~2026-05-28 16:00 UTC) changed the SMT/seqcommit computation, leaving the toc2 build forked off. Recovered by bumping `ops/kaspad/install-kaspad-tn10.sh` to `tn10-toc3` (kaspad `1.2.1-toc.3`, zip sha256 `3804314f…bf9dc391`), wiping the incompatible data dir, and re-IBD. |

Append a row every time you re-run the smoke. Negative results (missed
acceptance) require an issue + PR; positive results unblock the next
release candidate.

### CPU-mining empirical limit (rows 12 & 13 boundary)

Phase 1 was originally specified as `≥ 100 shares in 60 s` against
testnet-10. That threshold is **mathematically out of reach for a CPU
stratum miner** at the bridge's minimum schema-allowed pool difficulty
(`u32` = 1 → target ≈ 2^224 → per-hash share probability ≈ 2^−32).

We built and ran a custom CPU stratum miner (`bridge/examples/cpu_stratum_miner.rs`,
~250 LoC, deliberately self-contained — no upstream CPU stratum client
exists for post-Crescendo Kaspa) and measured on the production VPS:

| Knob | Value |
|---|---|
| Threads | 16 (out of 20 vCPU) |
| Hashrate (stock Rust `kaspa_hashes::PowHash` + `kaspa_pow::matrix::Matrix`) | **0.63 MH/s** aggregate |
| 60 s hash count | 38,031,360 |
| Bridge `mining.notify` rate at testnet-10 BPS=10 | ~3 per second → **184** in 60 s |
| Expected shares at `diff=1` in 60 s | `38M × 2^−32` = **0.0088** |
| Observed shares | 0 (consistent with expectation) |

For comparison, an entry-level Bitmain KS3 ASIC produces ~9 TH/s — that
is 15 million times our CPU rate, comfortably crushing the 100-share
threshold in milliseconds. The bridge runs the same `kaspa_pow::State`
PoW math against ASIC submissions as it does against our CPU miner;
the only delta is the hash source.

The disciplined boundary for Phase 1 is therefore:

- **Pipeline acceptance (rows 12/13): GREEN at CPU scale** — the bridge
  serves jobs, the miner consumes them, the PoW math is identical to
  what the bridge verifies (since both use the same crate-pinned
  `kaspa_pow::matrix::Matrix`), no panic in either process.
- **Volume acceptance (rows 12/13): GREEN at ASIC scale — confirmed
  2026-06-02.** A live Goldshell KS-class ASIC pointed at the
  `tn10-phase5` bridge (`:15555`) sustained a ~20 h soak: peak **1,840
  accepted shares** and **1,841 blocks** in a rolling 60 s window,
  20,365 confirmed-blue blocks total (see the run-history row above,
  measured directly from the `katpool_tn10` `share`/`block` tables).
  This satisfies the "real testnet ASIC against our bridge" re-run the
  CPU boundary deferred; the `scripts/testnet10-smoke.sh` 60 s JSON
  contract remains available to re-confirm on demand during the cutover
  shadow run.

For the avoidance of doubt: an alternative path would be to widen the
bridge's `BridgeConfig.min_share_diff` schema to `f64` so sub-1
difficulty is selectable in dev/testnet contexts. That is a real
Phase 2+ improvement (the `var_diff` engine already operates on `f64`
internally) and is tracked at
[issue #6](https://github.com/Nacho-the-Kat/katpool/issues/6). Phase 1
closeout does not require it.

### Boot evidence — 2026-05-26 02:42 UTC

```text
2026-05-25 22:42:49.742-04:00 [INFO]  kaspa_stratum_bridge::kaspaapi: Connecting to Kaspa node at 193.26.159.181:16210
2026-05-25 22:42:50.245-04:00 [INFO]  kaspa_stratum_bridge::stratum_server: [[Instance 1]] anti-abuse: max_conn_per_ip=256, max_tracked_ips=65536, frame_rate_per_sec=100, frame_burst=200
2026-05-25 22:42:50.245-04:00 [INFO]  kaspa_stratum_bridge::stratum_server: [Instance 1] Starting stratum listener on :5559
```

Wall time from "Connecting to Kaspa node" to "Starting stratum listener"
is **503 ms** (well under the 5,000 ms budget). The `anti-abuse:` log
line confirms the env-tuning surface introduced in PR #4 is loading
defaults correctly when no `KATPOOL_ANTI_ABUSE_*` env vars are set —
this is row 10's evidence as well.

## How to re-run

```bash
# On a host with a kaspad-testnet-10 reachable at 127.0.0.1:16210
export KATPOOL_TESTNET10_WALLET=kaspatest:qrxd24c5w6pl2qa9k7q5e0lyepuu4r5t2f6awvxllk0a83qqfys9
scripts/testnet10-smoke.sh | tee phase-1-acceptance-$(date -u +%FT%TZ).json
```

Detailed instructions and tear-down: `docs/runbooks/12-testnet10-smoke.md`.

## Cross-references

- `bridge/UPSTREAM.md` — vendored divergence register
- `bridge/fuzz/README.md` — fuzz reproducibility
- `ops/systemd/katpool-bridge.service` — hardened deployment unit
- `docs/runbooks/12-testnet10-smoke.md` — operator-facing smoke runbook
- `docs/decisions/0002-fork-rusty-kaspa-bridge.md` — vendoring rationale
