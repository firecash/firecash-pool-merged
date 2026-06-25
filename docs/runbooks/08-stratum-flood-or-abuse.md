# Runbook 08 — Stratum flood / abuse

## Symptom

Alert `StratumAbuse` fires when
`rate(stratum_connections_rejected_total[5m]) > 100`. Indicates a
flood of malformed-frame attempts, connection storms from a small
set of IPs, or other abuse patterns.

May also manifest as elevated CPU on the bridge or as legitimate
miners failing to connect because Railway edge or our nft layer
is dropping them under load.

Treat as SEV-2; SEV-1 if legitimate miners are visibly affected.

## Confirm

```bash
# Top offending source IPs in the last 10 minutes
ssh prod-vps journalctl -u katpool --since '10 min ago' | \
  grep -E 'stratum_connection_rejected' | \
  jq -r '.client_ip' | sort | uniq -c | sort -rn | head -20

# fail2ban current bans
ssh prod-vps fail2ban-client status stratum

# Connection rates per edge region (from Railway)
# (use the observability dashboard - panel: "stratum incoming by source")
```

## Diagnose

- [ ] **Real DDoS?** Concentrated traffic from many small sources
      = botnet pattern. Railway edge absorbs most of it; if it's
      reaching us, the volume exceeded edge capacity.
- [ ] **Single misbehaving miner?** One IP hammering with
      malformed frames or rapidly reconnecting. Easier to mitigate.
- [ ] **Legitimate miner with broken mining software?** Same
      symptoms as abuse but unintentional. Telegram the operator
      of that wallet address if known.
- [ ] **Our own change degraded miner-compat?** Recent deploy that
      changed stratum behaviour can cause clients to disconnect-
      and-retry. Roll back if correlation is clear.

## Remediate

### Single IP

```bash
ssh prod-vps fail2ban-client set stratum banip <ip>
# Or via nftables directly
ssh prod-vps nft add element inet katpool stratum_banlist { <ip> }
```

### Many IPs (volume)

1. Tighten per-IP rate limits via runtime config reload (no
   restart needed):

   ```bash
   ssh prod-vps katpool-cli config set anti-abuse.per-ip-conn-cap 4
   ssh prod-vps katpool-cli config set anti-abuse.per-ip-share-rate 200
   ```

2. If still saturating: temporarily restrict by ASN at the Railway
   edge (manual via Railway dashboard custom rules).

### Permanent badness

If a recurring source matches a known scam-recipient address or a
documented adversary, add to the permanent denylist in
`ops/secrets/secrets.sops.yaml` via PR (sops file decrypts the
denylist at boot).

## Verify

- `rate(stratum_connections_rejected_total[5m])` returns to
  baseline (< 10/min)
- Legitimate canary miner is back to credited normally
- `stratum_connections_active` reflects expected miner count

## Post-incident

- If we ran into edge capacity, that's input to whether we need
  Railway Pro for the edge services
- Document the attack pattern in a knowledge-base file under
  `docs/security-events/`
