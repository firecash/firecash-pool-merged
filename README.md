# katpool

[![CI](https://github.com/Nacho-the-Kat/katpool/actions/workflows/ci.yml/badge.svg)](https://github.com/Nacho-the-Kat/katpool/actions/workflows/ci.yml)
[![License: MIT or Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](rust-toolchain.toml)

Rust-first Kaspa mining pool. A single-binary deployment that owns
stratum, share validation, block submission, accounting, KAS payouts, and
native-Rust KRC-20 NACHO rebates — backed by `PostgreSQL`, fronted by
Railway TCP edges, and observed by a self-hosted Grafana LGTM stack.

## Status

> **Live on mainnet.** The Rust pool at `kas.katpool.com` runs stratum,
> accounting, KAS payouts, and NACHO rebates. Architecture is in
> [`docs/architecture.md`](docs/architecture.md); phase history and
> remaining hardening work are in [`docs/roadmap.md`](docs/roadmap.md).

## At a glance

**Runtime** (what `cargo build --bin katpool` ships):

| Component | Responsibility |
|---|---|
| [`bridge/`](bridge/) | Fork of `rusty-kaspa` v1.1.0 in-house stratum bridge. Accepts ASIC stratum, validates shares, submits blocks. Phase 1. |
| [`accountant/`](accountant/) | Subscribes to share + block events, computes PROP allocations, writes balances. Phase 3. |
| [`payout-kas/`](payout-kas/) | Daily KAS payouts; storage-mass-aware via KIP-9/KIP-13; idempotent across restarts. Phase 4. |
| [`payout-krc20/`](payout-krc20/) | Native Rust KRC-20 commit/reveal for NACHO rebate distribution. Phase 5. |
| [`api/`](api/) | Read-only `axum` HTTP API (`/health`, `/balance`, `/api/pool/*`). Phase 6. |
| [`katpool/`](katpool/) | Main wiring binary that runs all of the above in one process. |
| [`crates/`](crates/) | Shared libraries: `katpool-domain`, `-db`, `-config`, `-metrics`, `-storagemass`, `-idempotency`, `-telemetry`, `-secrets`, `-fault-injection`. |

**Frontends** (deploy separately on Railway): `katpool-dashboard-new/`,
`katpool-landing-new/`.

## Architecture

See [`docs/architecture.md`](docs/architecture.md) for the full diagram and
rationale. In short:

```
ASIC miners
   |  stratum TCP
   v
Railway TCP edge (3 regions, anycast-style failover)
   |  WireGuard / public
   v
NetCup VPS  ──  one katpool process  ──  PostgreSQL 17
                  bridge / accountant            |
                  payout-kas / payout-krc20      v
                  api / embedded kaspad        backups → Backblaze B2
   |  vmagent OTLP + push
   v
Railway: self-hosted Grafana + Loki + Tempo + Alertmanager + Blackbox
         Exporter + GlitchTip + Uptime Kuma + ntfy + canary miner
```

## Quick start (development)

Prerequisites: Rust 1.88+ (pinned via [`rust-toolchain.toml`](rust-toolchain.toml)),
Docker (for ephemeral test databases), and `cargo-deny` / `cargo-audit`.

```bash
git clone https://github.com/Nacho-the-Kat/katpool.git
cd katpool

# Verify your environment matches CI gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check

# Run the wiring binary (Phase 0 stub prints linked crate versions)
cargo run --release --bin katpool
```

Full onboarding instructions for new developers (including secret
provisioning, OS hardening for treasury custody, and the local
PostgreSQL setup) are in [`docs/onboarding.md`](docs/onboarding.md).

## Operating principles

- **Determinism**: every reward, mass, and payout calculation is a pure
  function tested with `proptest`. No floating-point money math.
- **Zero plaintext secrets**: treasury key only ever exists sops-encrypted
  on disk; loaded via systemd `LoadCredentialEncrypted`, mlocked, zeroized
  on drop.
- **Idempotent payouts**: every outbound transaction records an idempotency
  key in the DB *before* signing. Mid-cycle restarts cannot double-pay.
- **Pinned everything**: Rust toolchain, container images, and crate
  versions are pinned. Floating tags are CI-rejected.
- **Test pyramid**: unit + property + fuzz + integration + replay-from-prod
  + chaos + load + shadow + DR validation. Every layer is a gate.

## Documentation

Start at [`docs/README.md`](docs/README.md) for the full index. Highlights:

| Path | Topic |
|---|---|
| [`docs/architecture.md`](docs/architecture.md) | Component layout, data flow, deployment topology |
| [`docs/threat-model.md`](docs/threat-model.md) | STRIDE-style threat model and mitigations |
| [`docs/custody.md`](docs/custody.md) | Treasury custody design (sops/age + OS isolation) |
| [`docs/kips.md`](docs/kips.md) | KIP-9 and KIP-13 implementation notes |
| [`docs/capacity-plan.md`](docs/capacity-plan.md) | Sizing and capacity baselines |
| [`docs/cutover-plan.md`](docs/cutover-plan.md) | Production cutover and rollback procedure |
| [`docs/onboarding.md`](docs/onboarding.md) | Developer onboarding |
| [`docs/dev-workflow.md`](docs/dev-workflow.md) | Local-gate ritual, stacked-PR rules, label conventions |
| [`docs/db-schema.md`](docs/db-schema.md) | Database schema reference (operator-facing) |
| [`docs/roadmap.md`](docs/roadmap.md) | Phase status and remaining hardening |
| [`docs/decisions/`](docs/decisions/) | Architectural Decision Records (MADR 4.0) |
| [`docs/runbooks/`](docs/runbooks/) | One runbook per incident class |
| [`archives/`](archives/) | Retired handoffs and one-shot audits (not live docs) |
| [`SECURITY.md`](SECURITY.md) | Vulnerability disclosure policy |
| [`CHANGELOG.md`](CHANGELOG.md) | Release history (Keep a Changelog format) |

## License

Dual-licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.
