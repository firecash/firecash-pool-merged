# Runbook 13 — bootstrap & maintain `katpool-kaspad-tn10`

The Toccata-aware testnet-10 node used by the Phase 1 acceptance
smoke (see [runbook 12](12-testnet10-smoke.md)) and any future
testnet-only validation work. Co-resident with the existing mainnet
kaspad — see [ADR-0010](../decisions/0010-multi-tenant-kaspad-on-pool-vps.md)
for the rationale.

## Symptom

Used in three situations:

1. **Initial install.** First time this VPS needs a testnet-10 node.
2. **Upstream maintenance release.** kaspanet/rusty-kaspa ships a new
   `tn10-toc{N}` (or eventually a mainline that supersedes Toccata).
3. **Incident recovery.** The service died, the data dir was
   corrupted, or the host was rebuilt.

## Confirm

Quick state check:

```bash
sudo systemctl status katpool-kaspad-tn10 --no-pager
ss -ltnp | grep -E ':1[678]2[01]0'
journalctl -u katpool-kaspad-tn10 --since "5 min ago" --no-pager | tail -20
```

Expected at steady state: `active (running)`, ports 16210/16211/
17210/18210 all bound, last log line within the last few seconds
mentioning either `Processed N blocks` (during IBD) or `Accepted N
blocks ... via relay` (in sync).

## Diagnose

| Symptom | Likely cause | Next step |
|---|---|---|
| `Failed to start` with `kaspad-tn10: not found` | Installer never ran | Run the install procedure below |
| `Failed to start` with capability errors | Hardening profile too strict for the binary on this host | `journalctl -u katpool-kaspad-tn10` will show the offending syscall; consider relaxing one specific `SystemCallFilter=` entry in a drop-in |
| IBD stalls — no `Processed N blocks` for > 60 s | Peer list exhausted or peer P2P churn | `journalctl` will show `Disconnected from peer`; let it self-heal for 5 min, then restart if no recovery |
| `Processed 0 blocks` repeatedly | UTXO commitment mismatch or fork ahead of our build | Confirm we are on the pinned `tn10-toc{N}` release; if upstream released a newer tag, bump per "Upgrade" below |
| Out-of-disk on `/var/lib/kaspad-tn10` | Testnet-10 grew faster than capacity-plan estimate | `df -h /`; if pruning is disabled (default), enable `--archival=false --pruning-window=N` via drop-in |

## Remediate

### Initial install (or re-install after host wipe)

```bash
cd /root/katpool   # or wherever the repo lives on the deployment host
sudo ops/kaspad/install-kaspad-tn10.sh
sudo systemctl enable --now katpool-kaspad-tn10
journalctl -fu katpool-kaspad-tn10
```

Watch for the IBD progression. Expect:

1. `Starting IBD with headers proof with peer X` within ~10 s of
   start.
2. `Processed N blocks and N headers in the last 10.00s` ticking
   every 10 s through IBD. **Wall time on this VPS: ~4 hours from
   cold start to live tip** (measured 2026-05-26; ~22:16 EDT start,
   ~02:25 EDT first relay block). Earlier drafts of this runbook
   estimated 30–60 minutes — that turned out to be the headers-proof
   phase only; the UTXO-validation phase that follows takes the bulk
   of the time. Plan IBD windows accordingly (e.g. start it overnight
   if you need the node available for a morning operation).
3. `Accepted N blocks ... via relay` once IBD finishes and the node
   is following live tip. Steady-state arrival rate is 10–15 blocks
   per ~1 s batch, matching the post-Crescendo 10 BPS network rate.
   This is the discriminator the smoke script's pre-flight check
   keys on (see [runbook 12](12-testnet10-smoke.md)).

### Upgrade to a newer upstream release (e.g. `tn10-toc3`)

The pinned release tag and SHA-256 are two constants near the top of
`ops/kaspad/install-kaspad-tn10.sh`. To upgrade:

1. Find the new asset's SHA-256:

   ```bash
   curl -sSL -o /tmp/x.zip \
     https://github.com/kaspanet/rusty-kaspa/releases/download/<new-tag>/rusty-kaspa-<new-tag>-linux-amd64.zip
   sha256sum /tmp/x.zip
   ```

2. Open a PR that bumps `TN10_RELEASE_TAG` and `TN10_LINUX_SHA256`.
   No other code change is required for a kaspad-only update.

3. After merge, on the deployment host:

   ```bash
   sudo ops/kaspad/install-kaspad-tn10.sh
   sudo systemctl restart katpool-kaspad-tn10
   journalctl -fu katpool-kaspad-tn10
   ```

   Watch for the restart to complete and IBD to confirm caught-up
   (it should not need to re-sync — the data format is stable across
   tn10-toc{N} releases unless explicitly called out in the release
   notes).

### Incident recovery (corrupted data dir)

```bash
sudo systemctl stop katpool-kaspad-tn10
sudo rm -rf /var/lib/kaspad-tn10/data
sudo systemctl start katpool-kaspad-tn10   # IBD from genesis (~30-60 min)
```

Testnet data is *not* backed up via pgBackRest (see ADR-0007) and we
explicitly accept the re-sync cost.

## Verify

After install or upgrade:

- `sudo systemctl is-active katpool-kaspad-tn10` returns `active`.
- `systemd-analyze security katpool-kaspad-tn10` reports ≤ 2.5 OK.
- `ss -ltnp | grep ':16210'` shows the gRPC port bound.
- `journalctl -u katpool-kaspad-tn10 -n 50` shows recent
  `Accepted N blocks ... via relay` lines (steady state).
- `curl -s http://127.0.0.1:2114/metrics | grep ks_` shows
  bridge-side metrics non-zero **only after** the Phase 1 smoke has
  driven traffic through it (the smoke is runbook 12; the kaspad
  itself doesn't expose Prometheus).

## Post-incident

If this runbook fired because of a kaspad upgrade-related issue,
record in the `Run history` block at the top of
[`docs/phase-1-acceptance.md`](../phase-1-acceptance.md) which
upstream tag was attempted, what went wrong, and how we recovered.
That history is what a future operator will read before bumping the
next time.
