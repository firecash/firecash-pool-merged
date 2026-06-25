# fail2ban jails (origin host)

OS-level abuse banning for the pool VPS, complementing the in-process controls.
These are an **operational backstop**, not the primary defense:

- **API abuse** — the read-only API's per-IP `tower-governor` limiter (ADR-0021)
  already returns `429` cheaply. The `katpool-api-4xx` jail bans a client that
  ignores the limit and keeps generating 4xx, so it stops reaching the box.
- **Stratum abuse** — handled in-process by the per-IP anti-abuse guards
  (`ks_anti_abuse_*` metrics) and surfaced by the `StratumAbuseBurst` alert.
  fail2ban is **not** used for stratum here: the pool does not log per-IP reject
  lines at `info`, so there is no stable log target to match (don't guess one).
- **SSH** — keys-only per ADR-0008 / `custody.md`; the `sshd` jail bans
  password-guess / scanner bursts.

Bans use **nftables**, which the origin already runs for the stratum firewall
(`ops/edge/flyio/nftables/`).

## Install

```sh
sudo cp ops/security/fail2ban/filter.d/*.conf /etc/fail2ban/filter.d/
sudo cp ops/security/fail2ban/jail.d/katpool.conf /etc/fail2ban/jail.d/
# Add the dashboard BFF / anycast edge egress IP(s) to `ignoreip` in the jail so
# the same-origin BFF (one source IP) can never be collateral, then:
sudo systemctl reload fail2ban
sudo fail2ban-client status katpool-api-4xx
```

## Tuning

- `logpath` must point at the host nginx access log fronting
  `api-<network>.katpool.com` (default `/var/log/nginx/access.log`).
- `maxretry`/`findtime` are sized so legitimate 404s or one misbehaving page do
  not trip the jail — only a sustained 4xx burst from a single non-ignored IP
  does. Raise `maxretry` if a busy edge IP is wrongly banned (or add it to
  `ignoreip`).
- Verify the IP capture against a real log sample:
  `fail2ban-regex /var/log/nginx/access.log ops/security/fail2ban/filter.d/katpool-api-4xx.conf`.
