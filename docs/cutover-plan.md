# Cutover Plan

> **Executed (2026-06).** Mainnet cutover is complete; reconcile green;
> legacy pool shut down. The importer and replay operator binaries were
> retired from the repo post-sign-off (evidence under `cutover-evidence/`
> and `replay-evidence/`). This document is kept as the historical record.

The procedure for switching production from the legacy
`Nacho-the-Kat/katpool-app` stack to this rebuild. Detailed, exact,
revisable. Anything ambiguous here is a finding to fix before T-24h.

> **Stratum endpoint/edge mapping**: the connection-compatibility
> matrix (all 7 regional hostnames × 8 ports, the per-port difficulty
> seeds, and the fly.io anycast edge that supersedes the Railway edge in
> ADR-0005) lives in
> [`cutover-stratum-compatibility.md`](cutover-stratum-compatibility.md).
> The DNS step in §1 (T-0) below is revised by that document.

## 0. Pre-cutover gates

All of these must be true before scheduling cutover:

- [ ] Phase 9 acceptance criteria met (load test, chaos drills, all 11
      runbooks signed off, on-call paging dry-run successful)
- [ ] Automated DR validator has succeeded 4 consecutive weekly cycles
- [ ] Shadow accountant balances match production within 0 sompi for
      the last 72 h continuously
- [ ] All Phase 0–8 milestones merged to `main` with signed commits
      and green CI
- [ ] Rollback procedure rehearsed end-to-end on a non-production VPS

## 1. Timeline

`T` is the cutover instant — when DNS flips and miners start
connecting to the new pool.

### T-7d: Announcement

- Post on Telegram channel: scheduled maintenance window, expected
  duration < 5 minutes user-visible
- Update status.katpool.com banner with cutover date and time
- Email operators of largest miners (if any) directly
- Update kas.katpool.com landing page with the window

### T-72h: Long shadow run starts

- Bring up new pool on a separate environment (could be same VPS
  with separate ports + DB schema)
- Connect to the same production kaspad over wRPC (read-only —
  shadow does not submit blocks)
- New accountant subscribes to the same bridge event firehose as
  production via a fan-out splitter
- Shadow accountant writes to `katpool_shadow` schema
- Run continuous balance reconciliation: every 5 min compare
  shadow's `miners_balance` deltas to production's. Any divergence
  > 0 sompi pauses the cutover and triggers investigation.

### T-1h: Final-state capture

- `pg_dump` production DB → `katpool_production_pre_cutover_<ts>.sql.gz`
- Upload to B2 with object-lock and a 90-day retention.
- Snapshot current treasury balances via `api.kasplex.org` for
  KAS and NACHO, store in `cutover-evidence/`.
- Snapshot current pool's `block_details` count and most recent
  hash.
- Pre-lower DNS TTL to 60 s (done 24 h earlier; verified here).

### T-30m: Connection freeze on legacy pool

- Add `nft`-level filter on legacy pool VPS to refuse NEW stratum
  connections; ESTABLISHED connections continue to be served.
- Telegram bot announces "Pool entering maintenance — existing
  miners can finish their current work unit. New connections
  refused; reconnect after the maintenance window."
- Watch `katpool-app` log for "active session count" — wait until
  most sessions have naturally idled out or until T-5m hard cut.

### T-5m: Final legacy payout (if scheduled)

- Trigger legacy `katpool-payment` for one final pass. Verify the
  cycle completes (cutover does not wait for it; we'll reconcile
  any straggler in post-cutover).

### T-2m: Legacy stop + reconcile

- `docker compose stop katpool-app go-app katpool-payment
  katpool-monitor katpool-backup` on the legacy VPS. **Do not
  remove containers** — keep the legacy stack intact for
  rollback.
- **Importer hot-run.** The operator ran `katpool-import-legacy` with
  reconcile green; artefacts archived under `cutover-evidence/` (the
  one-shot tool was retired from the repo post-cutover).
- **Gate:** `manifest.reconcile_all_passed == "true"` and
  importer exit code 0. Anything else aborts cutover; revert to
  rollback procedure below.

### T-1m: Treasury key handover

- Operator transfers the sops-encrypted treasury secret to the new
  VPS via `scp` from the operator's workstation:
  - `scp ~/.age/katpool-prod.recipient.txt katpool@new-vps:/etc/katpool/`
    (recipient file only, used by systemd LoadCredentialEncrypted)
  - The encrypted secret file `ops/secrets/secrets.sops.yaml` is
    already deployed alongside the binary; systemd handles
    decryption at boot.
