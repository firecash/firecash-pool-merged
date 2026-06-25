# Runbook 10 — Automated DR validation

## What this runbook is for

The DR validator is a Railway-hosted cron service that exercises
the backup → restore → reconcile loop every Sunday at 04:00 UTC.
When it succeeds, it's invisible. When it fails or misses a
schedule, alerts fire, and you end up here.

References:
- ADR-0009 (decision): [`decisions/0009-automated-weekly-dr-validation.md`](../decisions/0009-automated-weekly-dr-validation.md)
- ADR-0007 (pgBackRest): [`decisions/0007-pgbackrest-wal-archiving.md`](../decisions/0007-pgbackrest-wal-archiving.md)

## Symptoms

| Alert | What it means |
|---|---|
| `DRValidatorMissed` | No successful validator run has finished in > 10 days |
| `DRValidatorFailed` | The most recent validator run reported a mismatch |

## DRValidatorMissed

```bash
# Check Railway cron service status
gh deployment list --repo Nacho-the-Kat/katpool --environment dr-validator
# Or via the Railway dashboard: katpool-observability → dr-validator service

# Inspect the most recent Railway logs for the service
# (use the Railway dashboard or `railway logs` CLI if configured)
```

Likely causes:

- Railway service paused or crashed
- B2 credentials expired or rotated without updating the service
- pgBackRest version mismatch between primary and validator

Remediate:

- Restart the Railway service
- If credentials issue, rotate per `runbooks/11-key-rotation.md`
  procedure (the B2 keys, not the treasury key)
- Trigger an out-of-band manual run to confirm restoration before
  the next scheduled run

```bash
gh workflow run dr-validator-manual.yml --ref main
```

## DRValidatorFailed

This is the alert that says "your backups don't restore cleanly".
Treat as SEV-2.

```bash
# Pull the validator's last-run log from Loki
# (use Grafana / Loki query: {service="dr-validator"} |= "FAIL")

# Or directly from Railway:
# Dashboard → katpool-observability → dr-validator → Logs
```

The log will identify which reconciliation step failed:

- `pgbackrest restore` itself failed → backup corruption.
  Examine the file integrity in B2:

  ```bash
  pgbackrest --stanza=katpool check
  ```

- `REINDEX DATABASE` failed → structural corruption. Investigate
  primary too (it may also be corrupted; pgBackRest base might
  have been taken from a corrupt state).
- Sum-of-balances mismatch → the validator's reference value is
  stale, OR there's a real divergence. Compare both to a fresh
  query on the primary.
- Recent-block timestamp out of range → there is a gap in WAL
  archive shipping. Check `pgbackrest_archive_lag` metric.

## Remediate (DRValidatorFailed)

1. **Pause the pool's payout cycles**:

   ```bash
   ssh prod-vps systemctl mask katpool-payout-kas.timer katpool-payout-krc20.timer
   ```

2. **Run a manual full backup** on the primary so we have a
   known-good snapshot of the current state:

   ```bash
   ssh prod-vps sudo -u postgres pgbackrest --stanza=katpool \
     --type=full --log-level-console=info backup
   ```

3. **Run the validator manually** against the new backup. If it
   passes, the previous failure was a transient WAL-shipping issue.

4. If it still fails, you are in **lost-backup-window territory**.
   File a SEV-1 incident, contact the operator, restore the most
   recent known-good snapshot to a staging environment, and
   reconcile drift manually.

5. **Unmask the payout timers** once DR validation is green
   again:

   ```bash
   ssh prod-vps systemctl unmask katpool-payout-kas.timer katpool-payout-krc20.timer
   ```

## Verify

- A new validator run completes successfully and the Grafana
  panel "Last successful DR drill" shows < 1 h ago
- `pgbackrest_archive_lag` < 1 minute steadily

## Post-incident

- Postmortem required for any `DRValidatorFailed` — the alert
  exists because backup silence kills companies
- Update the validator's reference values (sum-of-balances) if
  the previous reference was stale
- If the failure indicated a backup-tooling bug, file an upstream
  issue with pgBackRest
