# Runbook 03 — kaspad lost peers

## Symptom

Alert `KaspadPeerCountLow` fires when `kaspad_peers < 5` for 5
minutes. A kaspad with too few peers cannot reliably propagate
blocks or keep up with the chain tip.

## Confirm

```bash
ssh prod-vps 'docker logs --tail 50 kaspad' | grep -E 'peers|handshake'
ssh prod-vps 'docker exec kaspad kaspactl GetConnectedPeerInfo' 2>&1 || true
```

Look for: protocol version mismatches (a known issue around
hardforks), connection refused, or local firewall changes.

## Diagnose

- [ ] **Protocol version mismatch?** e.g. `P2P protocol version
      mismatch - local: 7, remote: 5`. This indicates a network
      transition; usually self-resolving. Confirm we're on the
      current protocol version per the latest rusty-kaspa release.
- [ ] **Firewall changed?** Verify nftables ruleset allows
      outbound to `:16111` (P2P) and inbound on the same.
- [ ] **DNS issue?** kaspad bootstraps via known seeders; check
      DNS resolution from inside the container.
- [ ] **Network outage at the provider?** Check NetCup status
      page.

## Remediate

1. **Restart kaspad**: `systemctl restart kaspad` (or via Docker
   if running standalone). Often the simplest fix; rejoins the
   network from scratch.
2. If protocol version mismatch and we're behind a hardfork: bump
   to the current rusty-kaspa version (this is a PR + deploy, not
   an in-the-moment patch).
3. If a specific peer is misbehaving (rare), the cleanest path is
   restart.

## Verify

- `kaspad_peers >= 10` sustained for 15 min
- New blocks being relayed (visible in kaspad logs as `Accepted N
  blocks ... via relay`)

## Post-incident

- Postmortem if outage > 30 min
- If protocol upgrade required, file a separate issue tracking the
  upgrade work
