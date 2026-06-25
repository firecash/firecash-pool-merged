# Architectural Decision Records

Every architecturally-significant decision is captured as a Markdown
file in this directory in [MADR 4.0](https://adr.github.io/madr/)
format. The numbering is monotonic — once an ADR is accepted, its
number never moves. If a decision is later superseded, mark the
original's `status` as `superseded by [ADR-NNNN]` and write a new
ADR pointing back.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-rust-first.md) | Rust as primary language | accepted |
| [0002](0002-fork-rusty-kaspa-bridge.md) | Fork rusty-kaspa v1.1.0 bridge | accepted |
| [0003](0003-sops-only-treasury-custody.md) | sops + age for treasury secrets at rest | accepted |
| [0004](0004-self-host-observability.md) | Self-host Grafana LGTM on Railway | accepted |
| [0005](0005-netcup-vps-railway-edge.md) | Stay on NetCup VPS; Railway for edge | accepted |
| [0006](0006-postgres-17-pinned.md) | Pin PostgreSQL to 17 major | accepted |
| [0007](0007-pgbackrest-wal-archiving.md) | pgBackRest WAL streaming to Backblaze B2 | accepted |
| [0008](0008-hot-only-treasury-with-os-isolation.md) | Hot-only treasury with OS isolation | accepted |
| [0009](0009-automated-weekly-dr-validation.md) | Automated weekly DR validation | accepted |
| [0010](0010-multi-tenant-kaspad-on-pool-vps.md) | Multi-tenant kaspad on the pool VPS (mainnet + testnet-10) | accepted |
| [0011](0011-db-schema-and-migrations.md) | DB schema, sqlx migrations, and legacy import | accepted |
| [0012](0012-fee-model-and-tier-classification.md) | Fee model and wallet-tier classification | accepted |
| [0013](0013-verification-posture.md) | Verification posture (integer-deterministic value math) | accepted |
| [0014](0014-maturity-tracker.md) | Block maturity tracker architecture | accepted |
| [0015](0015-krc20-inscription-envelope.md) | KRC-20 inscription envelope byte-compatible with production | accepted |
| [0016](0016-krc20-payout-conversion-and-floor-price.md) | KAS→NACHO payout conversion, floor price, no payout-time multiplier | accepted |
| [0017](0017-kaspa-version-pinning.md) | Couple kaspad, kaspa-* crates, and Rust toolchain under one version bump | accepted |
| [0018](0018-payout-fee-policy-and-on-demand-cycles.md) | KAS payout fee policy, exact-fee finalization, cadence, and on-demand cycles | accepted |
| [0019](0019-krc20-adaptive-fee-and-fee-persistence.md) | KRC-20 commit/reveal adaptive fees, frozen for crash-safe determinism | accepted |
| [0020](0020-krc20-sweep-coherent-utxo-chaining.md) | KRC-20 sweep-coherent UTXO chaining for sibling commits | accepted |
| [0021](0021-public-read-only-http-api.md) | Public read-only HTTP API: embedded axum service, versioned data surface, DoS posture | accepted |
| [0022](0022-multiport-stratum-and-flyio-anycast-edge.md) | Multi-port stratum and fly.io anycast edge (supersedes 0005's edge) | accepted |
| [0023](0023-new-dashboard-architecture-and-stack.md) | New dashboard architecture and stack | accepted |
| [0024](0024-dashboard-design-overhaul.md) | Dashboard design overhaul | accepted |
| [0025](0025-geo-distribution-via-geolite2.md) | Country-level miner geo distribution via GeoLite2 | accepted |

## When to write a new ADR

Any of the following is a strong signal:

- A change crosses a trust boundary
- A change introduces or removes a third-party service or
  dependency category
- A change alters the deployment shape (host, runtime, datastore)
- A change is hard to reverse (a database migration is hard to
  reverse, swapping `serde_json` for `simd-json` is not)
- A change involves money, custody, or cryptographic primitives
- A reviewer asks for an ADR (always honour this)

If a PR's description starts to sound like the body of an ADR, it
*is* an ADR. Promote it.

## Template

Copy [`template.md`](template.md) to `NNNN-short-title.md` where
NNNN is the next sequential number. Fill in the fields. Submit as
a PR; ADR PRs are reviewed for the rationale, not just the text.
