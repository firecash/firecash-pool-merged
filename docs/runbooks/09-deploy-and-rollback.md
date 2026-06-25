# Runbook 09 — Deploy and rollback

## When to use

- Normal feature/bugfix deploy after a merge to `main`
- Emergency rollback during or just after a problematic deploy
- Manual recovery if the deploy workflow itself misbehaves

## Deploy (signed binary + systemd, per network)

Deploys are driven by [`scripts/deploy.sh`](../../scripts/deploy.sh).
The unified `katpool` binary is **network-agnostic** — the network is
selected at runtime by the kaspad endpoint and the `kaspatest:`/`kaspa:`
address prefix — so the script takes a *deploy-target* flag and routes
the one build to the correct per-network location (symmetric layout):

| Network | Directory | Service | Env file |
|---|---|---|---|
| testnet-10 | `/root/katpool-tn10` | `katpool-tn10.service` | `/etc/katpool/tn10.env` |
| mainnet | `/root/katpool-mainnet` | `katpool-mainnet.service` | `/etc/katpool/mainnet.env` |

Each deploy reads a **host-local** `ops/env/<network>.env`. Only the
`*.env.example` templates are tracked; the real env files (endpoints, DB
creds) are gitignored and never hold the treasury key. On a fresh host,
seed it once: `cp ops/env/<network>.env.example ops/env/<network>.env`
and fill it in.

```bash
# RECOMMENDED — download + cosign-verify the signed release, then install:
sudo scripts/deploy.sh --network mainnet --release v1.2.0

# testnet-10 from source (builds --profile dist, installs unit + env, restarts):
sudo scripts/deploy.sh --network tn10

# install a manually downloaded signed artifact (bundle auto-detected beside it):
sudo scripts/deploy.sh --network mainnet --binary /path/to/katpool
```

The script renders the tracked unit template
[`ops/systemd/katpool.service.in`](../../ops/systemd/katpool.service.in)
to `/etc/systemd/system/katpool-<network>.service`, installs the host-local
`ops/env/<network>.env` to `/etc/katpool/<network>.env`, **backs up** the
existing binary
(`katpool.bak-<timestamp>`, keeping the last 5), installs the new binary,
`daemon-reload`s, restarts, then **waits for `/ready`** (DB-reachable AND
kaspad-synced, on `KATPOOL_HEALTH_CHECK_PORT`/`KATPOOL_API_PORT`). It fails
loudly — retaining the backup — if the service does not come back active or
does not report ready within 30 s.

### Supply-chain verification

`--release <tag>` and `--binary <path>` install a **prebuilt release
artifact**, which is **cosign-verified before install**. Verification is
keyless: the signature, produced by the
[`release.yml`](../../.github/workflows/release.yml) workflow (no long-lived
keys), is checked against that workflow's Sigstore identity and the Rekor
transparency log via
[`scripts/verify-release.sh`](../../scripts/verify-release.sh). A failed or
missing signature **aborts the deploy** before anything is swapped. To verify
an artifact by hand (without deploying):

```bash
scripts/verify-release.sh /path/to/katpool   # bundle: <artifact>.sigstore-bundle.json
```

A locally built binary (the from-source `--network tn10` path) is unsigned and
installed as-is. `--no-verify` exists only for offline/pre-verified flows and
is **not** for routine use.

**Rollback (binary):**

```bash
cp /root/katpool-<network>/katpool.bak-<timestamp> /root/katpool-<network>/katpool
sudo systemctl restart katpool-<network>
```

## How releases are built and signed

A `v*` tag push (or a manual `workflow_dispatch`) runs
[`release.yml`](../../.github/workflows/release.yml), which:

1. Builds a static `x86_64-unknown-linux-musl` binary (`--locked`)
2. Generates a CycloneDX SBOM (`anchore/sbom-action` / syft)
3. Signs the binary **and** the SBOM with `cosign` keyless (OIDC) —
   no long-lived keys — emitting `*.sigstore-bundle.json` bundles
4. Publishes a **draft** GitHub Release carrying the binary, the SBOM,
   and both signature bundles

All third-party actions are pinned by full commit SHA.

To cut a release manually (or just push a `vX.Y.Z` tag):

```bash
gh workflow run release.yml --ref main
```

Publish the drafted release, then deploy it to each VPS with the
verified path above:

```bash
sudo scripts/deploy.sh --network mainnet --release vX.Y.Z
```

> Not yet automated: there is no deploy-on-merge, no deploy PR, and no
> remote structured deploy log. Deploys are operator-initiated on the
> VPS via `scripts/deploy.sh`, which cosign-verifies the artifact and
> gates on `/ready` before declaring success.

## Rollback procedure

A bad deploy is backed out by restoring the previous binary backup
(retained automatically — the last 5 per network) and restarting:

```bash
ls -1t /root/katpool-<network>/katpool.bak-*          # newest first
cp /root/katpool-<network>/katpool.bak-<timestamp> /root/katpool-<network>/katpool
sudo systemctl restart katpool-<network>
```

`deploy.sh` keeps the backup whenever the new binary fails to come
active or ready, so the previous good binary is always on disk after a
failed deploy.

If the database is implicated — e.g. a startup migration moved the
schema somewhere the previous binary can't read — this is a manual
recovery scenario:

1. Stop the pool: `systemctl stop katpool-<network>`
2. Restore the database to just before the failed deploy via pgBackRest
   PITR — see [04](04-postgres-restore-from-backup.md)
3. Restore the previous binary backup (above) and start the service
4. File a SEV-2 incident; do not deploy again until the issue is
   understood

## Verification after deploy or rollback

- `/health` and `/ready` return 200
- `katpool_started_total` counter increments by 1 (proves the
  process actually restarted)
- No alerts firing
- Canary miner shares are being credited
- A test query against the API returns expected data
- Migrations applied or rolled back successfully (verify schema
  version table)

## When NOT to deploy

- During an active incident (unless the deploy is the
  mitigation)
- Within 1 h of a scheduled payout cycle
- Within 24 h of a Kaspa hardfork without explicit testnet
  validation
- Without a passing CI and signed commit on `main`

## Audit trail

Today the record of "who deployed what when" is reconstructed from:

- **journald** — `journalctl -u katpool-<network>` shows each restart
  and the `deploy.sh` output that drove it
- **Binary backups** — `katpool.bak-<timestamp>` (UTC) in the per-network
  directory mark each prior binary and the time it was replaced
- **GitHub Releases** — the published tag, its SBOM, and the Rekor entry
  behind the cosign bundle establish provenance of the deployed artifact

> Future (B-workstream): a structured `deploy.jsonl` shipped to Loki with
> operator identity, before/after artifact SHA-256, and outcome, so the
> audit trail is queryable rather than reconstructed.