- Operator does **not** type or copy-paste the private key bytes
  themselves into any chat, terminal, or file.

### T-0: DNS flip + start

- Flip DNS to the **fly.io anycast edge** (ADR-0022, which supersedes the
  Railway TCP edge of ADR-0005). A single anycast IPv4 routes each miner to the
  nearest fly region, which forwards to the NetCup origin over PROXY-v2:
  - `kas.katpool.com` → A record → fly.io anycast IPv4 → (PROXY-v2) → NetCup VPS
  - (No per-region CNAMEs: anycast collapses us-east / eu-west / ap-southeast
    into one address; regional pop selection is handled by fly's network.)
  - Origin firewall must already accept only fly egress IPs on the stratum
    ports (`ops/edge/flyio/nftables/`, workstream C2).
- `systemctl start katpool` on the new VPS.
- Watch logs: expect "katpool started OK", first stratum
  connection within seconds, first share within ~30 s, first
  block found within typical block interval.

### T+5m: Verification

- [ ] Liveness probe green
- [ ] Readiness probe green (kaspad synced, DB reachable)
- [ ] Stratum accepting shares (verify in `bridge` metrics)
- [ ] Canary miner credited within 5 min of cutover
- [ ] No alerts firing
- [ ] Datadog and Loki receiving logs (sanity check on log volume)

### T+1h: Out of dry-run

- Switch `payout-kas` and `payout-krc20` from dry-run to live mode
  via config reload (`systemctl reload katpool`).
- First scheduled payout cycle runs at the configured cron time.

### T+24h: Post-cutover review

- Compare 24h-of-shares allocations between new pool and what the
  legacy pool would have produced for the same period (via shadow
  trace).
- File a postmortem-style cutover report covering: what worked,
  what was unexpected, action items.

### T+7d: End rollback window

- Keep legacy pool containers stopped but present for 7 days. If
  no need to roll back, decommission the legacy stack (preserve
  the final pg_dump in B2 for historical record).

## 2. Rollback procedure

Trigger conditions:

- Balance reconciliation fails after T-2m (abort before T-0)
- Health probes red for > 5 min post-T
- > 1% of miner population fails to reconnect within 30 min
- Any treasury-related error detected
- Operator's call: anything that looks materially off

Steps (well-rehearsed; expected duration < 10 min):

1. Flip DNS back to the legacy VPS
2. `systemctl stop katpool` on the new VPS
3. `docker compose start katpool-app go-app katpool-payment
   katpool-monitor katpool-backup` on the legacy VPS
4. Verify legacy pool is accepting shares again
5. Run reconciliation: any balance deltas the new pool accrued
   between T-0 and rollback are imported back into the legacy DB
   via `migration/scripts/reconcile.rs --reverse`
6. Telegram + status page notification: "Rolled back to previous
   pool. No funds affected. New deployment will be re-attempted
   after root-cause analysis."
7. File a postmortem within 48 h. Do not re-attempt cutover until
   the postmortem has signed-off action items merged.

## 3. Communications template

### Pre-cutover announcement

> **Scheduled maintenance — [YYYY-MM-DD] [HH:MM] UTC**
>
> The pool will be briefly unavailable while we deploy a major
> rebuild. Expected impact: < 5 minutes of stratum unavailability.
> Active miners will be allowed to complete their current work unit
> before the cut. No fund impact; all accrued balances will be
> preserved exactly.
>
> If your mining software auto-reconnects (most do), you don't need
> to do anything. Otherwise, please reconnect after the window.

### During-cutover (in-progress)

> **Maintenance in progress.**
>
> Stage: [T-30m: connection freeze | T-2m: shutdown | T-0: DNS
> flip | T+5m: verification].
> Status: green. Next update at T+[15m].

### Success

> **Cutover complete.** New pool live. All balances preserved.
> Status page green. Any issues, please reply or open a GitHub
> Discussion.

### Rollback

> **Rolled back.** Cutover encountered [brief description]; we
> reverted to the previous deployment without fund impact. A
> postmortem will be published within 48 h. Next cutover attempt
> will be scheduled separately.

## 4. Decision rights

The operator is the sole authority on:

- Go/no-go at each gate
- Triggering rollback
- Scheduling re-attempts
- Communications wording

This is appropriate for a single-operator pool. A future expansion
to multi-operator governance is a separate ADR.
