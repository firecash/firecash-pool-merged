# Runbook 11 — Treasury key rotation

## When to rotate

- **Quarterly drill** — a no-impact rotation onto a fresh key to
  prove the rotation works
- **On suspected compromise** — any indication that the production
  age private key, the encrypted sops file, or the running pool's
  process memory may have been exposed
- **On operator change** — if the human-operator handling the age
  key changes
- **On audit recommendation** — per any post-incident finding

Treat real rotation (not the drill) as SEV-2 minimum.

## Automated key↔address audit (continuous)

The `katpool-treasury-audit-<network>.timer` runs `katpool treasury audit`
hourly (installed by `scripts/deploy.sh`). It is **read-only and offline**:
loads the treasury key, derives its schnorr P2PK address, and compares it to
`KATPOOL_POOL_ADDRESS`. It moves no funds and signs nothing for broadcast.

- **Pass** → logs `result=ok` and exits 0.
- **Mismatch** → logs a structured `ERROR` (`result=mismatch`,
  `"TREASURY KEY AUDIT FAILED"`) and exits non-zero, so the unit enters
  `failed` and the line ships to Loki. This means the running key does **not**
  control the configured treasury address — a botched rotation, a
  misconfiguration, or a possible compromise. Treat as SEV-2: stop payouts, do
  not deploy, and reconcile the key/address before resuming.

Run it on demand at any time (e.g. Phase D below): `katpool treasury audit`.

Check the timer and last result:

```sh
systemctl status katpool-treasury-audit-<network>.timer
journalctl -u katpool-treasury-audit-<network>.service -n 20
```

**Alerting hook:** page on the mismatch either via a Loki ruler rule matching
`result=mismatch` on `{job="katpool"}`, or a systemd `OnFailure=` notify
drop-in on the service. (Wiring the Loki rule is a follow-up; the audit trail
and `failed` unit state exist now.)

## Prerequisites

- Operator's workstation has both the current age private key and
  a fresh new age key
- Operator has the offline-backup material in hand (encrypted USB
  + paper printout)
- A scheduled maintenance window if this is a real rotation (the
  pool will be briefly unavailable; ASIC miners auto-reconnect)

## Quarterly drill (no production impact)

The drill rotates onto a *staging* key and back to the original.
No funds move; no real key is exposed.

```bash
# Generate a staging age key locally
age-keygen -o ~/.age/katpool-rotation-drill-$(date +%Y%m%d).key

# Re-encrypt the sops file with both recipients (current + drill)
sops --add-age "$(age-keygen -y ~/.age/katpool-rotation-drill-*.key)" \
  ops/secrets/secrets.sops.yaml

# Confirm both recipients can decrypt
SOPS_AGE_KEY_FILE=~/.age/katpool-prod.key sops -d ops/secrets/secrets.sops.yaml > /dev/null
SOPS_AGE_KEY_FILE=~/.age/katpool-rotation-drill-*.key sops -d ops/secrets/secrets.sops.yaml > /dev/null

# Remove the drill recipient
sops --rm-age "$(age-keygen -y ~/.age/katpool-rotation-drill-*.key)" \
  ops/secrets/secrets.sops.yaml

# Verify drill key no longer decrypts; production key still does
```

Drill outcome documented in a quarterly note under
`docs/security-events/` (file ad-hoc; structure is not formal).

## Real rotation procedure

Phase A — pre-rotation
1. Notify operator(s); set a maintenance window (~30 min)
2. Lower DNS TTL to 60 s, 24 h ahead
3. Run a full pgBackRest base backup; verify upload to B2
4. Snapshot on-chain treasury balances (KAS + NACHO)
5. Confirm the offline-backup material is accessible

Phase B — generate new wallet
1. **Offline** (airgapped laptop or hardware token):
   - Generate a new Kaspa private key
   - Derive the new treasury address
   - Print the BIP39 mnemonic (or hex bytes) to paper; seal
   - Encrypt the key bytes into a fresh sops file
2. Bring **only the sops-encrypted file** back to the connected
   workstation
3. Commit the encrypted file to git in a *separate branch*

Phase C — drain existing treasury
1. Send all KAS from the old treasury to the new address (one
   on-chain tx, verified)
2. Send all NACHO from the old treasury to the new address (one
   KRC-20 commit/reveal, verified)
3. Record both tx hashes in the operator's offline log

Phase D — deploy new key
1. Maintenance window: `systemctl stop katpool`
2. Replace `ops/secrets/secrets.sops.yaml` with the new file
3. Update the pool's configured treasury address in `config.toml`
4. Bring the pool up: `systemctl start katpool`
5. Verify the new wallet receives a coinbase reward within
   normal block intervals
6. Trigger a payout cycle in dry-run mode against the new key;
   confirm the signing flow works

Phase E — old key handling
1. The old age private key is destroyed: wipe from the
   workstation keyring, destroy the offline copy
2. The old paper mnemonic is destroyed: cross-cut shred
3. The old treasury address is documented as decommissioned in
   `docs/security-events/`

## Verify

- New treasury address shows incoming coinbases on-chain
- A test payout from the new key reaches a known recipient
  address (e.g. canary miner)
- The old address shows zero balance (or near-zero — there may
  be a small change-output residue)
- Pool process is healthy; no alerts firing

## Post-rotation

- Postmortem if this was a compromise-driven rotation
- Update the offline-backup material location records
- Schedule the next quarterly drill
- If anything in this runbook didn't go smoothly, fix it here
  before the next time
