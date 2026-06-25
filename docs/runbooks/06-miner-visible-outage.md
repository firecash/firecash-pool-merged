# Runbook 06 — Miner-visible outage

## Symptom

Alert `HealthEndpointDown` fires when the Blackbox Exporter's
probe of `/health` on the public API returns non-200 (or no
response). May also be detected by a sudden drop in
`stratum_connections_active` or by miner reports.

Treat as SEV-1 — by definition, miners can see this.

## Confirm

```bash
# From an external host (not the pool VPS itself)
curl -fsS https://kas.katpool.com/health
curl -fsS https://kas.katpool.com/ready

# Public stratum endpoint
nc -zv kas.katpool.com 3333    # primary stratum port
nc -zv kas.katpool.com 5555
nc -zv kas.katpool.com 8888
```

If those fail, identify the layer:

```bash
# From the pool VPS — does the local API respond?
ssh prod-vps curl -fsS http://127.0.0.1/health

# Is nginx up?
ssh prod-vps systemctl is-active nginx

# Is the pool process up?
ssh prod-vps systemctl is-active katpool

# Are Railway edges responsive?
nc -zv us-east.proxy.rlwy.net <port>
nc -zv eu-west.proxy.rlwy.net <port>
```

## Diagnose

The failure is somewhere on the path:

```text
miner → DNS → Railway edge → public internet → NetCup VPS → nginx → katpool
```

Walk the path and identify the first broken hop.

- If DNS fails → check DNS provider status; restore the CNAME
- If Railway edge fails → check Railway dashboard; failover to
  another region by adjusting DNS
- If NetCup network fails → check NetCup status; this is
  provider-level
- If nginx down → restart it; check `nginx -t` for config errors
  (recent change?)
- If katpool process down → see
  [09 (deploy/rollback)](09-deploy-and-rollback.md)

## Remediate

In order of disruption:

1. Whichever layer is broken, restart it
2. If a recent deploy or config change correlates, roll it back
3. If a Railway region is down with no ETA, lower the DNS TTL for
   the affected region's CNAME and point it to another region's
   edge

## Verify

- Public `/health` and `/ready` return 200
- Stratum TCP connects from at least two external regions
- Canary miner reports shares being credited again
- `stratum_connections_active` recovers within 5 min

## Post-incident

- File a SEV-1 postmortem within 48 h
- If a single Railway region's failure caused this, document why
  the multi-region setup didn't seamlessly fail over and fix the
  configuration
