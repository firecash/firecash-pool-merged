# Runbook 00 — On-call overview

What every on-call person needs to know before any specific incident
runbook makes sense.

## Audience

The operator and any future on-call contributor. This file should be
the first thing you read when you join the rotation, and the last
thing you re-read if it's been a while since your last shift.

## What "on call" means here

The pool is single-operator today. "On call" means: when an alert
fires, someone responds. Severity matrix:

| Severity | Definition | Response target |
|---|---|---|
| SEV-1 | Pool fully down OR treasury at active risk | Acknowledge within 15 min, mitigation within 1 h |
| SEV-2 | Payouts blocked, miner-visible degradation | 30 min ack, 4 h mitigation |
| SEV-3 | Single-component degradation, no user impact yet | 4 h ack, mitigation within 24 h |
| SEV-4 | Latent issue identified before impact | next-business-day |

## Communication

| Channel | Use |
|---|---|
| Telegram operator chat | First place to declare an incident |
| GitHub issue (incident template) | Source of truth for incident state |
| Status page (status.katpool.com) | Public-facing updates for SEV-1 and SEV-2 |
| ntfy.sh push | Where alerts arrive |

## First 5 minutes of any incident

1. **Acknowledge the alert** in ntfy/Telegram so others know you're
   on it.
2. **Open an incident issue** using the
   [`incident.md`](../../.github/ISSUE_TEMPLATE/incident.md)
   template. Even if it turns out to be nothing, the timeline is
   captured.
3. **Read the alert's linked runbook** — every alert in
   `alertmanager` embeds a runbook URL. Use it.
4. **Pause deploys** by manually disabling the deploy workflow's
   trigger. Resume after the incident closes.
5. **If treasury is at risk**, escalate to SEV-1 and consider
   pausing payouts (see [05](05-treasury-balance-below-threshold.md)).

## Helpful commands

```bash
# Pool process status
ssh prod-vps systemctl status katpool

# Last 100 lines of pool logs (locally; full logs in Loki)
ssh prod-vps journalctl -u katpool -n 100 --no-pager

# Check kaspad health
ssh prod-vps systemctl status kaspad

# Postgres connection check
ssh prod-vps psql -U katpool -d katpool -c 'SELECT 1'

# Treasury balance (on-chain, not via internal DB)
curl -s 'https://api.kasplex.org/v1/krc20/address/<treasury-address>/token/NACHO'
curl -s 'https://api.kaspa.org/addresses/<treasury-address>/balance'

# Stop / start pool (treat as disruptive)
ssh prod-vps systemctl stop katpool
ssh prod-vps systemctl start katpool
```

## Where to NOT make changes during an incident

- **Do not** modify `Cargo.toml` or anything that triggers a rebuild
  unless the rebuild *is* the mitigation
- **Do not** edit `ops/secrets/secrets.sops.yaml` in haste
- **Do not** run unreviewed SQL against production
- **Do not** push directly to `main`; even SEV-1 fixes go through
  PR with at least one approval after the situation stabilises

## After the incident

- Update the incident issue with the resolution
- Open a postmortem within 48 h using
  [`postmortem.md`](../../.github/ISSUE_TEMPLATE/postmortem.md)
- Add action items as GitHub issues; track them in the project
- Update this runbook (or the specific incident's runbook) with
  anything you learned that future-you would want to know
