# Runbook 21 — Phase 9 resilience drills (pre-cutover gate)

The `docs/cutover-plan.md` gate "Phase 9 acceptance" before mainnet cutover.
These drills produce the evidence; each is run by the operator and the result
recorded (a dated note under `docs/security-events/` or the cutover checklist).
None can be faked — they require real execution over real time.

## 1. DR validation (ADR-0009)

Automated by `ops/dr/dr-validate.sh`: dumps the source DB, restores it into a
scratch DB, and reconciles (schema completeness + core tables non-empty +
referential integrity). It publishes `dr_validator_*` to VictoriaMetrics, which
drives `DRValidatorMissed` / `DRValidatorFailed`.

```sh
export KATPOOL_DR_SOURCE_URL=postgres://...        # a read replica is ideal
export KATPOOL_DR_SCRATCH_URL=postgres://.../katpool_dr_scratch
export KATPOOL_DR_VMAUTH_URL=https://vmauth-...up.railway.app
export KATPOOL_DR_VMAUTH_USER=... KATPOOL_DR_VMAUTH_PASSWORD=...
export KATPOOL_DR_NETWORK=mainnet
ops/dr/dr-validate.sh
```

Schedule weekly (systemd timer or Railway cron). **Gate: 4 consecutive weekly
passes** (`cutover-plan.md`). Restore failures → Runbook 10 + Runbook 04.

## 2. Chaos drills

Use the `katpool-fault-injection` crate's chaos suite + manual fault injection.
For each, confirm the pool degrades safely and the matching alert fires:

| Fault | Inject | Expected |
|---|---|---|
| kaspad gRPC down | stop the kaspad container/service | `/ready` flips false (`ApiReadinessProbeFailing`); no panics; recovers on restart |
| Postgres down | stop Postgres | writes back-pressure, no data loss; `AccountantShareWritesStalled` after the window |
| Indexer/price down | block egress to kasplex/coingecko | KRC-20 quote circuit-breaker opens, cycle skips (never wrong price); `IndexerDependencyDown` |
| Network partition origin↔edge | drop fly egress at the origin nftables | miners fail over to another region; `StratumEdgeUnreachable` if total loss |

Confirm clean shutdown drains the event backlog (A2) and restart is idempotent
(no double-pay — payout idempotency keys).

## 3. Custody EPERM suite

The treasury account is `nologin`; the systemd unit denies privileged syscalls
(`SystemCallErrorNumber=EPERM`). Confirm the hardening holds: attempts to read
the key from another account, or privileged syscalls from the pool process, are
denied. Run `systemd-analyze security katpool-<network>` and record the score.

## 4. On-call paging dry-run

- **Last mile** (phone): `ops/dr/oncall-paging-test.sh` publishes a test page to
  ntfy; confirm it reaches the on-call phone at `urgent` priority.
- **Full path** (Alertmanager → bridge → ntfy): from inside Railway (Alertmanager
  is not public), fire a synthetic alert and confirm it pages:

  ```sh
  # in the alertmanager service shell (railway):
  amtool alert add DrillAlert severity=page \
    --alertmanager.url=http://localhost:9093 \
    summary="paging drill — not real"
  # or: curl -XPOST localhost:9093/api/v2/alerts -H 'Content-Type: application/json' \
  #   -d '[{"labels":{"alertname":"DrillAlert","severity":"page"}}]'
  ```

## 5. Load test

Stratum edge load + failover is in `ops/edge/flyio/README.md`
("Load + failover test"). Additionally confirm the API rate limit + BFF fan-out
hold under concurrent dashboard load, and that share-accept latency
(`katpool:share_accept_latency:p99_5m`) stays within budget.

## 6. All-runbooks sign-off

Walk every runbook (00–20) and confirm each is current and executable. Record
sign-off per runbook. Any runbook that didn't match reality during a drill is
fixed **before** sign-off — the runbook exists for the next person.

## Acceptance

Phase 9 passes when: 4 consecutive DR passes recorded, all chaos drills degrade
safely with the right alerts, the custody suite denies as designed, both paging
paths reach the phone, the load test meets budget, and all runbooks are signed
off. Record the bundle in `docs/security-events/` and tick the `cutover-plan.md`
gate.
