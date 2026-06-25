# Runbooks

One runbook per incident class. Each runbook is the on-call's first
stop when the corresponding alert fires.

| Runbook | Topic | Alert(s) |
|---|---|---|
| [00](00-on-call-overview.md) | On-call overview, comms, first 5 minutes | — |
| [01](01-blocks-stopped-being-found.md) | Blocks stopped being found | `BlocksNotFound` |
| [02](02-nacho-payout-failed.md) | NACHO payout failed | `NachoPayoutFailed` |
| [03](03-kaspad-lost-peers.md) | kaspad lost peers | `KaspadPeerCountLow` |
| [04](04-postgres-restore-from-backup.md) | Postgres restore from backup | manual, or chained from a DR-validator failure |
| [05](05-treasury-balance-below-threshold.md) | Treasury balance below threshold | `TreasuryBalanceLow` |
| [06](06-miner-visible-outage.md) | Miner-visible outage | `HealthEndpointDown` |
| [07](07-storage-mass-rejection-burst.md) | Storage-mass rejection burst | `StorageMassRejectionBurst` |
| [08](08-stratum-flood-or-abuse.md) | Stratum flood / abuse | `StratumAbuse` |
| [09](09-deploy-and-rollback.md) | Deploy and rollback | — (manual procedure) |
| [10](10-automated-dr-validation.md) | Automated DR validation | `DRValidatorMissed`, `DRValidatorFailed` |
| [11](11-key-rotation.md) | Treasury key rotation | — (scheduled drill or compromise response) |
| [12](12-testnet10-smoke.md) | testnet-10 bridge smoke | — (release-candidate acceptance) |
| [13](13-kaspad-tn10-bootstrap.md) | kaspad-tn10 install / upgrade / recover | — (ops procedure) |
| [15](15-testnet10-tracker-live.md) | Phase 3 M3c — maturity-tracker live exercise on testnet-10 | — (acceptance procedure) |
| [16](16-testnet10-full-pipeline-live.md) | Phase 3 M3d — full mine-and-allocate pipeline live exercise on testnet-10 | — (acceptance procedure with ASIC) |
| [20](20-kaspa-version-bump.md) | Kaspa/kaspad version bump (kaspad + kaspa-* crates + toolchain) | — (ops procedure) |
| [22](22-cutover-execution.md) | Mainnet cutover execution (shadow run, importer hot-run, DNS flip, rollback) | — (one-shot cutover procedure) |
| [21](21-resilience-drills.md) | Phase 9 resilience drills (DR validation, chaos, custody, on-call, load) | `DRValidatorMissed`, `DRValidatorFailed` |

Each runbook follows the same structure: Symptom → Confirm → Diagnose
→ Remediate → Verify → Post-incident. If a runbook deviates from this
shape, that's the runbook's fault, not yours.

Runbooks evolve. If you ran one during an incident and something was
wrong, missing, or stale, **the next thing you do after the incident
is open a PR fixing it** — before the postmortem, even. The runbook
exists for the next person.
